use crate::ConfigError;
use std::fs;
use std::path::{Path, PathBuf};

pub struct SecureFileLoader {
    allowed_dirs: Vec<PathBuf>,
    max_file_size: usize,
}

impl SecureFileLoader {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        Self {
            allowed_dirs,
            max_file_size: 10_485_760, // 10MB
        }
    }

    pub fn load_file(&self, file_path: &str) -> Result<String, ConfigError> {
        // Convert to Path
        let path = Path::new(file_path);

        // Canonicalize to resolve symlinks and relative paths
        let canonical_path = path.canonicalize()
            .map_err(|e| ConfigError::Io(e.to_string()))?;

        // Check if path is within allowed directories
        let is_allowed = self.allowed_dirs.iter().any(|allowed| {
            if let Ok(canonical_allowed) = allowed.canonicalize() {
                canonical_path.starts_with(canonical_allowed)
            } else {
                false
            }
        });

        if !is_allowed {
            return Err(ConfigError::ValidationFailed(
                "Access denied: path outside allowed directories".to_string()
            ));
        }

        // Additional safety check - the canonical path shouldn't contain ..
        if canonical_path.to_string_lossy().contains("..") {
            return Err(ConfigError::ValidationFailed(
                "Path traversal detected".to_string()
            ));
        }

        // Check file size before reading
        let metadata = fs::metadata(&canonical_path)
            .map_err(|e| ConfigError::Io(e.to_string()))?;
        
        if metadata.len() > self.max_file_size as u64 {
            return Err(ConfigError::ValidationFailed(
                format!("File exceeds maximum size of {} bytes", self.max_file_size)
            ));
        }

        // Read the file
        fs::read_to_string(canonical_path)
            .map_err(|e| ConfigError::Io(e.to_string()))
    }

    pub fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_file_size = max_size;
        self
    }
}