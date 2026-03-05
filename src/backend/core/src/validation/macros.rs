//! Validation helper macros for ergonomic validation code.
//!
//! This module provides macros for:
//! - Concise field validation
//! - Request validation builders
//! - Error accumulation patterns
//!
//! Note: Full derive macros would require a separate proc-macro crate.
//! These are declarative macro helpers that work within a single crate.

/// Validate multiple fields and collect errors.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{validate_fields, ValidationErrors};
///
/// let mut errors = ValidationErrors::new();
/// let email = "invalid";
/// let name = "";
///
/// validate_fields!(errors, {
///     "email" => {
///         required: email,
///         email: email,
///     },
///     "name" => {
///         required: name,
///         min_length: (name, 2),
///     },
/// });
/// ```
#[macro_export]
macro_rules! validate_fields {
    ($errors:expr, {
        $($field:literal => {
            $($rule:ident $(: $args:tt)?),* $(,)?
        }),* $(,)?
    }) => {
        $(
            $crate::validate_field_rules!($errors, $field, $($rule $(: $args)?),*);
        )*
    };
}

/// Internal macro for validating a single field with multiple rules.
#[macro_export]
macro_rules! validate_field_rules {
    ($errors:expr, $field:literal, $($rule:ident $(: $args:tt)?),*) => {
        $(
            $crate::validate_single_rule!($errors, $field, $rule $(, $args)?);
        )*
    };
}

/// Internal macro for applying a single validation rule.
#[macro_export]
macro_rules! validate_single_rule {
    // Required (string)
    ($errors:expr, $field:literal, required, $value:expr) => {
        if $value.trim().is_empty() {
            $errors.add_required($field);
        }
    };

    // Required (Option)
    ($errors:expr, $field:literal, required_option, $value:expr) => {
        if $value.is_none() {
            $errors.add_required($field);
        }
    };

    // Min length
    ($errors:expr, $field:literal, min_length, ($value:expr, $min:expr)) => {
        let len = $value.chars().count();
        if len < $min {
            $errors.add_error($field, $crate::validation::error::ValidationErrorKind::MinLength {
                min: $min,
                actual: len,
            });
        }
    };

    // Max length
    ($errors:expr, $field:literal, max_length, ($value:expr, $max:expr)) => {
        let len = $value.chars().count();
        if len > $max {
            $errors.add_error($field, $crate::validation::error::ValidationErrorKind::MaxLength {
                max: $max,
                actual: len,
            });
        }
    };

    // Email
    ($errors:expr, $field:literal, email, $value:expr) => {
        if let Err(e) = $crate::validation::rules::validate_email($field, $value) {
            $errors.merge(e);
        }
    };

    // URL
    ($errors:expr, $field:literal, url, $value:expr) => {
        if let Err(e) = $crate::validation::rules::validate_url($field, $value) {
            $errors.merge(e);
        }
    };

    // UUID
    ($errors:expr, $field:literal, uuid, $value:expr) => {
        if let Err(e) = $crate::validation::rules::validate_uuid($field, $value) {
            $errors.merge(e);
        }
    };

    // Min value
    ($errors:expr, $field:literal, min_value, ($value:expr, $min:expr)) => {
        if $value < $min {
            $errors.add_error($field, $crate::validation::error::ValidationErrorKind::MinValue {
                min: $min.to_string(),
                actual: $value.to_string(),
            });
        }
    };

    // Max value
    ($errors:expr, $field:literal, max_value, ($value:expr, $max:expr)) => {
        if $value > $max {
            $errors.add_error($field, $crate::validation::error::ValidationErrorKind::MaxValue {
                max: $max.to_string(),
                actual: $value.to_string(),
            });
        }
    };

    // Range (numeric)
    ($errors:expr, $field:literal, range, ($value:expr, $min:expr, $max:expr)) => {
        if $value < $min || $value > $max {
            $errors.add_error($field, $crate::validation::error::ValidationErrorKind::Range {
                min: $min.to_string(),
                max: $max.to_string(),
                actual: $value.to_string(),
            });
        }
    };
}

/// Create a validation error for a field quickly.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{validation_error, ValidationErrors};
///
/// let errors = validation_error!("email", required);
/// let errors = validation_error!("age", min_value: 18, 5);
/// ```
#[macro_export]
macro_rules! validation_error {
    ($field:literal, required) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_required($field);
        errors
    }};

    ($field:literal, min_length: $min:expr, $actual:expr) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_error($field, $crate::validation::error::ValidationErrorKind::MinLength {
            min: $min,
            actual: $actual,
        });
        errors
    }};

    ($field:literal, max_length: $max:expr, $actual:expr) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_error($field, $crate::validation::error::ValidationErrorKind::MaxLength {
            max: $max,
            actual: $actual,
        });
        errors
    }};

    ($field:literal, invalid_email) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_error($field, $crate::validation::error::ValidationErrorKind::InvalidEmail);
        errors
    }};

    ($field:literal, invalid_url) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_error($field, $crate::validation::error::ValidationErrorKind::InvalidUrl);
        errors
    }};

    ($field:literal, invalid_uuid) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_error($field, $crate::validation::error::ValidationErrorKind::InvalidUuid);
        errors
    }};

    ($field:literal, min_value: $min:expr, $actual:expr) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_error($field, $crate::validation::error::ValidationErrorKind::MinValue {
            min: $min.to_string(),
            actual: $actual.to_string(),
        });
        errors
    }};

    ($field:literal, max_value: $max:expr, $actual:expr) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_error($field, $crate::validation::error::ValidationErrorKind::MaxValue {
            max: $max.to_string(),
            actual: $actual.to_string(),
        });
        errors
    }};

    ($field:literal, custom: $code:expr, $message:expr) => {{
        let mut errors = $crate::validation::error::ValidationErrors::new();
        errors.add_with_message(
            $field,
            $crate::validation::error::ValidationErrorKind::Custom { code: $code.to_string() },
            $message,
        );
        errors
    }};
}

/// Ensure a condition is met, otherwise return a validation error.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{ensure, ValidationResult};
///
/// fn validate_age(age: i32) -> ValidationResult<()> {
///     ensure!(age >= 18, "age", "must be at least 18 years old");
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! ensure {
    ($condition:expr, $field:literal, $message:expr) => {
        if !$condition {
            let mut errors = $crate::validation::error::ValidationErrors::new();
            errors.add_with_message(
                $field,
                $crate::validation::error::ValidationErrorKind::Custom {
                    code: "validation_failed".to_string(),
                },
                $message,
            );
            return Err(errors);
        }
    };

    ($condition:expr, $field:literal, $code:expr, $message:expr) => {
        if !$condition {
            let mut errors = $crate::validation::error::ValidationErrors::new();
            errors.add_with_message(
                $field,
                $crate::validation::error::ValidationErrorKind::Custom {
                    code: $code.to_string(),
                },
                $message,
            );
            return Err(errors);
        }
    };
}

/// Bail early if errors exist.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{bail_if_errors, ValidationErrors, ValidationResult};
///
/// fn validate_request() -> ValidationResult<()> {
///     let mut errors = ValidationErrors::new();
///
///     // Collect some errors...
///     errors.add_required("email");
///
///     bail_if_errors!(errors); // Returns early if errors exist
///
///     // Continue with more validation...
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! bail_if_errors {
    ($errors:expr) => {
        if !$errors.is_empty() {
            return Err($errors);
        }
    };
}

/// Create a simple validator implementation.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{impl_validate, Validate, ValidationErrors, ValidationResult};
///
/// struct CreateUserRequest {
///     email: String,
///     name: String,
/// }
///
/// impl_validate!(CreateUserRequest, |self| {
///     let mut errors = ValidationErrors::new();
///
///     if self.email.is_empty() {
///         errors.add_required("email");
///     }
///     if self.name.is_empty() {
///         errors.add_required("name");
///     }
///
///     if errors.is_empty() {
///         Ok(())
///     } else {
///         Err(errors)
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_validate {
    ($type:ty, |$self:ident| $body:expr) => {
        impl $crate::validation::validator::Validate for $type {
            fn validate(&$self) -> $crate::validation::error::ValidationResult<()> {
                $body
            }
        }
    };
}

/// Create an async validator implementation.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{impl_validate_async, ValidateAsync, ValidationErrors, ValidationResult};
/// use async_trait::async_trait;
///
/// struct CreateUserRequest {
///     email: String,
/// }
///
/// impl_validate_async!(CreateUserRequest, |self| async {
///     let mut errors = ValidationErrors::new();
///
///     // Async validation (e.g., check if email exists in DB)
///     if email_exists(&self.email).await {
///         errors.add_with_message("email", ValidationErrorKind::Custom { code: "taken".into() }, "Email already exists");
///     }
///
///     if errors.is_empty() {
///         Ok(())
///     } else {
///         Err(errors)
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_validate_async {
    ($type:ty, |$self:ident| async $body:expr) => {
        #[async_trait::async_trait]
        impl $crate::validation::validator::ValidateAsync for $type {
            async fn validate_async(&$self) -> $crate::validation::error::ValidationResult<()> {
                $body
            }
        }
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helper Types for Macro Usage
// ═══════════════════════════════════════════════════════════════════════════════

/// A helper struct for building validation in a more declarative style.
///
/// This can be used with macros or directly for complex validation scenarios.
pub struct ValidationBuilder<T> {
    value: T,
    errors: crate::validation::error::ValidationErrors,
}

impl<T> ValidationBuilder<T> {
    /// Create a new validation builder for a value.
    pub fn new(value: T) -> Self {
        Self {
            value,
            errors: crate::validation::error::ValidationErrors::new(),
        }
    }

    /// Get a reference to the value being validated.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Get mutable access to the errors.
    pub fn errors_mut(&mut self) -> &mut crate::validation::error::ValidationErrors {
        &mut self.errors
    }

    /// Check if validation has passed so far.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Finish validation and return the result.
    pub fn finish(self) -> crate::validation::error::ValidationResult<T> {
        if self.errors.is_empty() {
            Ok(self.value)
        } else {
            Err(self.errors)
        }
    }

    /// Apply a validation function.
    pub fn validate<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&T, &mut crate::validation::error::ValidationErrors),
    {
        f(&self.value, &mut self.errors);
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use crate::validation::error::{ValidationErrorKind, ValidationErrors};

    #[test]
    fn test_validation_error_macro_required() {
        let errors = validation_error!("email", required);
        assert!(errors.has_errors("email"));
        assert_eq!(errors.error_count(), 1);
    }

    #[test]
    fn test_validation_error_macro_custom() {
        let errors = validation_error!("field", custom: "my_code", "My custom message");
        assert!(errors.has_errors("field"));
        let field_errors = errors.get("field").unwrap();
        assert_eq!(field_errors[0].message, "My custom message");
    }

    #[test]
    fn test_bail_if_errors_macro() {
        fn validate_with_bail() -> crate::validation::error::ValidationResult<()> {
            let mut errors = ValidationErrors::new();
            errors.add_required("field1");
            bail_if_errors!(errors);
            // This should not be reached
            errors.add_required("field2");
            Ok(())
        }

        let result = validate_with_bail();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.has_errors("field1"));
        assert!(!errors.has_errors("field2")); // Never reached
    }

    #[test]
    fn test_ensure_macro() {
        fn validate_age(age: i32) -> crate::validation::error::ValidationResult<()> {
            ensure!(age >= 18, "age", "must be at least 18 years old");
            Ok(())
        }

        assert!(validate_age(20).is_ok());
        assert!(validate_age(15).is_err());
    }

    #[test]
    fn test_validation_builder() {
        use super::ValidationBuilder;

        let result = ValidationBuilder::new("test@example.com".to_string())
            .validate(|value, errors| {
                if value.is_empty() {
                    errors.add_required("email");
                }
            })
            .validate(|value, errors| {
                if !value.contains('@') {
                    errors.add_error("email", ValidationErrorKind::InvalidEmail);
                }
            })
            .finish();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test@example.com");

        let result = ValidationBuilder::new("invalid".to_string())
            .validate(|value, errors| {
                if !value.contains('@') {
                    errors.add_error("email", ValidationErrorKind::InvalidEmail);
                }
            })
            .finish();

        assert!(result.is_err());
    }
}
