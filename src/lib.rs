use anyhow::{anyhow, bail, Result};
use fatfs::{FileSystem, FormatVolumeOptions, FsOptions};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

/// Mapping from external to image file.
pub struct FileMapping {
    /// Path to source file in external filesystem.
    pub ext: PathBuf,
    /// Where to place the file in the image filesystem.
    pub int: PathBuf,
}

/// Scans a directory tree and creates file mappings for image creation.
///
/// # Arguments
/// * `root` - Source directory to scan
/// * `exclude_root` - If true, only directory contents are included. If false, the root directory itself becomes the image root
///
/// # Returns
/// Vector of `FileMapping` structs containing source and destination paths
///
/// # Errors
/// Returns error if root is not a directory or filesystem operations fail
pub fn create_mappings(root: &Path, exclude_root: bool) -> Result<Vec<FileMapping>> {
    if !root.is_dir() {
        bail!("root must be a directory")
    };
    let canon_root = {
        let mut canon = root.canonicalize()?;
        if !exclude_root {
            canon.pop();
        }
        canon
    };
    let tree = WalkDir::new(root);
    let rerooted_mappings = reroot_tree(&canon_root, tree)?;
    Ok(rerooted_mappings)
}

/// Creates a standard FAT16 disk image (6MB fixed size).
///
/// # Arguments
/// * `img_file` - Output file handle for the image
/// * `file_mappings` - Vector of files to include in the image
///
/// # Errors
/// Returns error if filesystem operations fail
pub fn create(img_file: &mut File, file_mappings: &[FileMapping]) -> Result<()> {
    img_file.set_len(6 * 1024 * 1024)?;
    write_fs(img_file, file_mappings, fatfs::FatType::Fat16)?;
    Ok(())
}

/// Prints detailed contents of a disk image including directory structure and file contents for small files.
///
/// # Arguments
/// * `img_file` - Image file to examine
///
/// # Errors
/// Returns error if image cannot be read or is not a valid FAT filesystem
pub fn examine(img_file: &File) -> Result<()> {
    let fs = FileSystem::new(img_file, FsOptions::new())?;
    let fs_root = fs.root_dir();
    for entry in fs_root.iter() {
        let entry = entry?;
        let name = entry.file_name();
        let is_dir = entry.is_dir();
        let size = if is_dir { 0 } else { entry.len() };
        let is_dir_tag = if is_dir { "(DIR)" } else { "(FILE)" };
        println!("{name} {size} bytes {is_dir_tag}");
        if is_dir && name != "." && name != ".." {
            examine_directory(&fs_root, &name, 1)?;
        }
    }
    Ok(())
}

/// Extracts a single file from a disk image.
///
/// # Arguments
/// * `img_file` - Source image file
/// * `target_path` - Path to file within the image filesystem
/// * `buf` - Buffer to store extracted file contents
///
/// # Errors
/// Returns error if file not found or filesystem operations fail
pub fn extract(img_file: &mut File, target_path: &Path, buf: &mut Vec<u8>) -> Result<()> {
    let fs = FileSystem::new(img_file, FsOptions::new())?;
    let root_dir = fs.root_dir();
    let target_parts = target_path.iter().collect::<Vec<_>>();

    // Navigate through directories to find the file
    let mut current_path = String::new();
    for (i, part) in target_parts.iter().enumerate() {
        let part = part
            .to_str()
            .ok_or_else(|| anyhow!("invalid str {part:?}"))?;

        if i == target_parts.len() - 1 {
            // This is the filename, open the file
            let dir = if current_path.is_empty() {
                root_dir.clone()
            } else {
                root_dir.open_dir(&current_path)?
            };
            let mut file = dir.open_file(part)?;
            file.read_to_end(buf)?;
            break;
        } else {
            // This is a directory, add to path
            if !current_path.is_empty() {
                current_path.push('/');
            }
            current_path.push_str(part);
        }
    }

    Ok(())
}

// Create filesystem with FAT32 and copy files
fn write_fs(img_file: &mut File, tree: &[FileMapping], fat_type: fatfs::FatType) -> Result<()> {
    {
        fatfs::format_volume(
            &mut *img_file,
            FormatVolumeOptions::new().fat_type(fat_type),
        )?;
    }
    let fs = FileSystem::new(img_file, FsOptions::new())?;
    let root_dir = fs.root_dir();

    // Copy files from the source directory
    for FileMapping {
        ext: external_path,
        int: internal_path,
    } in tree
    {
        // Skip directories - only process files
        if external_path.is_dir() {
            continue;
        }

        let path_parts: Vec<_> = internal_path
            .to_str()
            .ok_or_else(|| anyhow!("invalid str {internal_path:?}"))?
            .split('/')
            .collect();

        // Create parent directories as needed
        let mut current_dir = &root_dir;
        let mut owned_dirs = Vec::new();

        for part in &path_parts[..path_parts.len() - 1] {
            if !part.is_empty() {
                match current_dir.open_dir(part) {
                    Ok(dir) => {
                        owned_dirs.push(dir);
                        current_dir = owned_dirs.last().unwrap();
                    }
                    Err(_) => {
                        current_dir.create_dir(part)?;
                        let dir = current_dir.open_dir(part)?;
                        owned_dirs.push(dir);
                        current_dir = owned_dirs.last().unwrap();
                    }
                }
            }
        }

        if let Some(filename) = path_parts.last().filter(|last| !last.is_empty()) {
            let file_content = std::fs::read(external_path)?;
            let mut file = current_dir.create_file(filename)?;
            file.write_all(&file_content)?;
            file.flush()?;
        }
    }

    drop(root_dir);
    fs.unmount()?;
    Ok(())
}

fn examine_directory(
    parent_dir: &fatfs::Dir<'_, &File>,
    dir_name: &str,
    depth: usize,
) -> Result<()> {
    let indent = "  ".repeat(depth + 1);
    if let Ok(subdir) = parent_dir.open_dir(dir_name) {
        println!("{}Contents of {}:", indent, dir_name);
        for entry in subdir.iter() {
            let entry = entry?;
            let name = entry.file_name();
            let is_dir = entry.is_dir();
            let size = if is_dir { 0 } else { entry.len() };
            println!(
                "{}  {} {} bytes {}",
                indent,
                name,
                size,
                if is_dir { "(DIR)" } else { "(FILE)" }
            );

            // Read file contents if it's a small file
            if let Ok(mut file) = subdir.open_file(&name) {
                if !is_dir && size <= 200000 {
                    let mut contents = Vec::new();
                    if file.read_to_end(&mut contents).is_ok() {
                        if contents.iter().all(|&b| {
                            b.is_ascii() && !b.is_ascii_control()
                                || b == b'\n'
                                || b == b'\r'
                                || b == b'\t'
                        }) {
                            println!(
                                "{}    Content: {:?}",
                                indent,
                                String::from_utf8_lossy(&contents)
                            );
                        } else {
                            println!(
                                "{}    Content: {} bytes of binary data",
                                indent,
                                contents.len()
                            );
                        }
                    }
                }
            }

            // Recursively explore subdirectories
            if is_dir && name != "." && name != ".." && depth < 5 {
                examine_directory(&subdir, &name, depth + 1)?;
            }
        }
    }
    Ok(())
}

/// Creates a deceptive FAT32 disk image that reports false size information.
///
/// Creates a 32MB FAT32 image, applies size deception to boot sector and FSInfo,
/// then shrinks the file to actual content size while maintaining the deception.
/// The resulting image will report 1.5x its actual size to basic filesystem queries.
///
/// # Arguments
/// * `img_file` - Output file handle for the image
/// * `file_mappings` - Vector of files to include in the image
///
/// # Errors
/// Returns error if filesystem operations fail
pub fn create_deceptive_img(img_file: &mut File, file_mappings: &[FileMapping]) -> Result<()> {
    // 32MB real size to ensure FAT32
    img_file.set_len(32 * 1024 * 1024)?;
    write_fs(img_file, file_mappings, fatfs::FatType::Fat32)?;
    apply_size_deception(img_file)?;
    shrink_file_after_deception(img_file)?;
    println!("Deceptive img created successfully!");
    Ok(())
}

fn apply_size_deception(img_file: &mut File) -> Result<()> {
    // Read the current boot sector
    let mut boot_sector = [0u8; 512];
    img_file.read_exact(&mut boot_sector)?;

    // Modify the total sectors field at offset 0x20 (32-bit value)
    // Use a more moderate deception: claim 4x the actual size
    let current_sectors = u32::from_le_bytes([
        boot_sector[0x20],
        boot_sector[0x21],
        boot_sector[0x22],
        boot_sector[0x23],
    ]);
    let fake_sectors: u32 = current_sectors + (current_sectors / 2); // 1.5x deception for better compatibility
    boot_sector[0x20..0x24].copy_from_slice(&fake_sectors.to_le_bytes());

    // Write back the modified boot sector
    img_file.seek(SeekFrom::Start(0))?;
    img_file.write_all(&boot_sector)?;

    // Also modify the FSInfo sector (usually at sector 1)
    img_file.seek(SeekFrom::Start(512))?;
    let mut fsinfo_sector = [0u8; 512];
    img_file.read_exact(&mut fsinfo_sector)?;

    // Check if this is actually an FSInfo sector (signature "RRaA" at offset 0)
    if &fsinfo_sector[0x00..0x04] == b"RRaA" {
        // Modify free cluster count at offset 0x1e8 to match our moderate deception
        let current_free = u32::from_le_bytes([
            fsinfo_sector[0x1e8],
            fsinfo_sector[0x1e9],
            fsinfo_sector[0x1ea],
            fsinfo_sector[0x1eb],
        ]);
        let fake_free_clusters: u32 = if current_free != 0xFFFFFFFF {
            current_free * 3
        } else {
            current_free
        };
        fsinfo_sector[0x1e8..0x1ec].copy_from_slice(&fake_free_clusters.to_le_bytes());

        // Write back the modified FSInfo sector
        img_file.seek(SeekFrom::Start(512))?;
        img_file.write_all(&fsinfo_sector)?;
    }

    img_file.flush()?;
    println!("Applied size deception - img now claims to be 1.5x actual size");
    Ok(())
}

fn shrink_file_after_deception(img_file: &mut File) -> Result<()> {
    // Find the last non-zero byte to determine minimum file size
    // Start from a reasonable minimum (like 512KB) and extend as needed
    let min_size = 512 * 1024; // 512KB minimum
    let mut actual_size = min_size;
    let mut content = Vec::with_capacity(img_file.metadata()?.len() as usize);
    img_file.read_to_end(&mut content)?;
    // Look for actual data beyond the minimum
    for i in (min_size..content.len()).rev() {
        if content[i] != 0 {
            actual_size = ((i / 512) + 1) * 512; // Round up to next sector
            break;
        }
    }
    img_file.set_len(actual_size as u64)?;
    img_file.flush()?;
    println!(
        "Shrunk file to {} bytes while maintaining deception",
        actual_size
    );
    Ok(())
}

/// Returns `(total size, [(external src, internal path), ..])`
fn reroot_tree(canon_root: &Path, walkdir: WalkDir) -> Result<Vec<FileMapping>> {
    let mut out = Vec::new();
    for entry in walkdir {
        let entry = entry?;
        let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let entry_path_buf = entry.path().to_path_buf();
        let rerooted_path = reroot_path(canon_root, &entry_path_buf)?;
        println!("{rerooted_path:?} {entry_path_buf:?} {len}");
        if rerooted_path != Path::new("") {
            out.push(FileMapping {
                ext: entry_path_buf,
                int: rerooted_path,
            });
        }
    }
    Ok(out)
}

fn reroot_path(canon_root: &Path, target: &Path) -> Result<PathBuf> {
    let canon_target = target.canonicalize()?;
    let rerooted_target = canon_target.strip_prefix(canon_root)?.to_path_buf();
    Ok(rerooted_target)
}
