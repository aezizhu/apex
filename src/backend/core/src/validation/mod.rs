//! Comprehensive request validation framework for Apex Core.
//!
//! This module provides a production-grade validation system with:
//!
//! - **Validation Rules**: Pre-built rules for common validation scenarios
//!   - Required fields
//!   - String length constraints (min, max, exact, range)
//!   - Numeric range constraints
//!   - Format validation (email, URL, UUID, phone, slug)
//!   - Custom regex patterns
//!   - Collection constraints (min/max items, unique items)
//!   - Set membership validation
//!
//! - **Validators**: Traits and builders for sync and async validation
//!   - `Validate` trait for synchronous validation
//!   - `ValidateAsync` trait for asynchronous validation (DB lookups, API calls)
//!   - `ValidateFull` trait for combined sync + async validation
//!   - `FieldValidator` for chaining rules on a single field
//!   - `RequestValidator` for validating entire request objects
//!
//! - **Error Handling**: Comprehensive field-level error tracking
//!   - Nested field paths (e.g., "user.address.street")
//!   - Array index support (e.g., "items[0].name")
//!   - Multiple errors per field
//!   - Serializable error responses for API integration
//!
//! - **Macros**: Helper macros for ergonomic validation code
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use apex_core::validation::{
//!     Validate, ValidationErrors, ValidationResult,
//!     validate_field, validate_request,
//!     Required, Email, MinLength, MaxLength,
//! };
//!
//! struct CreateUserRequest {
//!     email: String,
//!     name: String,
//!     age: Option<i32>,
//! }
//!
//! impl Validate for CreateUserRequest {
//!     fn validate(&self) -> ValidationResult<()> {
//!         validate_request()
//!             .field(
//!                 validate_field("email", &self.email)
//!                     .rule(Required)
//!                     .rule(Email)
//!                     .rule(MaxLength(255))
//!             )
//!             .field(
//!                 validate_field("name", &self.name)
//!                     .rule(Required)
//!                     .rule(MinLength(2))
//!                     .rule(MaxLength(100))
//!             )
//!             .result()
//!     }
//! }
//!
//! // Usage
//! let request = CreateUserRequest {
//!     email: "user@example.com".to_string(),
//!     name: "John Doe".to_string(),
//!     age: Some(25),
//! };
//!
//! match request.validate() {
//!     Ok(()) => println!("Valid!"),
//!     Err(errors) => {
//!         for (field, field_errors) in errors.iter() {
//!             for error in field_errors {
//!                 println!("{}: {}", field, error.message);
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! # Async Validation
//!
//! For validations that require async operations (database lookups, API calls):
//!
//! ```rust,ignore
//! use apex_core::validation::{ValidateAsync, ValidationErrors, ValidationResult};
//! use async_trait::async_trait;
//!
//! struct CreateUserRequest {
//!     email: String,
//! }
//!
//! #[async_trait]
//! impl ValidateAsync for CreateUserRequest {
//!     async fn validate_async(&self) -> ValidationResult<()> {
//!         let mut errors = ValidationErrors::new();
//!
//!         // Check if email is unique in database
//!         if email_exists_in_db(&self.email).await {
//!             errors.add_with_message(
//!                 "email",
//!                 ValidationErrorKind::Custom { code: "email_taken".into() },
//!                 "This email is already registered"
//!             );
//!         }
//!
//!         if errors.is_empty() {
//!             Ok(())
//!         } else {
//!             Err(errors)
//!         }
//!     }
//! }
//! ```
//!
//! # Nested Validation
//!
//! Validate nested objects with proper field path tracking:
//!
//! ```rust,ignore
//! use apex_core::validation::{Validate, validate_field, validate_request, Required};
//!
//! struct Address {
//!     street: String,
//!     city: String,
//! }
//!
//! impl Validate for Address {
//!     fn validate(&self) -> ValidationResult<()> {
//!         validate_request()
//!             .field(validate_field("street", &self.street).rule(Required))
//!             .field(validate_field("city", &self.city).rule(Required))
//!             .result()
//!     }
//! }
//!
//! struct User {
//!     name: String,
//!     address: Address,
//! }
//!
//! impl Validate for User {
//!     fn validate(&self) -> ValidationResult<()> {
//!         validate_request()
//!             .field(validate_field("name", &self.name).rule(Required))
//!             .nested("address", &self.address)
//!             .result()
//!     }
//! }
//!
//! // Errors will have paths like "address.street", "address.city"
//! ```
//!
//! # Custom Validation Rules
//!
//! Create custom validation rules:
//!
//! ```rust,ignore
//! use apex_core::validation::{ValidationRule, FieldError, ValidationErrorKind};
//!
//! struct PasswordStrength {
//!     min_uppercase: usize,
//!     min_digits: usize,
//! }
//!
//! impl ValidationRule<String> for PasswordStrength {
//!     fn validate(&self, value: &String) -> Option<FieldError> {
//!         let uppercase_count = value.chars().filter(|c| c.is_uppercase()).count();
//!         let digit_count = value.chars().filter(|c| c.is_numeric()).count();
//!
//!         if uppercase_count < self.min_uppercase || digit_count < self.min_digits {
//!             Some(FieldError::with_message(
//!                 ValidationErrorKind::Custom { code: "weak_password".into() },
//!                 format!(
//!                     "Password must have at least {} uppercase letters and {} digits",
//!                     self.min_uppercase, self.min_digits
//!                 )
//!             ))
//!         } else {
//!             None
//!         }
//!     }
//!
//!     fn description(&self) -> String {
//!         "strong password".to_string()
//!     }
//! }
//! ```

pub mod error;
pub mod macros;
pub mod rules;
pub mod validator;

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports
// ═══════════════════════════════════════════════════════════════════════════════

// Error types
pub use error::{
    FieldError, OptionExt, ValidationErrorBuilder, ValidationErrorKind, ValidationErrors,
    ValidationResult,
};

// Validation rules
pub use rules::{
    // Core rules
    Alphanumeric,
    Email,
    ExactLength,
    LengthRange,
    Max,
    MaxItems,
    MaxLength,
    Min,
    MinItems,
    MinLength,
    OneOf,
    Pattern,
    Phone,
    Range,
    Required,
    RequiredOption,
    RequiredString,
    Slug,
    UniqueItems,
    Url,
    Uuid,
    ValidationRule,
    // Convenience functions
    validate_email,
    validate_length,
    validate_pattern,
    validate_range,
    validate_required,
    validate_required_option,
    validate_url,
    validate_uuid,
};

// Validators
pub use validator::{
    AsyncFieldValidator, AsyncRequestValidator, FieldValidator, RequestValidator, Validate,
    ValidateAsync, ValidateFull, validate_field, validate_field_async, validate_request,
    validate_request_async,
};

// Macros helper
pub use macros::ValidationBuilder;

// ═══════════════════════════════════════════════════════════════════════════════
// Prelude
// ═══════════════════════════════════════════════════════════════════════════════

/// Common imports for validation.
pub mod prelude {
    pub use super::{
        // Error types
        FieldError,
        ValidationErrorKind,
        ValidationErrors,
        ValidationResult,
        // Traits
        Validate,
        ValidateAsync,
        ValidateFull,
        ValidationRule,
        // Builders
        FieldValidator,
        RequestValidator,
        // Common rules
        Email,
        Max,
        MaxLength,
        Min,
        MinLength,
        OneOf,
        Pattern,
        Range,
        Required,
        Url,
        Uuid,
        // Helper functions
        validate_field,
        validate_request,
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// Integration with Apex Error System
// ═══════════════════════════════════════════════════════════════════════════════

use crate::error::ApexError;

impl From<ValidationErrors> for ApexError {
    fn from(errors: ValidationErrors) -> Self {
        let message = if let Some((field, error)) = errors.first_error() {
            format!("Validation failed: {} - {}", field, error.message)
        } else {
            "Validation failed".to_string()
        };

        ApexError::validation(message)
            .with_context("field_errors", errors.to_message_map())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    struct TestUser {
        email: String,
        name: String,
        age: i32,
        tags: Vec<String>,
    }

    impl Validate for TestUser {
        fn validate(&self) -> ValidationResult<()> {
            validate_request()
                .field(
                    validate_field("email", &self.email)
                        .rule(Required)
                        .rule(Email)
                        .rule(MaxLength(255)),
                )
                .field(
                    validate_field("name", &self.name)
                        .rule(Required)
                        .rule(MinLength(2))
                        .rule(MaxLength(100)),
                )
                .field(validate_field("age", &self.age).rule(Range::new(0, 150)))
                .field(
                    validate_field("tags", &self.tags)
                        .rule(MaxItems(10))
                        .rule(UniqueItems),
                )
                .result()
        }
    }

    #[test]
    fn test_valid_user() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "John Doe".to_string(),
            age: 25,
            tags: vec!["rust".to_string(), "web".to_string()],
        };

        assert!(user.validate().is_ok());
        assert!(user.is_valid());
    }

    #[test]
    fn test_invalid_user() {
        let user = TestUser {
            email: "invalid-email".to_string(),
            name: "J".to_string(), // Too short
            age: 200,             // Out of range
            tags: vec!["a".to_string(), "a".to_string()], // Duplicates
        };

        let result = user.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors.has_errors("email"));
        assert!(errors.has_errors("name"));
        assert!(errors.has_errors("age"));
        assert!(errors.has_errors("tags"));
    }

    #[test]
    fn test_validation_errors_to_apex_error() {
        let mut errors = ValidationErrors::new();
        errors.add_required("email");
        errors.add_error("name", ValidationErrorKind::MinLength { min: 2, actual: 1 });

        let apex_error: ApexError = errors.into();
        assert_eq!(apex_error.code(), ErrorCode::ValidationError);
    }

    #[test]
    fn test_prelude_imports() {
        use super::prelude::*;

        // Ensure all prelude items are accessible
        let _ = ValidationErrors::new();
        let _ = Required;
        let _ = Email;
    }

    #[test]
    fn test_valid_user_with_boundary_values() {
        let user = TestUser {
            email: "a@b.co".to_string(),
            name: "Jo".to_string(), // Exactly min length
            age: 0,                 // Boundary: min of range
            tags: vec![],
        };
        // name meets MinLength(2), age is at range min 0
        assert!(user.validate().is_ok());
    }

    #[test]
    fn test_user_age_at_max_boundary() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "Jane".to_string(),
            age: 150, // Boundary: max of range
            tags: vec![],
        };
        assert!(user.validate().is_ok());
    }

    #[test]
    fn test_user_age_exceeds_max() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "Jane".to_string(),
            age: 151,
            tags: vec![],
        };
        let result = user.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().has_errors("age"));
    }

    #[test]
    fn test_user_negative_age() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "Jane".to_string(),
            age: -1,
            tags: vec![],
        };
        let result = user.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().has_errors("age"));
    }

    #[test]
    fn test_user_empty_email_required() {
        let user = TestUser {
            email: "".to_string(),
            name: "Jane".to_string(),
            age: 25,
            tags: vec![],
        };
        let result = user.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().has_errors("email"));
    }

    #[test]
    fn test_user_name_exactly_at_max_length() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "a".repeat(100), // Exactly max length
            age: 30,
            tags: vec![],
        };
        assert!(user.validate().is_ok());
    }

    #[test]
    fn test_user_name_exceeds_max_length() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "a".repeat(101), // One over max
            age: 30,
            tags: vec![],
        };
        let result = user.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().has_errors("name"));
    }

    #[test]
    fn test_user_tags_at_max_items() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "Jane".to_string(),
            age: 25,
            tags: (1..=10).map(|i| format!("tag{}", i)).collect(),
        };
        assert!(user.validate().is_ok());
    }

    #[test]
    fn test_user_tags_exceed_max_items() {
        let user = TestUser {
            email: "user@example.com".to_string(),
            name: "Jane".to_string(),
            age: 25,
            tags: (1..=11).map(|i| format!("tag{}", i)).collect(),
        };
        let result = user.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().has_errors("tags"));
    }

    #[test]
    fn test_validation_errors_to_apex_error_message_format() {
        let mut errors = ValidationErrors::new();
        errors.add_required("username");

        let apex_error: ApexError = errors.into();
        let msg = apex_error.to_string();
        assert!(msg.contains("Validation failed"));
        assert!(msg.contains("username"));
    }

    #[test]
    fn test_is_valid_trait_method() {
        let valid_user = TestUser {
            email: "test@test.com".to_string(),
            name: "Bob".to_string(),
            age: 50,
            tags: vec![],
        };
        assert!(valid_user.is_valid());

        let invalid_user = TestUser {
            email: "bad".to_string(),
            name: "X".to_string(),
            age: 999,
            tags: vec![],
        };
        assert!(!invalid_user.is_valid());
    }
}
