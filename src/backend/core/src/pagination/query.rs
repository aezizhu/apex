//! Query parameter parsing for pagination.
//!
//! This module provides:
//! - Unified pagination query parameters
//! - Support for both offset and cursor modes
//! - Validation and defaults
//! - Axum extractor integration

use serde::{Deserialize, Serialize};

use super::cursor::{Cursor, CursorBuilder, CursorPagination, SortDirection, SortField};
use super::offset::OffsetPagination;
use crate::error::{ApexError, ErrorCode};

// ═══════════════════════════════════════════════════════════════════════════════
// Pagination Mode
// ═══════════════════════════════════════════════════════════════════════════════

/// The pagination mode to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PaginationMode {
    /// Offset-based pagination (page/per_page).
    #[default]
    Offset,
    /// Cursor-based pagination (after/before cursors).
    Cursor,
}

impl std::fmt::Display for PaginationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offset => write!(f, "offset"),
            Self::Cursor => write!(f, "cursor"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pagination Query
// ═══════════════════════════════════════════════════════════════════════════════

/// Pagination query parameters supporting both offset and cursor modes.
///
/// This struct can be used as an Axum query parameter extractor.
///
/// # Offset Mode Parameters
/// - `page`: Page number (1-indexed, default: 1)
/// - `per_page` or `limit`: Items per page (default: 20, max: 100)
///
/// # Cursor Mode Parameters
/// - `after`: Cursor for forward pagination
/// - `before`: Cursor for backward pagination
/// - `first`: Number of items for forward pagination
/// - `last`: Number of items for backward pagination
///
/// # Common Parameters
/// - `sort_by`: Field to sort by
/// - `sort_order`: Sort direction (asc/desc)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PaginationQuery {
    // ─────────────────────────────────────────────────────────────────────────
    // Offset-based parameters
    // ─────────────────────────────────────────────────────────────────────────
    /// Page number (1-indexed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u64>,

    /// Number of items per page (alias: limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u64>,

    /// Alias for per_page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,

    // ─────────────────────────────────────────────────────────────────────────
    // Cursor-based parameters
    // ─────────────────────────────────────────────────────────────────────────
    /// Cursor for forward pagination.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,

    /// Cursor for backward pagination.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,

    /// Number of items for forward pagination (cursor mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<u64>,

    /// Number of items for backward pagination (cursor mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<u64>,

    // ─────────────────────────────────────────────────────────────────────────
    // Common parameters
    // ─────────────────────────────────────────────────────────────────────────
    /// Field to sort by.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,

    /// Sort direction (asc/desc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<String>,

    /// Additional sort fields (comma-separated, e.g., "created_at:desc,id:asc").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
}

impl PaginationQuery {
    /// Create a new empty pagination query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for pagination query.
    pub fn builder() -> PaginationQueryBuilder {
        PaginationQueryBuilder::new()
    }

    /// Detect the pagination mode based on parameters.
    pub fn detect_mode(&self) -> PaginationMode {
        if self.after.is_some()
            || self.before.is_some()
            || self.first.is_some()
            || self.last.is_some()
        {
            PaginationMode::Cursor
        } else {
            PaginationMode::Offset
        }
    }

    /// Get the effective limit (items per page).
    pub fn effective_limit(&self) -> u64 {
        self.first
            .or(self.last)
            .or(self.per_page)
            .or(self.limit)
            .unwrap_or(super::DEFAULT_PAGE_SIZE)
            .min(super::MAX_PAGE_SIZE)
    }

    /// Get the effective page number.
    pub fn effective_page(&self) -> u64 {
        self.page.unwrap_or(1).max(1)
    }

    /// Parse sort fields from the query parameters.
    pub fn parse_sort_fields(&self) -> Vec<SortField> {
        let mut fields = Vec::new();

        // Parse comma-separated sort string
        if let Some(ref sort) = self.sort {
            for part in sort.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }

                let (name, direction) = if let Some((name, dir)) = part.split_once(':') {
                    let direction = match dir.to_lowercase().as_str() {
                        "desc" | "d" | "-1" => SortDirection::Desc,
                        _ => SortDirection::Asc,
                    };
                    (name.trim().to_string(), direction)
                } else if let Some(stripped) = part.strip_prefix('-') {
                    (stripped.to_string(), SortDirection::Desc)
                } else if let Some(stripped) = part.strip_prefix('+') {
                    (stripped.to_string(), SortDirection::Asc)
                } else {
                    (part.to_string(), SortDirection::Asc)
                };

                if !name.is_empty() {
                    fields.push(SortField::new(name, direction));
                }
            }
        }

        // Add sort_by/sort_order if no sort string
        if fields.is_empty() {
            if let Some(ref sort_by) = self.sort_by {
                let direction = self
                    .sort_order
                    .as_ref()
                    .map(|o| match o.to_lowercase().as_str() {
                        "desc" | "d" | "-1" => SortDirection::Desc,
                        _ => SortDirection::Asc,
                    })
                    .unwrap_or(SortDirection::Asc);

                fields.push(SortField::new(sort_by.clone(), direction));
            }
        }

        fields
    }

    /// Convert to offset pagination.
    pub fn to_offset_pagination(&self) -> OffsetPagination {
        OffsetPagination::new(self.effective_page(), self.effective_limit())
    }

    /// Convert to cursor pagination.
    pub fn to_cursor_pagination(&self) -> Result<CursorPagination, ApexError> {
        let sort_fields = self.parse_sort_fields();

        let mut builder = CursorBuilder::new();
        for field in sort_fields {
            builder = builder.with_field(field.name, field.direction);
        }

        let mut pagination = CursorPagination {
            builder,
            limit: self.effective_limit(),
            after: None,
            before: None,
        };

        // Parse after cursor
        if let Some(ref after) = self.after {
            pagination = pagination.after_token(after)?;
        }

        // Parse before cursor
        if let Some(ref before) = self.before {
            pagination = pagination.before_token(before)?;
        }

        Ok(pagination)
    }

    /// Validate the query parameters.
    pub fn validate(&self) -> Result<(), ApexError> {
        // Check for conflicting cursor parameters
        if self.after.is_some() && self.before.is_some() {
            return Err(ApexError::new(
                ErrorCode::InvalidInput,
                "Cannot specify both 'after' and 'before' cursors",
            ));
        }

        if self.first.is_some() && self.last.is_some() {
            return Err(ApexError::new(
                ErrorCode::InvalidInput,
                "Cannot specify both 'first' and 'last'",
            ));
        }

        // Check limit bounds
        let limit = self.effective_limit();
        if limit > super::MAX_PAGE_SIZE {
            return Err(ApexError::new(
                ErrorCode::InvalidInput,
                format!("Page size cannot exceed {}", super::MAX_PAGE_SIZE),
            ));
        }

        // Validate cursors if provided
        if let Some(ref after) = self.after {
            Cursor::decode(after)?;
        }

        if let Some(ref before) = self.before {
            Cursor::decode(before)?;
        }

        Ok(())
    }

    /// Check if this is a forward pagination request.
    pub fn is_forward(&self) -> bool {
        self.before.is_none() && self.last.is_none()
    }

    /// Check if this is a backward pagination request.
    pub fn is_backward(&self) -> bool {
        self.before.is_some() || self.last.is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pagination Query Builder
// ═══════════════════════════════════════════════════════════════════════════════

/// Builder for creating pagination queries programmatically.
#[derive(Debug, Clone, Default)]
pub struct PaginationQueryBuilder {
    query: PaginationQuery,
}

impl PaginationQueryBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the page number (offset mode).
    pub fn page(mut self, page: u64) -> Self {
        self.query.page = Some(page);
        self
    }

    /// Set the items per page.
    pub fn per_page(mut self, per_page: u64) -> Self {
        self.query.per_page = Some(per_page);
        self
    }

    /// Alias for per_page.
    pub fn limit(mut self, limit: u64) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Set the after cursor (cursor mode).
    pub fn after(mut self, cursor: impl Into<String>) -> Self {
        self.query.after = Some(cursor.into());
        self
    }

    /// Set the before cursor (cursor mode).
    pub fn before(mut self, cursor: impl Into<String>) -> Self {
        self.query.before = Some(cursor.into());
        self
    }

    /// Set the first N items (cursor mode forward).
    pub fn first(mut self, n: u64) -> Self {
        self.query.first = Some(n);
        self
    }

    /// Set the last N items (cursor mode backward).
    pub fn last(mut self, n: u64) -> Self {
        self.query.last = Some(n);
        self
    }

    /// Set the sort field.
    pub fn sort_by(mut self, field: impl Into<String>) -> Self {
        self.query.sort_by = Some(field.into());
        self
    }

    /// Set the sort order.
    pub fn sort_order(mut self, order: SortDirection) -> Self {
        self.query.sort_order = Some(order.to_string());
        self
    }

    /// Set ascending sort order.
    pub fn asc(self) -> Self {
        self.sort_order(SortDirection::Asc)
    }

    /// Set descending sort order.
    pub fn desc(self) -> Self {
        self.sort_order(SortDirection::Desc)
    }

    /// Set the sort string (comma-separated fields).
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.query.sort = Some(sort.into());
        self
    }

    /// Build the pagination query.
    pub fn build(self) -> PaginationQuery {
        self.query
    }

    /// Build and validate the pagination query.
    pub fn build_validated(self) -> Result<PaginationQuery, ApexError> {
        let query = self.build();
        query.validate()?;
        Ok(query)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Axum Integration
// ═══════════════════════════════════════════════════════════════════════════════

mod axum_integration {
    use super::*;
    use axum::extract::{FromRequestParts, Query};
    use axum::http::request::Parts;

    #[axum::async_trait]
    impl<S> FromRequestParts<S> for PaginationQuery
    where
        S: Send + Sync,
    {
        type Rejection = ApexError;

        async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
            let Query(query) = Query::<PaginationQuery>::from_request_parts(parts, state)
                .await
                .map_err(|e| {
                    ApexError::new(
                        ErrorCode::InvalidInput,
                        format!("Invalid pagination parameters: {}", e),
                    )
                })?;

            query.validate()?;
            Ok(query)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_query_defaults() {
        let query = PaginationQuery::new();

        assert_eq!(query.effective_page(), 1);
        assert_eq!(query.effective_limit(), super::super::DEFAULT_PAGE_SIZE);
        assert_eq!(query.detect_mode(), PaginationMode::Offset);
    }

    #[test]
    fn test_pagination_query_offset_mode() {
        let query = PaginationQuery::builder()
            .page(3)
            .per_page(25)
            .build();

        assert_eq!(query.detect_mode(), PaginationMode::Offset);
        assert_eq!(query.effective_page(), 3);
        assert_eq!(query.effective_limit(), 25);
    }

    #[test]
    fn test_pagination_query_cursor_mode() {
        let query = PaginationQuery::builder()
            .first(10)
            .after("cursor123")
            .build();

        assert_eq!(query.detect_mode(), PaginationMode::Cursor);
        assert_eq!(query.effective_limit(), 10);
        assert!(query.is_forward());
    }

    #[test]
    fn test_pagination_query_backward() {
        let query = PaginationQuery::builder()
            .last(10)
            .before("cursor123")
            .build();

        assert!(query.is_backward());
        assert!(!query.is_forward());
    }

    #[test]
    fn test_parse_sort_fields_string() {
        let mut query = PaginationQuery::new();
        query.sort = Some("created_at:desc,id:asc,name".to_string());

        let fields = query.parse_sort_fields();

        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "created_at");
        assert_eq!(fields[0].direction, SortDirection::Desc);
        assert_eq!(fields[1].name, "id");
        assert_eq!(fields[1].direction, SortDirection::Asc);
        assert_eq!(fields[2].name, "name");
        assert_eq!(fields[2].direction, SortDirection::Asc);
    }

    #[test]
    fn test_parse_sort_fields_prefix() {
        let mut query = PaginationQuery::new();
        query.sort = Some("-created_at,+id".to_string());

        let fields = query.parse_sort_fields();

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "created_at");
        assert_eq!(fields[0].direction, SortDirection::Desc);
        assert_eq!(fields[1].name, "id");
        assert_eq!(fields[1].direction, SortDirection::Asc);
    }

    #[test]
    fn test_parse_sort_fields_single() {
        let query = PaginationQuery::builder()
            .sort_by("created_at")
            .desc()
            .build();

        let fields = query.parse_sort_fields();

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "created_at");
        assert_eq!(fields[0].direction, SortDirection::Desc);
    }

    #[test]
    fn test_to_offset_pagination() {
        let query = PaginationQuery::builder()
            .page(5)
            .per_page(30)
            .build();

        let pagination = query.to_offset_pagination();

        assert_eq!(pagination.page, 5);
        assert_eq!(pagination.per_page, 30);
        assert_eq!(pagination.offset(), 120);
    }

    #[test]
    fn test_validation_conflicting_cursors() {
        let query = PaginationQuery::builder()
            .after("cursor1")
            .before("cursor2")
            .build();

        assert!(query.validate().is_err());
    }

    #[test]
    fn test_validation_conflicting_limits() {
        let query = PaginationQuery::builder()
            .first(10)
            .last(10)
            .build();

        assert!(query.validate().is_err());
    }

    #[test]
    fn test_limit_clamping() {
        let query = PaginationQuery::builder()
            .per_page(500) // Exceeds max
            .build();

        assert_eq!(query.effective_limit(), super::super::MAX_PAGE_SIZE);
    }

    #[test]
    fn test_query_serialization() {
        let query = PaginationQuery::builder()
            .page(2)
            .per_page(20)
            .sort_by("created_at")
            .desc()
            .build();

        let json = serde_json::to_string(&query).unwrap();
        let parsed: PaginationQuery = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.page, Some(2));
        assert_eq!(parsed.per_page, Some(20));
        assert_eq!(parsed.sort_by, Some("created_at".to_string()));
    }

    #[test]
    fn test_query_deserialization() {
        let json = r#"{"page": 3, "per_page": 15, "sort": "name:asc,id:desc"}"#;
        let query: PaginationQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.effective_page(), 3);
        assert_eq!(query.effective_limit(), 15);

        let fields = query.parse_sort_fields();
        assert_eq!(fields.len(), 2);
    }
}
