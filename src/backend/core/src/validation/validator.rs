//! Validator trait and implementations for sync and async validation.
//!
//! This module provides:
//! - `Validate` trait for synchronous validation
//! - `ValidateAsync` trait for asynchronous validation
//! - `FieldValidator` for building field-level validation chains
//! - `RequestValidator` for validating entire request objects

use crate::validation::error::{FieldError, ValidationErrorKind, ValidationErrors, ValidationResult};
use crate::validation::rules::ValidationRule;
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

// ═══════════════════════════════════════════════════════════════════════════════
// Validate Trait (Synchronous)
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for types that can be validated synchronously.
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{Validate, ValidationErrors};
///
/// struct CreateUserRequest {
///     email: String,
///     name: String,
/// }
///
/// impl Validate for CreateUserRequest {
///     fn validate(&self) -> Result<(), ValidationErrors> {
///         let mut errors = ValidationErrors::new();
///
///         if self.email.is_empty() {
///             errors.add_required("email");
///         }
///         if self.name.is_empty() {
///             errors.add_required("name");
///         }
///
///         if errors.is_empty() {
///             Ok(())
///         } else {
///             Err(errors)
///         }
///     }
/// }
/// ```
pub trait Validate {
    /// Validate this object and return any validation errors.
    fn validate(&self) -> ValidationResult<()>;

    /// Check if this object is valid without returning detailed errors.
    fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Validate and return self if valid, otherwise return errors.
    fn validated(self) -> ValidationResult<Self>
    where
        Self: Sized,
    {
        self.validate()?;
        Ok(self)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ValidateAsync Trait (Asynchronous)
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for types that require asynchronous validation.
///
/// This is useful when validation requires:
/// - Database lookups (e.g., checking for unique email)
/// - External API calls
/// - Other async operations
///
/// # Example
///
/// ```rust,ignore
/// use apex_core::validation::{ValidateAsync, ValidationErrors};
/// use async_trait::async_trait;
///
/// struct CreateUserRequest {
///     email: String,
/// }
///
/// #[async_trait]
/// impl ValidateAsync for CreateUserRequest {
///     async fn validate_async(&self) -> Result<(), ValidationErrors> {
///         let mut errors = ValidationErrors::new();
///
///         // Check if email is unique (requires async DB call)
///         if email_exists_in_db(&self.email).await {
///             errors.add_with_message(
///                 "email",
///                 ValidationErrorKind::Custom { code: "email_taken".into() },
///                 "This email is already registered"
///             );
///         }
///
///         if errors.is_empty() {
///             Ok(())
///         } else {
///             Err(errors)
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait ValidateAsync {
    /// Validate this object asynchronously and return any validation errors.
    async fn validate_async(&self) -> ValidationResult<()>;

    /// Check if this object is valid asynchronously.
    async fn is_valid_async(&self) -> bool {
        self.validate_async().await.is_ok()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Combined Validation
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for types that support both sync and async validation.
///
/// Sync validation runs first, and if it passes, async validation runs.
#[async_trait]
pub trait ValidateFull: Validate + ValidateAsync {
    /// Run both sync and async validation.
    async fn validate_full(&self) -> ValidationResult<()> {
        // Run sync validation first
        self.validate()?;
        // Then run async validation
        self.validate_async().await
    }
}

// Blanket implementation for types that implement both traits
#[async_trait]
impl<T: Validate + ValidateAsync + Sync> ValidateFull for T {}

// ═══════════════════════════════════════════════════════════════════════════════
// Field Validator
// ═══════════════════════════════════════════════════════════════════════════════

/// A builder for validating a single field with multiple rules.
pub struct FieldValidator<'a, T> {
    field_name: &'a str,
    value: &'a T,
    errors: Vec<FieldError>,
    stop_on_first_error: bool,
}

impl<'a, T> FieldValidator<'a, T> {
    /// Create a new field validator.
    pub fn new(field_name: &'a str, value: &'a T) -> Self {
        Self {
            field_name,
            value,
            errors: Vec::new(),
            stop_on_first_error: false,
        }
    }

    /// Stop validation on the first error (fail-fast mode).
    pub fn stop_on_first(mut self) -> Self {
        self.stop_on_first_error = true;
        self
    }

    /// Apply a validation rule.
    pub fn rule<R: ValidationRule<T>>(mut self, rule: R) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if let Some(error) = rule.validate(self.value) {
            self.errors.push(error);
        }
        self
    }

    /// Apply a custom validation function.
    pub fn custom<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&T) -> Option<FieldError>,
    {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if let Some(error) = f(self.value) {
            self.errors.push(error);
        }
        self
    }

    /// Apply a custom validation with a simple boolean check.
    pub fn must<F>(mut self, predicate: F, error_kind: ValidationErrorKind) -> Self
    where
        F: FnOnce(&T) -> bool,
    {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if !predicate(self.value) {
            self.errors.push(FieldError::new(error_kind));
        }
        self
    }

    /// Apply a custom validation with a simple boolean check and custom message.
    pub fn must_with_message<F>(mut self, predicate: F, error_kind: ValidationErrorKind, message: &str) -> Self
    where
        F: FnOnce(&T) -> bool,
    {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if !predicate(self.value) {
            self.errors.push(FieldError::with_message(error_kind, message));
        }
        self
    }

    /// Get the field name.
    pub fn field_name(&self) -> &str {
        self.field_name
    }

    /// Check if validation passed (no errors).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the errors collected so far.
    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    /// Consume the validator and return the errors.
    pub fn into_errors(self) -> Vec<FieldError> {
        self.errors
    }

    /// Add the field's errors to a ValidationErrors collection.
    pub fn collect_into(self, errors: &mut ValidationErrors) {
        for error in self.errors {
            errors.add(self.field_name, error);
        }
    }

    /// Convert to a ValidationResult.
    pub fn result(self) -> ValidationResult<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            let mut validation_errors = ValidationErrors::new();
            for error in self.errors {
                validation_errors.add(self.field_name, error);
            }
            Err(validation_errors)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Async Field Validator
// ═══════════════════════════════════════════════════════════════════════════════

/// Type alias for async validation functions.
pub type AsyncValidationFn<'a, T> =
    Box<dyn FnOnce(&'a T) -> Pin<Box<dyn Future<Output = Option<FieldError>> + Send + 'a>> + Send + 'a>;

/// A builder for validating a single field with async rules.
pub struct AsyncFieldValidator<'a, T: Send + Sync> {
    field_name: &'a str,
    value: &'a T,
    sync_errors: Vec<FieldError>,
    async_validations: Vec<AsyncValidationFn<'a, T>>,
    stop_on_first_error: bool,
}

impl<'a, T: Send + Sync + 'a> AsyncFieldValidator<'a, T> {
    /// Create a new async field validator.
    pub fn new(field_name: &'a str, value: &'a T) -> Self {
        Self {
            field_name,
            value,
            sync_errors: Vec::new(),
            async_validations: Vec::new(),
            stop_on_first_error: false,
        }
    }

    /// Stop validation on the first error.
    pub fn stop_on_first(mut self) -> Self {
        self.stop_on_first_error = true;
        self
    }

    /// Apply a synchronous validation rule.
    pub fn rule<R: ValidationRule<T>>(mut self, rule: R) -> Self {
        if self.stop_on_first_error && !self.sync_errors.is_empty() {
            return self;
        }

        if let Some(error) = rule.validate(self.value) {
            self.sync_errors.push(error);
        }
        self
    }

    /// Apply an async validation function.
    pub fn async_rule<F, Fut>(mut self, f: F) -> Self
    where
        F: FnOnce(&'a T) -> Fut + Send + 'a,
        Fut: Future<Output = Option<FieldError>> + Send + 'a,
    {
        let boxed: AsyncValidationFn<'a, T> = Box::new(move |val| Box::pin(f(val)));
        self.async_validations.push(boxed);
        self
    }

    /// Run all validations and return the result.
    pub async fn validate(self) -> ValidationResult<()> {
        // First check sync errors
        if self.stop_on_first_error && !self.sync_errors.is_empty() {
            let mut errors = ValidationErrors::new();
            for error in self.sync_errors {
                errors.add(self.field_name, error);
            }
            return Err(errors);
        }

        // Run async validations
        let mut async_errors = Vec::new();
        for validation in self.async_validations {
            if self.stop_on_first_error && (!self.sync_errors.is_empty() || !async_errors.is_empty()) {
                break;
            }
            if let Some(error) = validation(self.value).await {
                async_errors.push(error);
            }
        }

        // Combine all errors
        let all_errors: Vec<_> = self.sync_errors.into_iter().chain(async_errors).collect();

        if all_errors.is_empty() {
            Ok(())
        } else {
            let mut errors = ValidationErrors::new();
            for error in all_errors {
                errors.add(self.field_name, error);
            }
            Err(errors)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Validator
// ═══════════════════════════════════════════════════════════════════════════════

/// A builder for validating entire request objects with multiple fields.
pub struct RequestValidator {
    errors: ValidationErrors,
    stop_on_first_error: bool,
}

impl RequestValidator {
    /// Create a new request validator.
    pub fn new() -> Self {
        Self {
            errors: ValidationErrors::new(),
            stop_on_first_error: false,
        }
    }

    /// Stop validation on the first error.
    pub fn stop_on_first(mut self) -> Self {
        self.stop_on_first_error = true;
        self
    }

    /// Validate a field and collect any errors.
    pub fn field<'a, T>(mut self, validator: FieldValidator<'a, T>) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        let field_name = validator.field_name().to_string();
        for error in validator.into_errors() {
            self.errors.add(&field_name, error);
        }
        self
    }

    /// Add a pre-built ValidationErrors (e.g., from nested validation).
    pub fn merge(mut self, other: ValidationErrors) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        self.errors.merge(other);
        self
    }

    /// Add errors with a prefix (for nested objects).
    pub fn merge_nested(mut self, prefix: &str, other: ValidationErrors) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        self.errors.merge_with_prefix(prefix, other);
        self
    }

    /// Validate a nested object.
    pub fn nested<V: Validate>(mut self, prefix: &str, value: &V) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if let Err(nested_errors) = value.validate() {
            self.errors.merge_with_prefix(prefix, nested_errors);
        }
        self
    }

    /// Validate items in a collection.
    pub fn items<V: Validate>(mut self, field: &str, items: &[V]) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        for (index, item) in items.iter().enumerate() {
            if self.stop_on_first_error && !self.errors.is_empty() {
                break;
            }

            if let Err(item_errors) = item.validate() {
                self.errors.merge_array_item(field, index, item_errors);
            }
        }
        self
    }

    /// Apply a custom validation function.
    pub fn custom<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut ValidationErrors),
    {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        f(&mut self.errors);
        self
    }

    /// Check if validation passed.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the collected errors.
    pub fn errors(&self) -> &ValidationErrors {
        &self.errors
    }

    /// Consume and return the errors.
    pub fn into_errors(self) -> ValidationErrors {
        self.errors
    }

    /// Convert to a ValidationResult.
    pub fn result(self) -> ValidationResult<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    /// Validate and return the original value if valid.
    pub fn validate<T>(self, value: T) -> ValidationResult<T> {
        if self.errors.is_empty() {
            Ok(value)
        } else {
            Err(self.errors)
        }
    }
}

impl Default for RequestValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Async Request Validator
// ═══════════════════════════════════════════════════════════════════════════════

/// A builder for validating entire request objects with async support.
pub struct AsyncRequestValidator {
    errors: ValidationErrors,
    stop_on_first_error: bool,
}

impl AsyncRequestValidator {
    /// Create a new async request validator.
    pub fn new() -> Self {
        Self {
            errors: ValidationErrors::new(),
            stop_on_first_error: false,
        }
    }

    /// Stop validation on the first error.
    pub fn stop_on_first(mut self) -> Self {
        self.stop_on_first_error = true;
        self
    }

    /// Validate a field synchronously.
    pub fn field<'a, T>(mut self, validator: FieldValidator<'a, T>) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        let field_name = validator.field_name().to_string();
        for error in validator.into_errors() {
            self.errors.add(&field_name, error);
        }
        self
    }

    /// Validate a field asynchronously.
    pub async fn field_async<'a, T: Send + Sync + 'a>(
        mut self,
        validator: AsyncFieldValidator<'a, T>,
    ) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if let Err(field_errors) = validator.validate().await {
            self.errors.merge(field_errors);
        }
        self
    }

    /// Add a pre-built ValidationErrors.
    pub fn merge(mut self, other: ValidationErrors) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        self.errors.merge(other);
        self
    }

    /// Validate a nested object synchronously.
    pub fn nested<V: Validate>(mut self, prefix: &str, value: &V) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if let Err(nested_errors) = value.validate() {
            self.errors.merge_with_prefix(prefix, nested_errors);
        }
        self
    }

    /// Validate a nested object asynchronously.
    pub async fn nested_async<V: ValidateAsync + Sync>(mut self, prefix: &str, value: &V) -> Self {
        if self.stop_on_first_error && !self.errors.is_empty() {
            return self;
        }

        if let Err(nested_errors) = value.validate_async().await {
            self.errors.merge_with_prefix(prefix, nested_errors);
        }
        self
    }

    /// Check if validation passed.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Convert to a ValidationResult.
    pub fn result(self) -> ValidationResult<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    /// Validate and return the original value if valid.
    pub fn validate<T>(self, value: T) -> ValidationResult<T> {
        if self.errors.is_empty() {
            Ok(value)
        } else {
            Err(self.errors)
        }
    }
}

impl Default for AsyncRequestValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Convenience Macros and Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Create a FieldValidator for a field.
pub fn validate_field<'a, T>(field_name: &'a str, value: &'a T) -> FieldValidator<'a, T> {
    FieldValidator::new(field_name, value)
}

/// Create an AsyncFieldValidator for a field.
pub fn validate_field_async<'a, T: Send + Sync>(
    field_name: &'a str,
    value: &'a T,
) -> AsyncFieldValidator<'a, T> {
    AsyncFieldValidator::new(field_name, value)
}

/// Create a new RequestValidator.
pub fn validate_request() -> RequestValidator {
    RequestValidator::new()
}

/// Create a new AsyncRequestValidator.
pub fn validate_request_async() -> AsyncRequestValidator {
    AsyncRequestValidator::new()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::rules::{Email, MaxLength, MinLength, Required};

    struct TestRequest {
        email: String,
        name: String,
        age: Option<i32>,
    }

    impl Validate for TestRequest {
        fn validate(&self) -> ValidationResult<()> {
            validate_request()
                .field(
                    validate_field("email", &self.email)
                        .rule(Required)
                        .rule(Email),
                )
                .field(
                    validate_field("name", &self.name)
                        .rule(Required)
                        .rule(MinLength(2))
                        .rule(MaxLength(100)),
                )
                .result()
        }
    }

    #[test]
    fn test_validate_trait() {
        let valid_request = TestRequest {
            email: "test@example.com".to_string(),
            name: "John Doe".to_string(),
            age: Some(25),
        };
        assert!(valid_request.validate().is_ok());

        let invalid_request = TestRequest {
            email: "invalid".to_string(),
            name: "".to_string(),
            age: None,
        };
        let result = invalid_request.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors.has_errors("email"));
        assert!(errors.has_errors("name"));
    }

    #[test]
    fn test_field_validator() {
        let email = "test@example.com".to_string();
        let validator = validate_field("email", &email)
            .rule(Required)
            .rule(Email);
        assert!(validator.is_valid());

        let bad_email = "invalid".to_string();
        let validator = validate_field("email", &bad_email)
            .rule(Required)
            .rule(Email);
        assert!(!validator.is_valid());
    }

    #[test]
    fn test_field_validator_custom() {
        let value = 42i32;
        let validator = validate_field("value", &value)
            .must(|v| *v > 0, ValidationErrorKind::MinValue {
                min: "0".to_string(),
                actual: "".to_string(),
            })
            .must(|v| *v % 2 == 0, ValidationErrorKind::Custom {
                code: "must_be_even".to_string(),
            });
        assert!(validator.is_valid());

        let odd_value = 41i32;
        let validator = validate_field("value", &odd_value)
            .must(|v| *v % 2 == 0, ValidationErrorKind::Custom {
                code: "must_be_even".to_string(),
            });
        assert!(!validator.is_valid());
    }

    #[test]
    fn test_request_validator_nested() {
        struct Address {
            street: String,
            city: String,
        }

        impl Validate for Address {
            fn validate(&self) -> ValidationResult<()> {
                validate_request()
                    .field(validate_field("street", &self.street).rule(Required))
                    .field(validate_field("city", &self.city).rule(Required))
                    .result()
            }
        }

        struct User {
            name: String,
            address: Address,
        }

        impl Validate for User {
            fn validate(&self) -> ValidationResult<()> {
                validate_request()
                    .field(validate_field("name", &self.name).rule(Required))
                    .nested("address", &self.address)
                    .result()
            }
        }

        let invalid_user = User {
            name: "John".to_string(),
            address: Address {
                street: "".to_string(),
                city: "".to_string(),
            },
        };

        let result = invalid_user.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors.has_errors("address.street"));
        assert!(errors.has_errors("address.city"));
    }

    #[test]
    fn test_stop_on_first() {
        let value = "".to_string();
        let validator = validate_field("email", &value)
            .stop_on_first()
            .rule(Required)
            .rule(Email);

        // Should only have the Required error, not the Email error
        let errors = validator.into_errors();
        assert_eq!(errors.len(), 1);
    }

    #[tokio::test]
    async fn test_async_field_validator() {
        let email = "test@example.com".to_string();

        let validator = validate_field_async("email", &email)
            .rule(Required)
            .rule(Email)
            .async_rule(|_value| async {
                // Simulate async validation (e.g., checking if email exists in DB)
                None // No error
            });

        let result = validator.validate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_field_validator_with_error() {
        let email = "taken@example.com".to_string();

        let validator = validate_field_async("email", &email)
            .rule(Required)
            .rule(Email)
            .async_rule(|_value| async {
                // Simulate async validation that finds the email is taken
                Some(FieldError::with_message(
                    ValidationErrorKind::Custom { code: "email_taken".to_string() },
                    "This email is already registered",
                ))
            });

        let result = validator.validate().await;
        assert!(result.is_err());
    }
}
