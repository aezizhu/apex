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
