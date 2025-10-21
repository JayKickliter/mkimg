use std::{
    fmt, io,
    path::{Path, PathBuf},
};

/// Custom Result type for mkimg operations
pub type MkimgRes<T = ()> = std::result::Result<T, MkimgError>;

/// Custom error type for mkimg operations
#[derive(Debug)]
pub enum MkimgError {
    /// I/O operation failed
    Io(io::Error),
    /// Path operation failed (canonicalization, strip_prefix, etc.)
    Path {
        operation: String,
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Invalid path string conversion
    InvalidPath { path: PathBuf, message: String },
    /// Validation error with custom message
    Validation(String),
    /// WalkDir iteration error
    WalkDir(walkdir::Error),
}

impl fmt::Display for MkimgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MkimgError::Io(err) => write!(f, "I/O error: {}", err),
            MkimgError::Path {
                operation,
                path,
                source,
            } => {
                write!(
                    f,
                    "Path {} failed for '{}': {}",
                    operation,
                    path.display(),
                    source
                )
            }
            MkimgError::InvalidPath { path, message } => {
                write!(f, "Invalid path '{}': {}", path.display(), message)
            }
            MkimgError::Validation(msg) => write!(f, "Validation error: {}", msg),
            MkimgError::WalkDir(err) => write!(f, "Directory traversal error: {}", err),
        }
    }
}

impl std::error::Error for MkimgError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MkimgError::Io(err) => Some(err),
            MkimgError::Path { source, .. } => Some(source.as_ref()),
            MkimgError::WalkDir(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for MkimgError {
    fn from(err: io::Error) -> Self {
        MkimgError::Io(err)
    }
}

impl From<walkdir::Error> for MkimgError {
    fn from(err: walkdir::Error) -> Self {
        MkimgError::WalkDir(err)
    }
}

impl From<std::path::StripPrefixError> for MkimgError {
    fn from(err: std::path::StripPrefixError) -> Self {
        MkimgError::Path {
            operation: "strip_prefix".to_string(),
            path: PathBuf::new(),
            source: Box::new(err),
        }
    }
}

impl MkimgError {
    /// Create a validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        MkimgError::Validation(msg.into())
    }

    /// Create an invalid path error
    pub fn invalid_path(path: impl Into<PathBuf>, msg: impl Into<String>) -> Self {
        MkimgError::InvalidPath {
            path: path.into(),
            message: msg.into(),
        }
    }

    /// Create a path operation error
    pub fn path_operation(
        operation: impl Into<String>,
        path: impl Into<PathBuf>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        MkimgError::Path {
            operation: operation.into(),
            path: path.into(),
            source: Box::new(source),
        }
    }
}

/// Helper function to handle strip_prefix errors with context
pub fn strip_prefix_with_context<'a>(path: &'a Path, prefix: &Path) -> MkimgRes<&'a Path> {
    path.strip_prefix(prefix)
        .map_err(|err| MkimgError::path_operation("strip_prefix", path.to_path_buf(), err))
}

/// Helper function to handle canonicalize errors with context
pub fn canonicalize_with_context(path: &Path) -> MkimgRes<PathBuf> {
    path.canonicalize()
        .map_err(|err| MkimgError::path_operation("canonicalize", path.to_path_buf(), err))
}

/// Helper function to convert OsStr to str with context
pub fn path_to_str_with_context(path: &Path) -> MkimgRes<&str> {
    path.to_str().ok_or_else(|| {
        MkimgError::invalid_path(path.to_path_buf(), "path contains invalid UTF-8 characters")
    })
}
