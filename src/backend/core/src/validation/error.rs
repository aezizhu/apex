//! Validation error types with field-level error support.
//!
//! This module provides comprehensive validation error handling with:
//! - Field-level error tracking
//! - Nested field path support (e.g., "user.address.street")
//! - Multiple errors per field
//! - Serializable error responses for API integration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════════
// Validation Error Types
// ═══════════════════════════════════════════════════════════════════════════════

/// The kind of validation error that occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationErrorKind {
    /// Field is required but was missing or empty.
    Required,
    /// String length is below the minimum.
    MinLength { min: usize, actual: usize },
    /// String length exceeds the maximum.
    MaxLength { max: usize, actual: usize },
    /// String length must be exact.
    ExactLength { expected: usize, actual: usize },
    /// Numeric value is below the minimum.
    MinValue { min: String, actual: String },
    /// Numeric value exceeds the maximum.
    MaxValue { max: String, actual: String },
    /// Value must be within a range.
    Range { min: String, max: String, actual: String },
    /// Value does not match the expected email format.
    InvalidEmail,
    /// Value does not match the expected URL format.
    InvalidUrl,
    /// Value does not match the expected UUID format.
    InvalidUuid,
    /// Value does not match the expected pattern.
    Pattern { pattern: String },
    /// Value is not in the allowed set.
    NotInSet { allowed: Vec<String> },
    /// Array/collection has too few items.
    MinItems { min: usize, actual: usize },
    /// Array/collection has too many items.
    MaxItems { max: usize, actual: usize },
    /// Array/collection contains duplicate items.
    DuplicateItems,
    /// Nested validation failed.
    Nested,
    /// Custom validation failed.
    Custom { code: String },
}

impl fmt::Display for ValidationErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Required => write!(f, "field is required"),
            Self::MinLength { min, actual } => {
                write!(f, "must be at least {} characters (got {})", min, actual)
            }
            Self::MaxLength { max, actual } => {
                write!(f, "must be at most {} characters (got {})", max, actual)
            }
            Self::ExactLength { expected, actual } => {
                write!(f, "must be exactly {} characters (got {})", expected, actual)
            }
            Self::MinValue { min, actual } => {
                write!(f, "must be at least {} (got {})", min, actual)
            }
            Self::MaxValue { max, actual } => {
                write!(f, "must be at most {} (got {})", max, actual)
            }
            Self::Range { min, max, actual } => {
                write!(f, "must be between {} and {} (got {})", min, max, actual)
            }
            Self::InvalidEmail => write!(f, "must be a valid email address"),
            Self::InvalidUrl => write!(f, "must be a valid URL"),
            Self::InvalidUuid => write!(f, "must be a valid UUID"),
            Self::Pattern { pattern } => write!(f, "must match pattern: {}", pattern),
            Self::NotInSet { allowed } => {
                write!(f, "must be one of: {}", allowed.join(", "))
            }
            Self::MinItems { min, actual } => {
                write!(f, "must have at least {} items (got {})", min, actual)
            }
            Self::MaxItems { max, actual } => {
                write!(f, "must have at most {} items (got {})", max, actual)
            }
            Self::DuplicateItems => write!(f, "must not contain duplicate items"),
            Self::Nested => write!(f, "nested validation failed"),
            Self::Custom { code } => write!(f, "validation failed: {}", code),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Field Error
// ═══════════════════════════════════════════════════════════════════════════════

/// A single validation error for a specific field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldError {
    /// The kind of validation error.
    pub kind: ValidationErrorKind,
    /// Human-readable error message.
    pub message: String,
    /// Optional custom error code for client-side handling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl FieldError {
    /// Create a new field error.
    pub fn new(kind: ValidationErrorKind) -> Self {
        let message = kind.to_string();
        Self {
            kind,
            message,
            code: None,
        }
    }

    /// Create a new field error with a custom message.
    pub fn with_message(kind: ValidationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            code: None,
        }
    }

    /// Add a custom error code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Validation Errors Collection
// ═══════════════════════════════════════════════════════════════════════════════

/// A collection of validation errors organized by field path.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationErrors {
    /// Errors organized by field path (e.g., "user.email", "items[0].name").
    #[serde(flatten)]
    errors: HashMap<String, Vec<FieldError>>,
}

impl ValidationErrors {
    /// Create a new empty validation errors collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any validation errors.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the total number of errors across all fields.
    pub fn error_count(&self) -> usize {
        self.errors.values().map(|v| v.len()).sum()
    }

    /// Get the number of fields with errors.
    pub fn field_count(&self) -> usize {
        self.errors.len()
    }

    /// Add an error for a specific field.
    pub fn add(&mut self, field: impl Into<String>, error: FieldError) {
        self.errors
            .entry(field.into())
            .or_default()
            .push(error);
    }

    /// Add an error with just the kind (auto-generates message).
    pub fn add_error(&mut self, field: impl Into<String>, kind: ValidationErrorKind) {
        self.add(field, FieldError::new(kind));
    }

    /// Add an error with a custom message.
    pub fn add_with_message(
        &mut self,
        field: impl Into<String>,
        kind: ValidationErrorKind,
        message: impl Into<String>,
    ) {
        self.add(field, FieldError::with_message(kind, message));
    }

    /// Add a required field error.
    pub fn add_required(&mut self, field: impl Into<String>) {
        self.add_error(field, ValidationErrorKind::Required);
    }

    /// Get errors for a specific field.
    pub fn get(&self, field: &str) -> Option<&Vec<FieldError>> {
        self.errors.get(field)
    }

    /// Check if a specific field has errors.
    pub fn has_errors(&self, field: &str) -> bool {
        self.errors.get(field).map(|v| !v.is_empty()).unwrap_or(false)
    }

    /// Merge another ValidationErrors into this one.
    pub fn merge(&mut self, other: ValidationErrors) {
        for (field, errors) in other.errors {
            self.errors.entry(field).or_default().extend(errors);
        }
    }

    /// Merge errors with a field prefix (for nested validation).
    pub fn merge_with_prefix(&mut self, prefix: &str, other: ValidationErrors) {
        for (field, errors) in other.errors {
            let prefixed_field = if field.is_empty() {
                prefix.to_string()
            } else {
                format!("{}.{}", prefix, field)
            };
            self.errors.entry(prefixed_field).or_default().extend(errors);
        }
    }

    /// Merge errors for array items.
    pub fn merge_array_item(&mut self, field: &str, index: usize, other: ValidationErrors) {
        let prefix = format!("{}[{}]", field, index);
        self.merge_with_prefix(&prefix, other);
    }

    /// Get all field paths that have errors.
    pub fn fields(&self) -> impl Iterator<Item = &String> {
        self.errors.keys()
    }

    /// Iterate over all errors.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<FieldError>)> {
        self.errors.iter()
    }

    /// Convert to a simple map of field -> error messages.
    pub fn to_message_map(&self) -> HashMap<String, Vec<String>> {
        self.errors
            .iter()
            .map(|(field, errors)| {
                (
                    field.clone(),
                    errors.iter().map(|e| e.message.clone()).collect(),
                )
            })
            .collect()
    }

    /// Get the first error message (useful for simple error displays).
    pub fn first_error(&self) -> Option<(&String, &FieldError)> {
        self.errors.iter().next().and_then(|(field, errors)| {
            errors.first().map(|error| (field, error))
        })
    }

    /// Convert to a flat list of error messages with field prefixes.
    pub fn to_flat_messages(&self) -> Vec<String> {
        self.errors
            .iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |e| format!("{}: {}", field, e.message))
            })
            .collect()
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let messages = self.to_flat_messages();
        write!(f, "{}", messages.join("; "))
    }
}

impl std::error::Error for ValidationErrors {}

impl IntoIterator for ValidationErrors {
    type Item = (String, Vec<FieldError>);
    type IntoIter = std::collections::hash_map::IntoIter<String, Vec<FieldError>>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.into_iter()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Validation Result Type
// ═══════════════════════════════════════════════════════════════════════════════

/// Result type for validation operations.
pub type ValidationResult<T> = std::result::Result<T, ValidationErrors>;

/// Extension trait for converting Option to ValidationResult.
pub trait OptionExt<T> {
    /// Convert None to a required field error.
    fn required(self, field: &str) -> ValidationResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn required(self, field: &str) -> ValidationResult<T> {
        match self {
            Some(value) => Ok(value),
            None => {
                let mut errors = ValidationErrors::new();
                errors.add_required(field);
                Err(errors)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Builder Pattern for Errors
// ═══════════════════════════════════════════════════════════════════════════════

/// Builder for creating validation errors fluently.
pub struct ValidationErrorBuilder {
    errors: ValidationErrors,
    current_field: Option<String>,
}

impl ValidationErrorBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            errors: ValidationErrors::new(),
            current_field: None,
        }
    }

    /// Set the current field for subsequent error additions.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.current_field = Some(field.into());
        self
    }

    /// Add an error to the current field.
    pub fn error(mut self, kind: ValidationErrorKind) -> Self {
        if let Some(field) = &self.current_field {
            self.errors.add_error(field.clone(), kind);
        }
        self
    }

    /// Add an error with a custom message to the current field.
    pub fn error_with_message(mut self, kind: ValidationErrorKind, message: impl Into<String>) -> Self {
        if let Some(field) = &self.current_field {
            self.errors.add_with_message(field.clone(), kind, message);
        }
        self
    }

    /// Add a required error to the current field.
    pub fn required(self) -> Self {
        self.error(ValidationErrorKind::Required)
    }

    /// Build the validation errors.
    pub fn build(self) -> ValidationErrors {
        self.errors
    }

    /// Check if any errors were added.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Convert to Result - Ok(()) if no errors, Err(errors) otherwise.
    pub fn result(self) -> ValidationResult<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }
}

impl Default for ValidationErrorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_error_display() {
        let error = FieldError::new(ValidationErrorKind::Required);
        assert_eq!(error.to_string(), "field is required");

        let error = FieldError::new(ValidationErrorKind::MinLength { min: 3, actual: 1 });
        assert_eq!(error.to_string(), "must be at least 3 characters (got 1)");
    }

    #[test]
    fn test_validation_errors_add_and_get() {
        let mut errors = ValidationErrors::new();
        errors.add_required("email");
        errors.add_error("name", ValidationErrorKind::MinLength { min: 2, actual: 1 });

        assert_eq!(errors.field_count(), 2);
        assert_eq!(errors.error_count(), 2);
        assert!(errors.has_errors("email"));
        assert!(errors.has_errors("name"));
        assert!(!errors.has_errors("other"));
    }

    #[test]
    fn test_validation_errors_merge() {
        let mut errors1 = ValidationErrors::new();
        errors1.add_required("field1");

        let mut errors2 = ValidationErrors::new();
        errors2.add_required("field2");

        errors1.merge(errors2);
        assert_eq!(errors1.field_count(), 2);
    }

    #[test]
    fn test_validation_errors_merge_with_prefix() {
        let mut parent = ValidationErrors::new();

        let mut child = ValidationErrors::new();
        child.add_required("street");
        child.add_required("city");

        parent.merge_with_prefix("address", child);

        assert!(parent.has_errors("address.street"));
        assert!(parent.has_errors("address.city"));
    }

    #[test]
    fn test_validation_errors_array_item() {
        let mut errors = ValidationErrors::new();

        let mut item_errors = ValidationErrors::new();
        item_errors.add_required("name");

        errors.merge_array_item("items", 0, item_errors);

        assert!(errors.has_errors("items[0].name"));
    }

    #[test]
    fn test_builder_pattern() {
        let errors = ValidationErrorBuilder::new()
            .field("email")
            .required()
            .field("name")
            .error(ValidationErrorKind::MinLength { min: 2, actual: 1 })
            .build();

        assert_eq!(errors.field_count(), 2);
    }

    #[test]
    fn test_option_required() {
        let some_value: Option<i32> = Some(42);
        let result = some_value.required("value");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let none_value: Option<i32> = None;
        let result = none_value.required("value");
        assert!(result.is_err());
        assert!(result.unwrap_err().has_errors("value"));
    }
}
