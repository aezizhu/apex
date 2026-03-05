//! Offset-based pagination for traditional page-based navigation.
//!
//! This module provides:
//! - Page/per_page parameter handling
//! - Total count tracking
//! - Page metadata computation
//! - SQL OFFSET/LIMIT clause generation

use serde::{Deserialize, Serialize};

use crate::error::{ApexError, ErrorCode};

// ═══════════════════════════════════════════════════════════════════════════════
// Page Metadata
// ═══════════════════════════════════════════════════════════════════════════════

/// Metadata about a paginated result set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageMetadata {
    /// Current page number (1-indexed).
    pub page: u64,
    /// Number of items per page.
    pub per_page: u64,
    /// Total number of items across all pages.
    pub total_items: u64,
    /// Total number of pages.
    pub total_pages: u64,
    /// Whether there is a previous page.
    pub has_previous: bool,
    /// Whether there is a next page.
    pub has_next: bool,
    /// The range of items on this page (1-indexed).
    pub item_range: (u64, u64),
}

impl PageMetadata {
    /// Create page metadata from pagination parameters and total count.
    pub fn new(page: u64, per_page: u64, total_items: u64) -> Self {
        let total_pages = if total_items == 0 {
            1
        } else {
            total_items.div_ceil(per_page)
        };

        let page = page.clamp(1, total_pages.max(1));
        let has_previous = page > 1;
        let has_next = page < total_pages;

        let start = (page - 1) * per_page + 1;
        let end = (start + per_page - 1).min(total_items);
        let item_range = if total_items == 0 { (0, 0) } else { (start, end) };

        Self {
            page,
            per_page,
            total_items,
            total_pages,
            has_previous,
            has_next,
            item_range,
        }
    }

    /// Get the number of items on this page.
    pub fn items_on_page(&self) -> u64 {
        if self.item_range.0 == 0 {
            0
        } else {
            self.item_range.1 - self.item_range.0 + 1
        }
    }

    /// Get the previous page number if available.
    pub fn previous_page(&self) -> Option<u64> {
        if self.has_previous {
            Some(self.page - 1)
        } else {
            None
        }
    }

    /// Get the next page number if available.
    pub fn next_page(&self) -> Option<u64> {
        if self.has_next {
            Some(self.page + 1)
        } else {
            None
        }
    }

    /// Get a range of page numbers for pagination UI.
    ///
    /// Returns page numbers centered around the current page,
    /// with `window_size` pages on each side.
    pub fn page_window(&self, window_size: u64) -> Vec<u64> {
        let start = self.page.saturating_sub(window_size).max(1);
        let end = (self.page + window_size).min(self.total_pages);
        (start..=end).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Offset Pagination
// ═══════════════════════════════════════════════════════════════════════════════

/// Offset-based pagination parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffsetPagination {
    /// Current page number (1-indexed).
    pub page: u64,
    /// Number of items per page.
    pub per_page: u64,
}

impl OffsetPagination {
    /// Create a new offset pagination with the given page and per_page.
    pub fn new(page: u64, per_page: u64) -> Self {
        Self {
            page: page.max(super::MIN_PAGE_NUMBER),
            per_page: per_page.clamp(1, super::MAX_PAGE_SIZE),
        }
    }

    /// Create a new offset pagination with default values.
    pub fn default_pagination() -> Self {
        Self {
            page: 1,
            per_page: super::DEFAULT_PAGE_SIZE,
        }
    }

    /// Get the SQL OFFSET value.
    pub fn offset(&self) -> u64 {
        (self.page - 1) * self.per_page
    }

    /// Get the SQL LIMIT value.
    pub fn limit(&self) -> u64 {
        self.per_page
    }

    /// Generate the SQL OFFSET and LIMIT clause.
    pub fn sql_clause(&self) -> String {
        format!("LIMIT {} OFFSET {}", self.limit(), self.offset())
    }

    /// Create page metadata from a total count.
    pub fn metadata(&self, total_items: u64) -> PageMetadata {
        PageMetadata::new(self.page, self.per_page, total_items)
    }

    /// Validate the pagination parameters.
    pub fn validate(&self) -> Result<(), ApexError> {
        if self.page < 1 {
            return Err(ApexError::new(
                ErrorCode::InvalidInput,
                "Page number must be at least 1",
            ));
        }

        if self.per_page < 1 {
            return Err(ApexError::new(
                ErrorCode::InvalidInput,
                "Items per page must be at least 1",
            ));
        }

        if self.per_page > super::MAX_PAGE_SIZE {
            return Err(ApexError::new(
                ErrorCode::InvalidInput,
                format!("Items per page cannot exceed {}", super::MAX_PAGE_SIZE),
            ));
        }

        Ok(())
    }

    /// Apply pagination to a slice of items.
    pub fn paginate_slice<T: Clone>(&self, items: &[T]) -> Vec<T> {
        let start = self.offset() as usize;
        let end = (start + self.per_page as usize).min(items.len());

        if start >= items.len() {
            Vec::new()
        } else {
            items[start..end].to_vec()
        }
    }

    /// Apply pagination to an iterator.
    pub fn paginate_iter<T, I: Iterator<Item = T>>(&self, iter: I) -> Vec<T> {
        iter.skip(self.offset() as usize)
            .take(self.per_page as usize)
            .collect()
    }

    /// Go to the next page.
    pub fn next_page(mut self) -> Self {
        self.page += 1;
        self
    }

    /// Go to the previous page (minimum page 1).
    pub fn previous_page(mut self) -> Self {
        self.page = self.page.saturating_sub(1).max(1);
        self
    }

    /// Go to a specific page.
    pub fn go_to_page(mut self, page: u64) -> Self {
        self.page = page.max(1);
        self
    }

    /// Set the items per page.
    pub fn with_per_page(mut self, per_page: u64) -> Self {
        self.per_page = per_page.clamp(1, super::MAX_PAGE_SIZE);
        self
    }
}

impl Default for OffsetPagination {
    fn default() -> Self {
        Self::default_pagination()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Offset Pagination Builder
// ═══════════════════════════════════════════════════════════════════════════════

/// Builder for creating offset-based pagination.
#[derive(Debug, Clone, Default)]
pub struct OffsetPaginationBuilder {
    page: Option<u64>,
    per_page: Option<u64>,
}

impl OffsetPaginationBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the page number.
    pub fn page(mut self, page: u64) -> Self {
        self.page = Some(page);
        self
    }

    /// Set the items per page.
    pub fn per_page(mut self, per_page: u64) -> Self {
        self.per_page = Some(per_page);
        self
    }

    /// Build the pagination.
    pub fn build(self) -> OffsetPagination {
        OffsetPagination::new(
            self.page.unwrap_or(1),
            self.per_page.unwrap_or(super::DEFAULT_PAGE_SIZE),
        )
    }

    /// Build and validate the pagination.
    pub fn build_validated(self) -> Result<OffsetPagination, ApexError> {
        let pagination = self.build();
        pagination.validate()?;
        Ok(pagination)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_metadata_basic() {
        let meta = PageMetadata::new(1, 10, 100);

        assert_eq!(meta.page, 1);
        assert_eq!(meta.per_page, 10);
        assert_eq!(meta.total_items, 100);
        assert_eq!(meta.total_pages, 10);
        assert!(!meta.has_previous);
        assert!(meta.has_next);
        assert_eq!(meta.item_range, (1, 10));
    }

    #[test]
    fn test_page_metadata_last_page() {
        let meta = PageMetadata::new(10, 10, 100);

        assert_eq!(meta.page, 10);
        assert!(meta.has_previous);
        assert!(!meta.has_next);
        assert_eq!(meta.item_range, (91, 100));
    }

    #[test]
    fn test_page_metadata_partial_page() {
        let meta = PageMetadata::new(3, 10, 25);

        assert_eq!(meta.page, 3);
        assert_eq!(meta.total_pages, 3);
        assert_eq!(meta.item_range, (21, 25));
        assert_eq!(meta.items_on_page(), 5);
    }

    #[test]
    fn test_page_metadata_empty() {
        let meta = PageMetadata::new(1, 10, 0);

        assert_eq!(meta.page, 1);
        assert_eq!(meta.total_pages, 1);
        assert!(!meta.has_previous);
        assert!(!meta.has_next);
        assert_eq!(meta.item_range, (0, 0));
        assert_eq!(meta.items_on_page(), 0);
    }

    #[test]
    fn test_page_metadata_page_window() {
        let meta = PageMetadata::new(5, 10, 100);
        let window = meta.page_window(2);

        assert_eq!(window, vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_page_metadata_window_at_start() {
        let meta = PageMetadata::new(1, 10, 100);
        let window = meta.page_window(2);

        assert_eq!(window, vec![1, 2, 3]);
    }

    #[test]
    fn test_page_metadata_window_at_end() {
        let meta = PageMetadata::new(10, 10, 100);
        let window = meta.page_window(2);

        assert_eq!(window, vec![8, 9, 10]);
    }

    #[test]
    fn test_offset_pagination_basic() {
        let pagination = OffsetPagination::new(1, 20);

        assert_eq!(pagination.offset(), 0);
        assert_eq!(pagination.limit(), 20);
        assert_eq!(pagination.sql_clause(), "LIMIT 20 OFFSET 0");
    }

    #[test]
    fn test_offset_pagination_page_2() {
        let pagination = OffsetPagination::new(2, 20);

        assert_eq!(pagination.offset(), 20);
        assert_eq!(pagination.limit(), 20);
        assert_eq!(pagination.sql_clause(), "LIMIT 20 OFFSET 20");
    }

    #[test]
    fn test_offset_pagination_clamps_values() {
        let pagination = OffsetPagination::new(0, 500);

        assert_eq!(pagination.page, 1); // Clamped to minimum
        assert_eq!(pagination.per_page, crate::pagination::MAX_PAGE_SIZE); // Clamped to maximum
    }

    #[test]
    fn test_offset_pagination_validate() {
        let valid = OffsetPagination::new(1, 20);
        assert!(valid.validate().is_ok());
    }

    #[test]
    fn test_offset_pagination_paginate_slice() {
        let items: Vec<i32> = (1..=100).collect();

        let page1 = OffsetPagination::new(1, 10).paginate_slice(&items);
        assert_eq!(page1, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let page5 = OffsetPagination::new(5, 10).paginate_slice(&items);
        assert_eq!(page5, vec![41, 42, 43, 44, 45, 46, 47, 48, 49, 50]);

        let last_page = OffsetPagination::new(10, 10).paginate_slice(&items);
        assert_eq!(last_page, vec![91, 92, 93, 94, 95, 96, 97, 98, 99, 100]);

        let beyond = OffsetPagination::new(11, 10).paginate_slice(&items);
        assert!(beyond.is_empty());
    }

    #[test]
    fn test_offset_pagination_navigation() {
        let pagination = OffsetPagination::new(5, 20);

        let next = pagination.clone().next_page();
        assert_eq!(next.page, 6);

        let prev = pagination.clone().previous_page();
        assert_eq!(prev.page, 4);

        let goto = pagination.go_to_page(10);
        assert_eq!(goto.page, 10);
    }

    #[test]
    fn test_offset_pagination_builder() {
        let pagination = OffsetPaginationBuilder::new()
            .page(3)
            .per_page(25)
            .build();

        assert_eq!(pagination.page, 3);
        assert_eq!(pagination.per_page, 25);
    }

    #[test]
    fn test_offset_pagination_metadata() {
        let pagination = OffsetPagination::new(2, 10);
        let meta = pagination.metadata(45);

        assert_eq!(meta.page, 2);
        assert_eq!(meta.per_page, 10);
        assert_eq!(meta.total_items, 45);
        assert_eq!(meta.total_pages, 5);
        assert!(meta.has_previous);
        assert!(meta.has_next);
    }

    #[test]
    fn test_page_clamp_to_max() {
        let meta = PageMetadata::new(100, 10, 50); // Page 100 of only 5 pages

        assert_eq!(meta.page, 5); // Clamped to last page
        assert!(!meta.has_next);
    }
}
