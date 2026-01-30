//! Validation rules for common data validation scenarios.
//!
//! This module provides a comprehensive set of validation rules including:
//! - Required field validation
//! - String length constraints
//! - Numeric range constraints
//! - Format validation (email, URL, UUID)
//! - Custom regex pattern matching
//! - Collection size constraints

use crate::validation::error::{FieldError, ValidationErrorKind, ValidationErrors, ValidationResult};
use regex::Regex;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::LazyLock;

// ═══════════════════════════════════════════════════════════════════════════════
// Pre-compiled Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Email validation regex (RFC 5322 simplified).
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"
    ).expect("Invalid email regex")
});

/// URL validation regex.
static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^https?://(?:www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b(?:[-a-zA-Z0-9()@:%_\+.~#?&/=]*)$"
    ).expect("Invalid URL regex")
});

/// UUID validation regex (v4).
static UUID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
    ).expect("Invalid UUID regex")
});

/// Alphanumeric validation regex.
static ALPHANUMERIC_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9]+$").expect("Invalid alphanumeric regex")
});

/// Slug validation regex (lowercase letters, numbers, hyphens).
static SLUG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").expect("Invalid slug regex")
});

/// Phone number validation regex (basic international format).
static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\+?[1-9]\d{1,14}$").expect("Invalid phone regex")
});

// ═══════════════════════════════════════════════════════════════════════════════
// Validation Rule Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// A validation rule that can be applied to a value.
pub trait ValidationRule<T> {
    /// Validate the value and return any errors.
    fn validate(&self, value: &T) -> Option<FieldError>;

    /// Get a description of this rule.
    fn description(&self) -> String;
}

// ═══════════════════════════════════════════════════════════════════════════════
// Required Field Rule
// ═══════════════════════════════════════════════════════════════════════════════

/// Rule that validates a field is present and non-empty.
#[derive(Debug, Clone)]
pub struct Required;

impl ValidationRule<String> for Required {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.trim().is_empty() {
            Some(FieldError::new(ValidationErrorKind::Required))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        "field is required".to_string()
    }
}

impl<T> ValidationRule<Vec<T>> for Required {
    fn validate(&self, value: &Vec<T>) -> Option<FieldError> {
        if value.is_empty() {
            Some(FieldError::new(ValidationErrorKind::Required))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        "field is required".to_string()
    }
}

/// Rule that validates an optional field is present (Some).
/// For `Option<String>`, use `RequiredString` to also check for non-empty.
#[derive(Debug, Clone)]
pub struct RequiredOption;

impl<T> ValidationRule<Option<T>> for RequiredOption {
    fn validate(&self, value: &Option<T>) -> Option<FieldError> {
        if value.is_none() {
            Some(FieldError::new(ValidationErrorKind::Required))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        "field is required".to_string()
    }
}

/// Rule that validates an optional string is present and non-empty.
#[derive(Debug, Clone)]
pub struct RequiredString;

impl ValidationRule<Option<String>> for RequiredString {
    fn validate(&self, value: &Option<String>) -> Option<FieldError> {
        match value {
            Some(s) if !s.trim().is_empty() => None,
            _ => Some(FieldError::new(ValidationErrorKind::Required)),
        }
    }

    fn description(&self) -> String {
        "field is required and must not be empty".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// String Length Rules
// ═══════════════════════════════════════════════════════════════════════════════

/// Rule that validates string minimum length.
#[derive(Debug, Clone)]
pub struct MinLength(pub usize);

impl ValidationRule<String> for MinLength {
    fn validate(&self, value: &String) -> Option<FieldError> {
        let len = value.chars().count();
        if len < self.0 {
            Some(FieldError::new(ValidationErrorKind::MinLength {
                min: self.0,
                actual: len,
            }))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        format!("minimum length: {}", self.0)
    }
}

impl ValidationRule<Option<String>> for MinLength {
    fn validate(&self, value: &Option<String>) -> Option<FieldError> {
        match value {
            Some(s) => <MinLength as ValidationRule<String>>::validate(self, s),
            None => None, // Not validating presence, just length if present
        }
    }

    fn description(&self) -> String {
        format!("minimum length: {}", self.0)
    }
}

/// Rule that validates string maximum length.
#[derive(Debug, Clone)]
pub struct MaxLength(pub usize);

impl ValidationRule<String> for MaxLength {
    fn validate(&self, value: &String) -> Option<FieldError> {
        let len = value.chars().count();
        if len > self.0 {
            Some(FieldError::new(ValidationErrorKind::MaxLength {
                max: self.0,
                actual: len,
            }))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        format!("maximum length: {}", self.0)
    }
}

impl ValidationRule<Option<String>> for MaxLength {
    fn validate(&self, value: &Option<String>) -> Option<FieldError> {
        match value {
            Some(s) => <MaxLength as ValidationRule<String>>::validate(self, s),
            None => None,
        }
    }

    fn description(&self) -> String {
        format!("maximum length: {}", self.0)
    }
}

/// Rule that validates exact string length.
#[derive(Debug, Clone)]
pub struct ExactLength(pub usize);

impl ValidationRule<String> for ExactLength {
    fn validate(&self, value: &String) -> Option<FieldError> {
        let len = value.chars().count();
        if len != self.0 {
            Some(FieldError::new(ValidationErrorKind::ExactLength {
                expected: self.0,
                actual: len,
            }))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        format!("exact length: {}", self.0)
    }
}

/// Rule that validates string length is within a range.
#[derive(Debug, Clone)]
pub struct LengthRange {
    pub min: usize,
    pub max: usize,
}

impl LengthRange {
    pub fn new(min: usize, max: usize) -> Self {
        Self { min, max }
    }
}

impl ValidationRule<String> for LengthRange {
    fn validate(&self, value: &String) -> Option<FieldError> {
        let len = value.chars().count();
        if len < self.min {
            Some(FieldError::new(ValidationErrorKind::MinLength {
                min: self.min,
                actual: len,
            }))
        } else if len > self.max {
            Some(FieldError::new(ValidationErrorKind::MaxLength {
                max: self.max,
                actual: len,
            }))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        format!("length between {} and {}", self.min, self.max)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Numeric Range Rules
// ═══════════════════════════════════════════════════════════════════════════════

/// Rule that validates minimum numeric value.
#[derive(Debug, Clone)]
pub struct Min<T>(pub T);

macro_rules! impl_min_rule {
    ($($t:ty),+) => {
        $(
            impl ValidationRule<$t> for Min<$t> {
                fn validate(&self, value: &$t) -> Option<FieldError> {
                    if *value < self.0 {
                        Some(FieldError::new(ValidationErrorKind::MinValue {
                            min: self.0.to_string(),
                            actual: value.to_string(),
                        }))
                    } else {
                        None
                    }
                }

                fn description(&self) -> String {
                    format!("minimum value: {}", self.0)
                }
            }

            impl ValidationRule<Option<$t>> for Min<$t> {
                fn validate(&self, value: &Option<$t>) -> Option<FieldError> {
                    match value {
                        Some(v) => <Min<$t> as ValidationRule<$t>>::validate(self, v),
                        None => None,
                    }
                }

                fn description(&self) -> String {
                    format!("minimum value: {}", self.0)
                }
            }
        )+
    };
}

impl_min_rule!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

/// Rule that validates maximum numeric value.
#[derive(Debug, Clone)]
pub struct Max<T>(pub T);

macro_rules! impl_max_rule {
    ($($t:ty),+) => {
        $(
            impl ValidationRule<$t> for Max<$t> {
                fn validate(&self, value: &$t) -> Option<FieldError> {
                    if *value > self.0 {
                        Some(FieldError::new(ValidationErrorKind::MaxValue {
                            max: self.0.to_string(),
                            actual: value.to_string(),
                        }))
                    } else {
                        None
                    }
                }

                fn description(&self) -> String {
                    format!("maximum value: {}", self.0)
                }
            }

            impl ValidationRule<Option<$t>> for Max<$t> {
                fn validate(&self, value: &Option<$t>) -> Option<FieldError> {
                    match value {
                        Some(v) => <Max<$t> as ValidationRule<$t>>::validate(self, v),
                        None => None,
                    }
                }

                fn description(&self) -> String {
                    format!("maximum value: {}", self.0)
                }
            }
        )+
    };
}

impl_max_rule!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

/// Rule that validates numeric value is within a range.
#[derive(Debug, Clone)]
pub struct Range<T> {
    pub min: T,
    pub max: T,
}

impl<T> Range<T> {
    pub fn new(min: T, max: T) -> Self {
        Self { min, max }
    }
}

macro_rules! impl_range_rule {
    ($($t:ty),+) => {
        $(
            impl ValidationRule<$t> for Range<$t> {
                fn validate(&self, value: &$t) -> Option<FieldError> {
                    if *value < self.min || *value > self.max {
                        Some(FieldError::new(ValidationErrorKind::Range {
                            min: self.min.to_string(),
                            max: self.max.to_string(),
                            actual: value.to_string(),
                        }))
                    } else {
                        None
                    }
                }

                fn description(&self) -> String {
                    format!("value between {} and {}", self.min, self.max)
                }
            }

            impl ValidationRule<Option<$t>> for Range<$t> {
                fn validate(&self, value: &Option<$t>) -> Option<FieldError> {
                    match value {
                        Some(v) => <Range<$t> as ValidationRule<$t>>::validate(self, v),
                        None => None,
                    }
                }

                fn description(&self) -> String {
                    format!("value between {} and {}", self.min, self.max)
                }
            }
        )+
    };
}

impl_range_rule!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

// ═══════════════════════════════════════════════════════════════════════════════
// Format Validation Rules
// ═══════════════════════════════════════════════════════════════════════════════

/// Rule that validates email format.
#[derive(Debug, Clone, Default)]
pub struct Email;

impl ValidationRule<String> for Email {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.is_empty() || EMAIL_REGEX.is_match(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::InvalidEmail))
        }
    }

    fn description(&self) -> String {
        "valid email format".to_string()
    }
}

impl ValidationRule<Option<String>> for Email {
    fn validate(&self, value: &Option<String>) -> Option<FieldError> {
        match value {
            Some(s) => <Email as ValidationRule<String>>::validate(self, s),
            None => None,
        }
    }

    fn description(&self) -> String {
        "valid email format".to_string()
    }
}

/// Rule that validates URL format.
#[derive(Debug, Clone, Default)]
pub struct Url;

impl ValidationRule<String> for Url {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.is_empty() || URL_REGEX.is_match(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::InvalidUrl))
        }
    }

    fn description(&self) -> String {
        "valid URL format".to_string()
    }
}

impl ValidationRule<Option<String>> for Url {
    fn validate(&self, value: &Option<String>) -> Option<FieldError> {
        match value {
            Some(s) => <Url as ValidationRule<String>>::validate(self, s),
            None => None,
        }
    }

    fn description(&self) -> String {
        "valid URL format".to_string()
    }
}

/// Rule that validates UUID format.
#[derive(Debug, Clone, Default)]
pub struct Uuid;

impl ValidationRule<String> for Uuid {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.is_empty() || UUID_REGEX.is_match(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::InvalidUuid))
        }
    }

    fn description(&self) -> String {
        "valid UUID format".to_string()
    }
}

impl ValidationRule<Option<String>> for Uuid {
    fn validate(&self, value: &Option<String>) -> Option<FieldError> {
        match value {
            Some(s) => <Uuid as ValidationRule<String>>::validate(self, s),
            None => None,
        }
    }

    fn description(&self) -> String {
        "valid UUID format".to_string()
    }
}

/// Rule that validates alphanumeric characters only.
#[derive(Debug, Clone, Default)]
pub struct Alphanumeric;

impl ValidationRule<String> for Alphanumeric {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.is_empty() || ALPHANUMERIC_REGEX.is_match(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::Pattern {
                pattern: "alphanumeric".to_string(),
            }))
        }
    }

    fn description(&self) -> String {
        "alphanumeric characters only".to_string()
    }
}

/// Rule that validates slug format (lowercase, numbers, hyphens).
#[derive(Debug, Clone, Default)]
pub struct Slug;

impl ValidationRule<String> for Slug {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.is_empty() || SLUG_REGEX.is_match(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::Pattern {
                pattern: "slug (lowercase letters, numbers, hyphens)".to_string(),
            }))
        }
    }

    fn description(&self) -> String {
        "valid slug format".to_string()
    }
}

/// Rule that validates phone number format.
#[derive(Debug, Clone, Default)]
pub struct Phone;

impl ValidationRule<String> for Phone {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.is_empty() || PHONE_REGEX.is_match(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::Pattern {
                pattern: "phone number".to_string(),
            }))
        }
    }

    fn description(&self) -> String {
        "valid phone number format".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Custom Pattern Rule
// ═══════════════════════════════════════════════════════════════════════════════

/// Rule that validates against a custom regex pattern.
#[derive(Debug, Clone)]
pub struct Pattern {
    regex: Regex,
    description: String,
}

impl Pattern {
    /// Create a new pattern rule from a regex string.
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            regex: Regex::new(pattern)?,
            description: pattern.to_string(),
        })
    }

    /// Create a new pattern rule with a custom description.
    pub fn with_description(pattern: &str, description: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            regex: Regex::new(pattern)?,
            description: description.to_string(),
        })
    }
}

impl ValidationRule<String> for Pattern {
    fn validate(&self, value: &String) -> Option<FieldError> {
        if value.is_empty() || self.regex.is_match(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::Pattern {
                pattern: self.description.clone(),
            }))
        }
    }

    fn description(&self) -> String {
        format!("matches pattern: {}", self.description)
    }
}

impl ValidationRule<Option<String>> for Pattern {
    fn validate(&self, value: &Option<String>) -> Option<FieldError> {
        match value {
            Some(s) => <Pattern as ValidationRule<String>>::validate(self, s),
            None => None,
        }
    }

    fn description(&self) -> String {
        format!("matches pattern: {}", self.description)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Collection Rules
// ═══════════════════════════════════════════════════════════════════════════════

/// Rule that validates minimum number of items in a collection.
#[derive(Debug, Clone)]
pub struct MinItems(pub usize);

impl<T> ValidationRule<Vec<T>> for MinItems {
    fn validate(&self, value: &Vec<T>) -> Option<FieldError> {
        if value.len() < self.0 {
            Some(FieldError::new(ValidationErrorKind::MinItems {
                min: self.0,
                actual: value.len(),
            }))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        format!("minimum {} items", self.0)
    }
}

/// Rule that validates maximum number of items in a collection.
#[derive(Debug, Clone)]
pub struct MaxItems(pub usize);

impl<T> ValidationRule<Vec<T>> for MaxItems {
    fn validate(&self, value: &Vec<T>) -> Option<FieldError> {
        if value.len() > self.0 {
            Some(FieldError::new(ValidationErrorKind::MaxItems {
                max: self.0,
                actual: value.len(),
            }))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        format!("maximum {} items", self.0)
    }
}

/// Rule that validates no duplicate items in a collection.
#[derive(Debug, Clone, Default)]
pub struct UniqueItems;

impl<T: Eq + Hash> ValidationRule<Vec<T>> for UniqueItems {
    fn validate(&self, value: &Vec<T>) -> Option<FieldError> {
        let set: HashSet<&T> = value.iter().collect();
        if set.len() != value.len() {
            Some(FieldError::new(ValidationErrorKind::DuplicateItems))
        } else {
            None
        }
    }

    fn description(&self) -> String {
        "unique items only".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Set Membership Rule
// ═══════════════════════════════════════════════════════════════════════════════

/// Rule that validates a value is in a predefined set.
#[derive(Debug, Clone)]
pub struct OneOf<T> {
    allowed: Vec<T>,
}

impl<T> OneOf<T> {
    pub fn new(allowed: Vec<T>) -> Self {
        Self { allowed }
    }
}

impl<T: PartialEq + ToString> ValidationRule<T> for OneOf<T> {
    fn validate(&self, value: &T) -> Option<FieldError> {
        if self.allowed.contains(value) {
            None
        } else {
            Some(FieldError::new(ValidationErrorKind::NotInSet {
                allowed: self.allowed.iter().map(|v| v.to_string()).collect(),
            }))
        }
    }

    fn description(&self) -> String {
        format!("one of: {:?}", self.allowed.iter().map(|v| v.to_string()).collect::<Vec<_>>())
    }
}

impl<T: PartialEq + ToString> ValidationRule<Option<T>> for OneOf<T> {
    fn validate(&self, value: &Option<T>) -> Option<FieldError> {
        match value {
            Some(v) => <OneOf<T> as ValidationRule<T>>::validate(self, v),
            None => None,
        }
    }

    fn description(&self) -> String {
        format!("one of: {:?}", self.allowed.iter().map(|v| v.to_string()).collect::<Vec<_>>())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Convenience Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Validate that a string field is present and non-empty.
pub fn validate_required(field: &str, value: &str) -> ValidationResult<()> {
    if value.trim().is_empty() {
        let mut errors = ValidationErrors::new();
        errors.add_required(field);
        Err(errors)
    } else {
        Ok(())
    }
}

/// Validate that an optional field is present.
pub fn validate_required_option<T>(field: &str, value: &Option<T>) -> ValidationResult<()> {
    if value.is_none() {
        let mut errors = ValidationErrors::new();
        errors.add_required(field);
        Err(errors)
    } else {
        Ok(())
    }
}

/// Validate string length.
pub fn validate_length(field: &str, value: &str, min: Option<usize>, max: Option<usize>) -> ValidationResult<()> {
    let len = value.chars().count();
    let mut errors = ValidationErrors::new();

    if let Some(min) = min {
        if len < min {
            errors.add_error(field, ValidationErrorKind::MinLength { min, actual: len });
        }
    }

    if let Some(max) = max {
        if len > max {
            errors.add_error(field, ValidationErrorKind::MaxLength { max, actual: len });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate email format.
pub fn validate_email(field: &str, value: &str) -> ValidationResult<()> {
    if !value.is_empty() && !EMAIL_REGEX.is_match(value) {
        let mut errors = ValidationErrors::new();
        errors.add_error(field, ValidationErrorKind::InvalidEmail);
        Err(errors)
    } else {
        Ok(())
    }
}

/// Validate URL format.
pub fn validate_url(field: &str, value: &str) -> ValidationResult<()> {
    if !value.is_empty() && !URL_REGEX.is_match(value) {
        let mut errors = ValidationErrors::new();
        errors.add_error(field, ValidationErrorKind::InvalidUrl);
        Err(errors)
    } else {
        Ok(())
    }
}

/// Validate UUID format.
pub fn validate_uuid(field: &str, value: &str) -> ValidationResult<()> {
    if !value.is_empty() && !UUID_REGEX.is_match(value) {
        let mut errors = ValidationErrors::new();
        errors.add_error(field, ValidationErrorKind::InvalidUuid);
        Err(errors)
    } else {
        Ok(())
    }
}

/// Validate against a custom regex pattern.
pub fn validate_pattern(field: &str, value: &str, pattern: &Regex, description: &str) -> ValidationResult<()> {
    if !value.is_empty() && !pattern.is_match(value) {
        let mut errors = ValidationErrors::new();
        errors.add_error(field, ValidationErrorKind::Pattern {
            pattern: description.to_string(),
        });
        Err(errors)
    } else {
        Ok(())
    }
}

/// Validate numeric range.
pub fn validate_range<T: PartialOrd + ToString>(
    field: &str,
    value: T,
    min: Option<T>,
    max: Option<T>,
) -> ValidationResult<()> {
    let mut errors = ValidationErrors::new();

    if let Some(min_val) = &min {
        if value < *min_val {
            errors.add_error(field, ValidationErrorKind::MinValue {
                min: min_val.to_string(),
                actual: value.to_string(),
            });
        }
    }

    if let Some(max_val) = &max {
        if value > *max_val {
            errors.add_error(field, ValidationErrorKind::MaxValue {
                max: max_val.to_string(),
                actual: value.to_string(),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_string() {
        let rule = Required;
        assert!(rule.validate(&"hello".to_string()).is_none());
        assert!(rule.validate(&"".to_string()).is_some());
        assert!(rule.validate(&"   ".to_string()).is_some());
    }

    #[test]
    fn test_min_length() {
        let rule = MinLength(3);
        assert!(rule.validate(&"hello".to_string()).is_none());
        assert!(rule.validate(&"hi".to_string()).is_some());
    }

    #[test]
    fn test_max_length() {
        let rule = MaxLength(5);
        assert!(rule.validate(&"hello".to_string()).is_none());
        assert!(rule.validate(&"hello world".to_string()).is_some());
    }

    #[test]
    fn test_email() {
        let rule = Email;
        assert!(rule.validate(&"test@example.com".to_string()).is_none());
        assert!(rule.validate(&"invalid-email".to_string()).is_some());
        assert!(rule.validate(&"".to_string()).is_none()); // Empty is valid (use Required for presence)
    }

    #[test]
    fn test_url() {
        let rule = Url;
        assert!(rule.validate(&"https://example.com".to_string()).is_none());
        assert!(rule.validate(&"http://example.com/path?query=1".to_string()).is_none());
        assert!(rule.validate(&"not-a-url".to_string()).is_some());
    }

    #[test]
    fn test_uuid() {
        let rule = Uuid;
        assert!(rule.validate(&"550e8400-e29b-41d4-a716-446655440000".to_string()).is_none());
        assert!(rule.validate(&"not-a-uuid".to_string()).is_some());
    }

    #[test]
    fn test_numeric_range() {
        let rule = Range::new(0, 100);
        assert!(rule.validate(&50).is_none());
        assert!(rule.validate(&-1).is_some());
        assert!(rule.validate(&101).is_some());
    }

    #[test]
    fn test_custom_pattern() {
        let rule = Pattern::new(r"^[A-Z]{3}$").unwrap();
        assert!(rule.validate(&"ABC".to_string()).is_none());
        assert!(rule.validate(&"abc".to_string()).is_some());
        assert!(rule.validate(&"ABCD".to_string()).is_some());
    }

    #[test]
    fn test_one_of() {
        let rule = OneOf::new(vec!["red", "green", "blue"]);
        assert!(rule.validate(&"red").is_none());
        assert!(rule.validate(&"yellow").is_some());
    }

    #[test]
    fn test_unique_items() {
        let rule = UniqueItems;
        assert!(rule.validate(&vec![1, 2, 3]).is_none());
        assert!(rule.validate(&vec![1, 2, 2]).is_some());
    }

    #[test]
    fn test_convenience_functions() {
        assert!(validate_required("name", "John").is_ok());
        assert!(validate_required("name", "").is_err());

        assert!(validate_email("email", "test@example.com").is_ok());
        assert!(validate_email("email", "invalid").is_err());

        assert!(validate_length("name", "John", Some(1), Some(10)).is_ok());
        assert!(validate_length("name", "J", Some(2), None).is_err());
    }

    #[test]
    fn test_slug_valid() {
        let rule = Slug;
        assert!(rule.validate(&"hello-world".to_string()).is_none());
        assert!(rule.validate(&"test-123".to_string()).is_none());
    }

    #[test]
    fn test_slug_invalid() {
        let rule = Slug;
        assert!(rule.validate(&"Hello World".to_string()).is_some());
        assert!(rule.validate(&"invalid_slug!".to_string()).is_some());
    }

    #[test]
    fn test_slug_empty() {
        let rule = Slug;
        assert!(rule.validate(&"".to_string()).is_none());
    }

    #[test]
    fn test_phone_valid() {
        let rule = Phone;
        assert!(rule.validate(&"+1234567890".to_string()).is_none());
        assert!(rule.validate(&"+1-234-567-8901".to_string()).is_none());
    }

    #[test]
    fn test_phone_invalid() {
        let rule = Phone;
        assert!(rule.validate(&"not-a-phone".to_string()).is_some());
    }

    #[test]
    fn test_phone_empty() {
        let rule = Phone;
        assert!(rule.validate(&"".to_string()).is_none());
    }

    #[test]
    fn test_alphanumeric_valid() {
        let rule = Alphanumeric;
        assert!(rule.validate(&"abc123".to_string()).is_none());
        assert!(rule.validate(&"ABC".to_string()).is_none());
    }

    #[test]
    fn test_alphanumeric_invalid() {
        let rule = Alphanumeric;
        assert!(rule.validate(&"abc-123".to_string()).is_some());
        assert!(rule.validate(&"hello world".to_string()).is_some());
    }

    #[test]
    fn test_alphanumeric_empty() {
        let rule = Alphanumeric;
        assert!(rule.validate(&"".to_string()).is_none());
    }

    #[test]
    fn test_exact_length() {
        let rule = ExactLength(5);
        assert!(rule.validate(&"hello".to_string()).is_none());
        assert!(rule.validate(&"hi".to_string()).is_some());
        assert!(rule.validate(&"toolong".to_string()).is_some());
    }

    #[test]
    fn test_length_range() {
        let rule = LengthRange(2, 5);
        assert!(rule.validate(&"hi".to_string()).is_none());
        assert!(rule.validate(&"hello".to_string()).is_none());
        assert!(rule.validate(&"x".to_string()).is_some());
        assert!(rule.validate(&"toolong".to_string()).is_some());
    }

    #[test]
    fn test_required_option_some() {
        let rule = RequiredOption;
        let val: Option<String> = Some("hello".into());
        assert!(rule.validate(&val).is_none());
    }

    #[test]
    fn test_required_option_none() {
        let rule = RequiredOption;
        let val: Option<String> = None;
        assert!(rule.validate(&val).is_some());
    }

    #[test]
    fn test_required_string_nonempty() {
        let rule = RequiredString;
        assert!(rule.validate(&Some("hello".to_string())).is_none());
    }

    #[test]
    fn test_required_string_empty() {
        let rule = RequiredString;
        assert!(rule.validate(&Some("".to_string())).is_some());
    }

    #[test]
    fn test_required_string_whitespace() {
        let rule = RequiredString;
        assert!(rule.validate(&Some("   ".to_string())).is_some());
    }

    #[test]
    fn test_required_string_none() {
        let rule = RequiredString;
        assert!(rule.validate(&None).is_some());
    }

    #[test]
    fn test_required_vec() {
        let rule = Required;
        assert!(rule.validate(&vec![1, 2, 3]).is_none());
        assert!(rule.validate(&Vec::<i32>::new()).is_some());
    }

    #[test]
    fn test_min_i32() {
        let rule = Min(0);
        assert!(rule.validate(&5).is_none());
        assert!(rule.validate(&0).is_none());
        assert!(rule.validate(&-1).is_some());
    }

    #[test]
    fn test_max_i32() {
        let rule = Max(100);
        assert!(rule.validate(&50).is_none());
        assert!(rule.validate(&100).is_none());
        assert!(rule.validate(&101).is_some());
    }

    #[test]
    fn test_option_min_length() {
        let rule = MinLength(3);
        let val: Option<String> = Some("hello".into());
        assert!(rule.validate(&val).is_none());
        let val: Option<String> = Some("hi".into());
        assert!(rule.validate(&val).is_some());
        let val: Option<String> = None;
        assert!(rule.validate(&val).is_none());
    }

    #[test]
    fn test_option_max_length() {
        let rule = MaxLength(5);
        let val: Option<String> = Some("hello".into());
        assert!(rule.validate(&val).is_none());
        let val: Option<String> = Some("toolongvalue".into());
        assert!(rule.validate(&val).is_some());
        let val: Option<String> = None;
        assert!(rule.validate(&val).is_none());
    }

    #[test]
    fn test_option_email() {
        let rule = Email;
        assert!(rule.validate(&Some("test@example.com".to_string())).is_none());
        assert!(rule.validate(&Some("invalid".to_string())).is_some());
        assert!(rule.validate(&None::<String>).is_none());
    }

    #[test]
    fn test_option_url() {
        let rule = Url;
        assert!(rule.validate(&Some("https://example.com".to_string())).is_none());
        assert!(rule.validate(&Some("bad-url".to_string())).is_some());
        assert!(rule.validate(&None::<String>).is_none());
    }

    #[test]
    fn test_option_uuid() {
        let rule = Uuid;
        assert!(rule.validate(&Some("550e8400-e29b-41d4-a716-446655440000".to_string())).is_none());
        assert!(rule.validate(&Some("bad-uuid".to_string())).is_some());
        assert!(rule.validate(&None::<String>).is_none());
    }

    #[test]
    fn test_min_items() {
        let rule = MinItems(2);
        assert!(rule.validate(&vec![1, 2]).is_none());
        assert!(rule.validate(&vec![1]).is_some());
    }

    #[test]
    fn test_max_items() {
        let rule = MaxItems(3);
        assert!(rule.validate(&vec![1, 2, 3]).is_none());
        assert!(rule.validate(&vec![1, 2, 3, 4]).is_some());
    }

    #[test]
    fn test_one_of_with_option() {
        let rule = OneOf::new(vec!["a", "b", "c"]);
        assert!(rule.validate(&Some("a")).is_none());
        assert!(rule.validate(&Some("z")).is_some());
        assert!(rule.validate(&None::<&str>).is_none());
    }

    #[test]
    fn test_pattern_with_description() {
        let rule = Pattern::with_description(r"^\d{3}$", "three digits").unwrap();
        assert!(rule.validate(&"123".to_string()).is_none());
        assert!(rule.validate(&"12".to_string()).is_some());
        assert_eq!(rule.description(), "matches pattern: three digits");
    }

    #[test]
    fn test_validate_required_option() {
        assert!(validate_required_option("field", &Some(42)).is_ok());
        assert!(validate_required_option::<i32>("field", &None).is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("url", "https://example.com").is_ok());
        assert!(validate_url("url", "bad").is_err());
    }

    #[test]
    fn test_validate_uuid() {
        assert!(validate_uuid("id", "550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_uuid("id", "bad").is_err());
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range("val", 5, Some(0), Some(10)).is_ok());
        assert!(validate_range("val", -1, Some(0), Some(10)).is_err());
        assert!(validate_range("val", 11, Some(0), Some(10)).is_err());
    }

    #[test]
    fn test_validate_pattern() {
        let re = Regex::new(r"^\d+$").unwrap();
        assert!(validate_pattern("num", "123", &re, "digits only").is_ok());
        assert!(validate_pattern("num", "abc", &re, "digits only").is_err());
    }

    #[test]
    fn test_rule_descriptions() {
        assert!(!Required.description().is_empty());
        assert!(!MinLength(3).description().is_empty());
        assert!(!MaxLength(10).description().is_empty());
        assert!(!Email.description().is_empty());
        assert!(!Url.description().is_empty());
        assert!(!Uuid.description().is_empty());
        assert!(!Min(0i32).description().is_empty());
        assert!(!Max(100i32).description().is_empty());
        assert!(!Range::new(0, 100).description().is_empty());
        assert!(!MinItems(1).description().is_empty());
        assert!(!MaxItems(10).description().is_empty());
        assert!(!UniqueItems.description().is_empty());
    }
}
