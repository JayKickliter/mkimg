# mkimg

Create bootable FAT16/FAT32 disk images from directories.

## Overview

`mkimg` is a Rust library and CLI tool for creating disk images from
directory contents. It can create standard FAT filesystem images.

This project was created to generate bootable virtual floppies for testing
UEFI firmware.

**Note**: This project is very alpha. It does not automatically calculate the
needed image size based on input files. Image sizes are currently fixed at
6MB for plain images and start at 32MB for modified images.


## Installation

```bash
cargo build --release
```

## Usage

### CLI Commands

#### Create Image

Create a disk image from a directory:

```bash
# Create modified image (default)
mkimg create --root /path/to/directory

# Create plain image
mkimg create --root /path/to/directory --plain

# Exclude root directory from image structure
mkimg create --root /path/to/directory --exclude-root

# Custom output path
mkimg create --root /path/to/directory output.img

# Manual file mappings
mkimg create --map /local/file1.txt /image/file1.txt \
             --map /local/file2.txt /image/file2.txt
```

#### Examine Image

List contents of an existing disk image:

```bash
mkimg examine disk.img
```

#### Extract File

Extract a specific file from a disk image:

```bash
mkimg extract disk.img "path/in/image.txt" output.txt
```

## Library Functions

### Core Functions

#### `create_mappings(root: &Path, exclude_root: bool) -> Result<Vec<FileMapping>>`

Scans a directory tree and creates file mappings for image creation.

- `root` - Source directory to scan
- `exclude_root` - If true, only directory contents are included. If false,
  the root directory itself becomes the image root
- Returns vector of `FileMapping` structs containing source and destination
  paths

#### `create(img_file: &mut File, file_mappings: &[FileMapping]) -> Result<()>`

Creates a standard FAT16 disk image (6MB).

- `img_file` - Output file handle for the image
- `file_mappings` - Vector of files to include in the image

#### `create_deceptive_img(img_file: &mut File, file_mappings: &[FileMapping]) -> Result<()>`

Creates a modified FAT32 disk image that reports altered size information.

- `img_file` - Output file handle for the image
- `file_mappings` - Vector of files to include in the image
- Creates 32MB image initially, applies size modification, then shrinks to
  actual content size

#### `examine(img_file: &File) -> Result<()>`

Prints detailed contents of a disk image including directory structure and
file contents for small files.

- `img_file` - Image file to examine

#### `extract(img_file: &mut File, target_path: &Path, buf: &mut Vec<u8>) -> Result<()>`

Extracts a single file from a disk image.

- `img_file` - Source image file
- `target_path` - Path to file within the image filesystem
- `buf` - Buffer to store extracted file contents

### Data Structures

#### `FileMapping`

Represents mapping between external filesystem and image filesystem paths.

```rust
pub struct FileMapping {
    pub ext: PathBuf,  // Source file path
    pub int: PathBuf,  // Destination path in image
}
```

## Implementation Details

### Image Types

- **Plain images**: Standard FAT16 filesystem, 6MB fixed size
- **Modified images**: FAT32 filesystem with modified boot sector claiming
  1.5x actual size, then shrunk to minimal size while maintaining the
  modification

### Filesystem Support

- FAT16 for plain images
- FAT32 for deceptive images
- Automatic directory creation
- Preserves file contents and basic directory structure

## License

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
  ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
