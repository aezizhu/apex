//! Pagination utilities for Apex Core.
//!
//! This module provides comprehensive pagination support including:
//! - Cursor-based pagination for efficient streaming of large datasets
//! - Offset-based pagination for traditional page-based navigation
//! - Unified response types for consistent API responses
//! - Query parameter parsing for HTTP request handling
//!
//! # Usage
//!
//! ```rust,ignore
//! use apex_core::pagination::{
//!     CursorPagination, OffsetPagination, PaginatedResponse,
//!     PaginationQuery, PageInfo, CursorInfo,
//! };
//!
//! // Cursor-based pagination
//! let cursor = CursorPagination::new()
//!     .with_field("created_at", SortDirection::Desc)
//!     .with_field("id", SortDirection::Asc);
//! let encoded = cursor.encode(&my_record)?;
//! let decoded = CursorPagination::decode::<MyRecord>(&encoded)?;
//!
//! // Offset-based pagination
//! let pagination = OffsetPagination::new(1, 20);
//! let response = pagination.paginate(items, total_count);
//! ```

mod cursor;
mod offset;
mod query;
mod response;

pub use cursor::{
    Cursor, CursorBuilder, CursorPagination, CursorValue, SortDirection, SortField,
};
pub use offset::{OffsetPagination, OffsetPaginationBuilder, PageMetadata};
pub use query::{PaginationMode, PaginationQuery, PaginationQueryBuilder};
pub use response::{CursorInfo, PageInfo, PaginatedResponse, PaginationInfo};

/// Default page size if not specified.
pub const DEFAULT_PAGE_SIZE: u64 = 20;

/// Maximum allowed page size.
pub const MAX_PAGE_SIZE: u64 = 100;

/// Minimum page number (1-indexed).
pub const MIN_PAGE_NUMBER: u64 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_page_size_value() {
        assert_eq!(DEFAULT_PAGE_SIZE, 20);
    }

    #[test]
    fn test_max_page_size_value() {
        assert_eq!(MAX_PAGE_SIZE, 100);
    }

    #[test]
    fn test_min_page_number_value() {
        assert_eq!(MIN_PAGE_NUMBER, 1);
    }

    #[test]
    fn test_offset_pagination_default() {
        let p = OffsetPagination::default();
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn test_offset_pagination_sql_clause_page_3() {
        let p = OffsetPagination::new(3, 25);
        assert_eq!(p.sql_clause(), "LIMIT 25 OFFSET 50");
    }

    #[test]
    fn test_offset_pagination_per_page_clamped_to_max() {
        let p = OffsetPagination::new(1, 200);
        assert_eq!(p.per_page, MAX_PAGE_SIZE);
    }

    #[test]
    fn test_offset_pagination_per_page_clamped_to_min() {
        let p = OffsetPagination::new(1, 0);
        assert_eq!(p.per_page, 1);
    }

    #[test]
    fn test_page_metadata_previous_and_next_page() {
        let meta = PageMetadata::new(3, 10, 50);
        assert_eq!(meta.previous_page(), Some(2));
        assert_eq!(meta.next_page(), Some(4));
    }

    #[test]
    fn test_page_metadata_first_page_no_previous() {
        let meta = PageMetadata::new(1, 10, 50);
        assert_eq!(meta.previous_page(), None);
        assert!(meta.next_page().is_some());
    }

    #[test]
    fn test_page_metadata_last_page_no_next() {
        let meta = PageMetadata::new(5, 10, 50);
        assert_eq!(meta.next_page(), None);
        assert!(meta.previous_page().is_some());
    }

    #[test]
    fn test_cursor_encode_decode_roundtrip() {
        let cursor = Cursor::with_value("id", CursorValue::Integer(42));
        let encoded = cursor.encode().unwrap();
        let decoded = Cursor::decode(&encoded).unwrap();
        assert_eq!(
            decoded.get_value("id"),
            Some(&CursorValue::Integer(42))
        );
    }

    #[test]
    fn test_cursor_empty() {
        let cursor = Cursor::new();
        assert!(cursor.is_empty());
    }

    #[test]
    fn test_sort_direction_opposite() {
        assert_eq!(SortDirection::Asc.opposite(), SortDirection::Desc);
        assert_eq!(SortDirection::Desc.opposite(), SortDirection::Asc);
    }

    #[test]
    fn test_sort_field_order_by() {
        let field = SortField::desc("created_at");
        assert_eq!(field.order_by_clause(), "created_at DESC");
    }

    #[test]
    fn test_cursor_pagination_default_limit() {
        let cp = CursorPagination::new();
        assert_eq!(cp.limit, DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn test_cursor_pagination_with_limit_clamped() {
        let cp = CursorPagination::new().with_limit(500);
        assert_eq!(cp.limit, MAX_PAGE_SIZE);
    }

    #[test]
    fn test_paginate_slice_empty() {
        let items: Vec<i32> = vec![];
        let result = OffsetPagination::new(1, 10).paginate_slice(&items);
        assert!(result.is_empty());
    }

    #[test]
    fn test_paginate_iter() {
        let items = 1..=100;
        let result = OffsetPagination::new(2, 10).paginate_iter(items);
        assert_eq!(result, vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
    }

    #[test]
    fn test_offset_pagination_with_per_page() {
        let p = OffsetPagination::new(1, 10).with_per_page(50);
        assert_eq!(p.per_page, 50);
    }

    #[test]
    fn test_offset_pagination_go_to_page() {
        let p = OffsetPagination::new(1, 10).go_to_page(5);
        assert_eq!(p.page, 5);
        assert_eq!(p.offset(), 40);
    }

    #[test]
    fn test_offset_pagination_previous_page_at_one() {
        let p = OffsetPagination::new(1, 10).previous_page();
        assert_eq!(p.page, 1); // Should not go below 1
    }

    #[test]
    fn test_cursor_value_sql_representations() {
        assert_eq!(CursorValue::Integer(42).to_sql_value(), "42");
        assert_eq!(CursorValue::Boolean(true).to_sql_value(), "true");
        assert_eq!(CursorValue::Null.to_sql_value(), "NULL");
        assert_eq!(CursorValue::Float(3.14).to_sql_value(), "3.14");
        assert!(CursorValue::String("hello".into()).to_sql_value().contains("hello"));
    }
}
