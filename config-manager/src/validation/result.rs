//! Validation result types.

use std::fmt;

/// Result of validation, containing errors and warnings.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// Create a new empty validation result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an error to the result.
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Add a warning to the result.
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// Check if validation passed (no errors).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get all errors.
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Get all warnings.
    pub fn warnings(&self) -> &[ValidationWarning] {
        &self.warnings
    }

    /// Get the number of errors.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Get the number of warnings.
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Merge another validation result into this one.
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "Validation passed")?;
            if !self.warnings.is_empty() {
                write!(f, " ({} warnings)", self.warnings.len())?;
            }
            Ok(())
        } else {
            writeln!(f, "Validation failed with {} error(s):", self.errors.len())?;
            for error in &self.errors {
                writeln!(f, "  - {}", error)?;
            }
            if !self.warnings.is_empty() {
                writeln!(f, "Warnings ({}):", self.warnings.len())?;
                for warning in &self.warnings {
                    writeln!(f, "  - {}", warning)?;
                }
            }
            Ok(())
        }
    }
}

/// A validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    field: String,
    message: String,
}

impl ValidationError {
    /// Create a new validation error.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Get the field that failed validation.
    pub fn field(&self) -> &str {
        &self.field
    }

    /// Get the error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// A validation warning.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    field: String,
    message: String,
}

impl ValidationWarning {
    /// Create a new validation warning.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Get the field that triggered the warning.
    pub fn field(&self) -> &str {
        &self.field
    }

    /// Get the warning message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}
