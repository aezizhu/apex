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

    #[test]
    fn test_error_kind_display_all_variants() {
        assert_eq!(ValidationErrorKind::Required.to_string(), "field is required");
        assert_eq!(ValidationErrorKind::MinLength { min: 3, actual: 1 }.to_string(), "must be at least 3 characters (got 1)");
        assert_eq!(ValidationErrorKind::MaxLength { max: 5, actual: 10 }.to_string(), "must be at most 5 characters (got 10)");
        assert_eq!(ValidationErrorKind::ExactLength { expected: 4, actual: 3 }.to_string(), "must be exactly 4 characters (got 3)");
        assert_eq!(ValidationErrorKind::MinValue { min: "0".into(), actual: "-1".into() }.to_string(), "must be at least 0 (got -1)");
        assert_eq!(ValidationErrorKind::MaxValue { max: "100".into(), actual: "101".into() }.to_string(), "must be at most 100 (got 101)");
        assert_eq!(ValidationErrorKind::Range { min: "0".into(), max: "10".into(), actual: "15".into() }.to_string(), "must be between 0 and 10 (got 15)");
        assert_eq!(ValidationErrorKind::InvalidEmail.to_string(), "must be a valid email address");
        assert_eq!(ValidationErrorKind::InvalidUrl.to_string(), "must be a valid URL");
        assert_eq!(ValidationErrorKind::InvalidUuid.to_string(), "must be a valid UUID");
        assert!(ValidationErrorKind::Pattern { pattern: "\\d+".into() }.to_string().contains("\\d+"));
        assert!(ValidationErrorKind::NotInSet { allowed: vec!["a".into(), "b".into()] }.to_string().contains("a, b"));
        assert_eq!(ValidationErrorKind::MinItems { min: 1, actual: 0 }.to_string(), "must have at least 1 items (got 0)");
        assert_eq!(ValidationErrorKind::MaxItems { max: 3, actual: 5 }.to_string(), "must have at most 3 items (got 5)");
        assert_eq!(ValidationErrorKind::DuplicateItems.to_string(), "must not contain duplicate items");
        assert_eq!(ValidationErrorKind::Nested.to_string(), "nested validation failed");
        assert!(ValidationErrorKind::Custom { code: "ERR001".into() }.to_string().contains("ERR001"));
    }

    #[test]
    fn test_field_error_with_custom_message() {
        let error = FieldError::with_message(ValidationErrorKind::Required, "Name is mandatory");
        assert_eq!(error.to_string(), "Name is mandatory");
        assert_eq!(error.kind, ValidationErrorKind::Required);
    }

    #[test]
    fn test_field_error_with_code() {
        let error = FieldError::new(ValidationErrorKind::Required).with_code("ERR_REQUIRED");
        assert_eq!(error.code, Some("ERR_REQUIRED".to_string()));
    }

    #[test]
    fn test_validation_errors_empty() {
        let errors = ValidationErrors::new();
        assert!(errors.is_empty());
        assert_eq!(errors.error_count(), 0);
        assert_eq!(errors.field_count(), 0);
    }

    #[test]
    fn test_validation_errors_multiple_per_field() {
        let mut errors = ValidationErrors::new();
        errors.add_error("name", ValidationErrorKind::Required);
        errors.add_error("name", ValidationErrorKind::MinLength { min: 2, actual: 0 });
        assert_eq!(errors.field_count(), 1);
        assert_eq!(errors.error_count(), 2);
        assert_eq!(errors.get("name").unwrap().len(), 2);
    }

    #[test]
    fn test_validation_errors_add_with_message() {
        let mut errors = ValidationErrors::new();
        errors.add_with_message("email", ValidationErrorKind::InvalidEmail, "Please provide a valid email");
        let field_errors = errors.get("email").unwrap();
        assert_eq!(field_errors[0].message, "Please provide a valid email");
    }

    #[test]
    fn test_validation_errors_to_message_map() {
        let mut errors = ValidationErrors::new();
        errors.add_required("name");
        errors.add_error("email", ValidationErrorKind::InvalidEmail);
        let map = errors.to_message_map();
        assert!(map.contains_key("name"));
        assert!(map.contains_key("email"));
    }

    #[test]
    fn test_validation_errors_first_error() {
        let mut errors = ValidationErrors::new();
        errors.add_required("first_field");
        let (field, error) = errors.first_error().unwrap();
        assert_eq!(field, "first_field");
        assert_eq!(error.kind, ValidationErrorKind::Required);
    }

    #[test]
    fn test_validation_errors_display() {
        let mut errors = ValidationErrors::new();
        errors.add_required("name");
        let display = errors.to_string();
        assert!(display.contains("name"));
        assert!(display.contains("required"));
    }

    #[test]
    fn test_validation_errors_into_iter() {
        let mut errors = ValidationErrors::new();
        errors.add_required("a");
        errors.add_required("b");
        let collected: Vec<_> = errors.into_iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn test_builder_has_errors() {
        let builder = ValidationErrorBuilder::new();
        assert!(!builder.has_errors());
    }

    #[test]
    fn test_builder_result_ok() {
        let result = ValidationErrorBuilder::new().result();
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_result_err() {
        let result = ValidationErrorBuilder::new()
            .field("email")
            .required()
            .result();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_error_with_message() {
        let errors = ValidationErrorBuilder::new()
            .field("age")
            .error_with_message(ValidationErrorKind::MinValue { min: "18".into(), actual: "15".into() }, "Must be 18+")
            .build();
        assert!(errors.has_errors("age"));
        let field_errors = errors.get("age").unwrap();
        assert_eq!(field_errors[0].message, "Must be 18+");
    }

    #[test]
    fn test_builder_no_field_set() {
        // Adding error with no field set should be a no-op
        let errors = ValidationErrorBuilder::new()
            .error(ValidationErrorKind::Required)
            .build();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_merge_empty_field_prefix() {
        let mut parent = ValidationErrors::new();
        let mut child = ValidationErrors::new();
        child.add_required("");
        parent.merge_with_prefix("root", child);
        assert!(parent.has_errors("root"));
    }

    #[test]
    fn test_merge_overlapping_fields() {
        let mut errors1 = ValidationErrors::new();
        errors1.add_required("field");
        let mut errors2 = ValidationErrors::new();
        errors2.add_error("field", ValidationErrorKind::MinLength { min: 3, actual: 0 });
        errors1.merge(errors2);
        assert_eq!(errors1.error_count(), 2);
        assert_eq!(errors1.field_count(), 1);
    }

    #[test]
    fn test_to_flat_messages() {
        let mut errors = ValidationErrors::new();
        errors.add_required("name");
        errors.add_error("email", ValidationErrorKind::InvalidEmail);
        let messages = errors.to_flat_messages();
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().any(|m| m.contains("name")));
        assert!(messages.iter().any(|m| m.contains("email")));
    }

    #[test]
    fn test_fields_iterator() {
        let mut errors = ValidationErrors::new();
        errors.add_required("a");
        errors.add_required("b");
        errors.add_required("c");
        let fields: Vec<_> = errors.fields().collect();
        assert_eq!(fields.len(), 3);
    }
}
