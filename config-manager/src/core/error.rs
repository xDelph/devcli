//! Error types for config-manager.

use std::fmt;

/// Result type alias using config-manager's Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for config-manager operations
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error loading configuration
    LoadError(String),

    /// Error saving configuration
    SaveError(String),

    /// Entity not found during resolution
    NotFound {
        id: String,
        suggestion: Option<String>,
    },

    /// Multiple entities found (ambiguous)
    Ambiguous { id: String, candidates: Vec<String> },

    /// Dependency resolution error
    DependencyError(String),

    /// Circular dependency detected
    CircularDependency { path: Vec<String> },

    /// Validation error
    ValidationError(String),

    /// I/O error
    IoError(std::io::Error),

    /// Serialization/deserialization error
    SerdeError(String),

    /// Custom error
    Custom(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::LoadError(msg) => write!(f, "Failed to load configuration: {}", msg),
            Error::SaveError(msg) => write!(f, "Failed to save configuration: {}", msg),
            Error::NotFound { id, suggestion } => {
                if let Some(suggest) = suggestion {
                    write!(f, "Entity '{}' not found. Did you mean '{}'?", id, suggest)
                } else {
                    write!(f, "Entity '{}' not found", id)
                }
            }
            Error::Ambiguous { id, candidates } => {
                write!(
                    f,
                    "Entity '{}' is ambiguous. Found in: {}",
                    id,
                    candidates.join(", ")
                )
            }
            Error::DependencyError(msg) => write!(f, "Dependency error: {}", msg),
            Error::CircularDependency { path } => {
                write!(f, "Circular dependency detected: {}", path.join(" -> "))
            }
            Error::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            Error::IoError(err) => write!(f, "I/O error: {}", err),
            Error::SerdeError(msg) => write!(f, "Serialization error: {}", msg),
            Error::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl Error {
    /// Check if this is a "not found" error.
    #[must_use]
    #[inline]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound { .. })
    }

    /// Check if this is a validation error.
    #[must_use]
    #[inline]
    pub fn is_validation_error(&self) -> bool {
        matches!(self, Error::ValidationError(_))
    }

    /// Check if this is a circular dependency error.
    #[must_use]
    #[inline]
    pub fn is_circular_dependency(&self) -> bool {
        matches!(self, Error::CircularDependency { .. })
    }

    /// Check if this is a load error.
    #[must_use]
    #[inline]
    pub fn is_load_error(&self) -> bool {
        matches!(self, Error::LoadError(_))
    }

    /// Check if this is a save error.
    #[must_use]
    #[inline]
    pub fn is_save_error(&self) -> bool {
        matches!(self, Error::SaveError(_))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::SerdeError(err.to_string())
    }
}

#[cfg(feature = "toml")]
impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Error::SerdeError(err.to_string())
    }
}

#[cfg(feature = "toml")]
impl From<toml::ser::Error> for Error {
    fn from(err: toml::ser::Error) -> Self {
        Error::SerdeError(err.to_string())
    }
}

#[cfg(feature = "yaml")]
impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Error::SerdeError(err.to_string())
    }
}

// Support conversion from anyhow::Error for compatibility
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

// Note: Conversion to anyhow::Error is automatic via the std::error::Error trait
