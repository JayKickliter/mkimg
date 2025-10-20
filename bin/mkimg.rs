use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use mkimg::{self, FileMapping};
use std::fs::File;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser)]
enum Commands {
    /// Create a disk img (deceptive by default).
    Create {
        /// Output path name for the created img.
        img_path: Option<Utf8PathBuf>,
        /// Create a plain (non-deceptive) img instead of deceptive.
        #[arg(long)]
        plain: bool,
        /// If set, only the root dir contents will be included.
        ///
        /// If not set, the root of the img will only be the provided
        /// root dir.
        #[arg(short, long)]
        exclude_root: bool,
        /// The root path of the created img.
        #[arg(long)]
        root: Option<Utf8PathBuf>,
        /// A mapping from <EXT PATH> <INT PATH>.
        #[arg(long, conflicts_with = "root", num_args = 2)]
        map: Vec<Utf8PathBuf>,
    },
    /// Examine an existing disk img
    Examine {
        /// Path to the disk img to examine
        img_path: Utf8PathBuf,
    },
    /// Extract a file from a disk img.
    Extract {
        /// Path to the disk img.
        img_path: Utf8PathBuf,
        /// Path to the file within the img (e.g.,
        /// ""EFI/boot/bootx64.efi"").
        file_path: Utf8PathBuf,
        /// Output path for the extracted file/
        output_path: Utf8PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Create {
            root,
            img_path,
            plain,
            exclude_root,
            map,
        } => {
            let file_mappings = if let Some(root) = root {
                mkimg::create_mappings(&root, exclude_root)?
            } else {
                let mut mappings = Vec::new();
                for pair in map.chunks(2) {
                    if pair.len() != 2 {
                        bail!("File mappings must be src dst src dst ..");
                    }
                    mappings.push(FileMapping {
                        ext: pair[0].clone(),
                        int: pair[1].clone(),
                    })
                }
                mappings
            };
            let img_path = img_path.unwrap_or_else(|| {
                if plain {
                    Utf8PathBuf::from("disk.img")
                } else {
                    Utf8PathBuf::from("deceptive.img")
                }
            });
            let mut img_file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .read(true)
                .write(true)
                .open(img_path)?;
            if plain {
                mkimg::create(&mut img_file, &file_mappings)?;
            } else {
                mkimg::create_deceptive_img(&mut img_file, &file_mappings)?;
            }
        }
        Commands::Examine { img_path } => {
            let img_file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .read(true)
                .write(true)
                .open(img_path)?;
            mkimg::examine(&img_file)?;
        }
        Commands::Extract {
            img_path,
            file_path,
            output_path,
        } => {
            let mut img_file = File::open(img_path)?;
            let mut buf = Vec::new();
            mkimg::extract(&mut img_file, &file_path, &mut buf)?;
            std::fs::write(output_path, &buf)?;
        }
    }
    Ok(())
}
