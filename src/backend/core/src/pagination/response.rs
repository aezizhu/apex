//! Paginated response types for consistent API responses.
//!
//! This module provides:
//! - Generic PaginatedResponse<T> wrapper
//! - PageInfo for offset-based results
//! - CursorInfo for cursor-based results
//! - Unified pagination information

use serde::{Deserialize, Serialize};

use super::offset::PageMetadata;

// ═══════════════════════════════════════════════════════════════════════════════
// Page Info (Offset-based)
// ═══════════════════════════════════════════════════════════════════════════════

/// Page information for offset-based pagination responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    /// Current page number (1-indexed).
    pub current_page: u64,
    /// Number of items per page.
    pub per_page: u64,
    /// Total number of items.
    pub total_items: u64,
    /// Total number of pages.
    pub total_pages: u64,
    /// Whether there is a previous page.
    pub has_previous_page: bool,
    /// Whether there is a next page.
    pub has_next_page: bool,
}

impl PageInfo {
    /// Create page info from page metadata.
    pub fn from_metadata(meta: &PageMetadata) -> Self {
        Self {
            current_page: meta.page,
            per_page: meta.per_page,
            total_items: meta.total_items,
            total_pages: meta.total_pages,
            has_previous_page: meta.has_previous,
            has_next_page: meta.has_next,
        }
    }

    /// Create page info directly.
    pub fn new(page: u64, per_page: u64, total_items: u64) -> Self {
        Self::from_metadata(&PageMetadata::new(page, per_page, total_items))
    }
}

impl From<PageMetadata> for PageInfo {
    fn from(meta: PageMetadata) -> Self {
        Self::from_metadata(&meta)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cursor Info (Cursor-based)
// ═══════════════════════════════════════════════════════════════════════════════

/// Cursor information for cursor-based pagination responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorInfo {
    /// Cursor for the first item in the result set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_cursor: Option<String>,
    /// Cursor for the last item in the result set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_cursor: Option<String>,
    /// Whether there are more items before the start cursor.
    pub has_previous_page: bool,
    /// Whether there are more items after the end cursor.
    pub has_next_page: bool,
}

impl CursorInfo {
    /// Create cursor info with no cursors.
    pub fn empty() -> Self {
        Self {
            start_cursor: None,
            end_cursor: None,
            has_previous_page: false,
            has_next_page: false,
        }
    }

    /// Create cursor info with cursors.
    pub fn new(
        start_cursor: Option<String>,
        end_cursor: Option<String>,
        has_previous: bool,
        has_next: bool,
    ) -> Self {
        Self {
            start_cursor,
            end_cursor,
            has_previous_page: has_previous,
            has_next_page: has_next,
        }
    }

    /// Set the start cursor.
    pub fn with_start_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.start_cursor = Some(cursor.into());
        self
    }

    /// Set the end cursor.
    pub fn with_end_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.end_cursor = Some(cursor.into());
        self
    }

    /// Set has_previous_page.
    pub fn with_has_previous(mut self, has_previous: bool) -> Self {
        self.has_previous_page = has_previous;
        self
    }

    /// Set has_next_page.
    pub fn with_has_next(mut self, has_next: bool) -> Self {
        self.has_next_page = has_next;
        self
    }
}

impl Default for CursorInfo {
    fn default() -> Self {
        Self::empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pagination Info (Unified)
// ═══════════════════════════════════════════════════════════════════════════════

/// Unified pagination information supporting both offset and cursor styles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationInfo {
    /// Page-based pagination info (offset style).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_info: Option<PageInfo>,
    /// Cursor-based pagination info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_info: Option<CursorInfo>,
    /// Total number of items (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
}

impl PaginationInfo {
    /// Create pagination info for offset-based pagination.
    pub fn offset(page_info: PageInfo) -> Self {
        Self {
            total_count: Some(page_info.total_items),
            page_info: Some(page_info),
            cursor_info: None,
        }
    }

    /// Create pagination info for cursor-based pagination.
    pub fn cursor(cursor_info: CursorInfo, total_count: Option<u64>) -> Self {
        Self {
            total_count,
            page_info: None,
            cursor_info: Some(cursor_info),
        }
    }

    /// Create pagination info with both offset and cursor info.
    pub fn combined(page_info: PageInfo, cursor_info: CursorInfo) -> Self {
        Self {
            total_count: Some(page_info.total_items),
            page_info: Some(page_info),
            cursor_info: Some(cursor_info),
        }
    }

    /// Check if there is a next page.
    pub fn has_next_page(&self) -> bool {
        self.page_info
            .as_ref()
            .map(|p| p.has_next_page)
            .or_else(|| self.cursor_info.as_ref().map(|c| c.has_next_page))
            .unwrap_or(false)
    }

    /// Check if there is a previous page.
    pub fn has_previous_page(&self) -> bool {
        self.page_info
            .as_ref()
            .map(|p| p.has_previous_page)
            .or_else(|| self.cursor_info.as_ref().map(|c| c.has_previous_page))
            .unwrap_or(false)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Paginated Response
// ═══════════════════════════════════════════════════════════════════════════════

/// A paginated response wrapper.
///
/// This provides a consistent structure for all paginated API responses,
/// supporting both offset-based and cursor-based pagination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    /// The items in this page.
    pub data: Vec<T>,
    /// Pagination information.
    pub pagination: PaginationInfo,
    /// Whether the request was successful.
    #[serde(default = "default_success")]
    pub success: bool,
}

fn default_success() -> bool {
    true
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response.
    pub fn new(data: Vec<T>, pagination: PaginationInfo) -> Self {
        Self {
            data,
            pagination,
            success: true,
        }
    }

    /// Create an empty paginated response.
    pub fn empty() -> Self {
        Self {
            data: Vec::new(),
            pagination: PaginationInfo {
                page_info: None,
                cursor_info: Some(CursorInfo::empty()),
                total_count: Some(0),
            },
            success: true,
        }
    }

    /// Create a response for offset-based pagination.
    pub fn offset(data: Vec<T>, page: u64, per_page: u64, total_items: u64) -> Self {
        Self::new(
            data,
            PaginationInfo::offset(PageInfo::new(page, per_page, total_items)),
        )
    }

    /// Create a response for cursor-based pagination.
    pub fn cursor(
        data: Vec<T>,
        cursor_info: CursorInfo,
        total_count: Option<u64>,
    ) -> Self {
        Self::new(data, PaginationInfo::cursor(cursor_info, total_count))
    }

    /// Get the number of items in this response.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the response is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the total count if available.
    pub fn total_count(&self) -> Option<u64> {
        self.pagination.total_count
    }

    /// Check if there are more items available.
    pub fn has_more(&self) -> bool {
        self.pagination.has_next_page()
    }

    /// Map the data items to a different type.
    pub fn map<U, F>(self, f: F) -> PaginatedResponse<U>
    where
        F: FnMut(T) -> U,
    {
        PaginatedResponse {
            data: self.data.into_iter().map(f).collect(),
            pagination: self.pagination,
            success: self.success,
        }
    }

    /// Filter the data items.
    pub fn filter<P>(mut self, predicate: P) -> Self
    where
        P: FnMut(&T) -> bool,
    {
        self.data = self.data.into_iter().filter(predicate).collect();
        self
    }

    /// Get a reference to the first item.
    pub fn first(&self) -> Option<&T> {
        self.data.first()
    }

    /// Get a reference to the last item.
    pub fn last(&self) -> Option<&T> {
        self.data.last()
    }

    /// Get an iterator over references to the items.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }
}

impl<T> Default for PaginatedResponse<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T> IntoIterator for PaginatedResponse<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Edge Type for GraphQL-style Connections
// ═══════════════════════════════════════════════════════════════════════════════

/// An edge in a connection (GraphQL Relay style).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Edge<T> {
    /// The item at this edge.
    pub node: T,
    /// The cursor for this edge.
    pub cursor: String,
}

#[allow(dead_code)]
impl<T> Edge<T> {
    /// Create a new edge.
    pub fn new(node: T, cursor: impl Into<String>) -> Self {
        Self {
            node,
            cursor: cursor.into(),
        }
    }
}

/// A connection (GraphQL Relay style pagination).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Connection<T> {
    /// The edges in this connection.
    pub edges: Vec<Edge<T>>,
    /// Pagination information.
    pub page_info: CursorInfo,
    /// Total count of items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
}

#[allow(dead_code)]
impl<T> Connection<T> {
    /// Create a new connection.
    pub fn new(edges: Vec<Edge<T>>, page_info: CursorInfo, total_count: Option<u64>) -> Self {
        Self {
            edges,
            page_info,
            total_count,
        }
    }

    /// Create an empty connection.
    pub fn empty() -> Self {
        Self {
            edges: Vec::new(),
            page_info: CursorInfo::empty(),
            total_count: Some(0),
        }
    }

    /// Get the nodes without cursors.
    pub fn nodes(&self) -> Vec<&T> {
        self.edges.iter().map(|e| &e.node).collect()
    }

    /// Map the nodes to a different type.
    pub fn map<U, F>(self, mut f: F) -> Connection<U>
    where
        F: FnMut(T) -> U,
    {
        Connection {
            edges: self
                .edges
                .into_iter()
                .map(|e| Edge::new(f(e.node), e.cursor))
                .collect(),
            page_info: self.page_info,
            total_count: self.total_count,
        }
    }
}

impl<T> Default for Connection<T> {
    fn default() -> Self {
        Self::empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_info_creation() {
        let info = PageInfo::new(2, 10, 45);

        assert_eq!(info.current_page, 2);
        assert_eq!(info.per_page, 10);
        assert_eq!(info.total_items, 45);
        assert_eq!(info.total_pages, 5);
        assert!(info.has_previous_page);
        assert!(info.has_next_page);
    }

    #[test]
    fn test_cursor_info_builder() {
        let info = CursorInfo::empty()
            .with_start_cursor("start123")
            .with_end_cursor("end456")
            .with_has_previous(true)
            .with_has_next(true);

        assert_eq!(info.start_cursor, Some("start123".to_string()));
        assert_eq!(info.end_cursor, Some("end456".to_string()));
        assert!(info.has_previous_page);
        assert!(info.has_next_page);
    }

    #[test]
    fn test_paginated_response_offset() {
        let items = vec![1, 2, 3, 4, 5];
        let response = PaginatedResponse::offset(items, 1, 5, 100);

        assert_eq!(response.len(), 5);
        assert!(!response.is_empty());
        assert_eq!(response.total_count(), Some(100));
        assert!(response.has_more());
    }

    #[test]
    fn test_paginated_response_cursor() {
        let items = vec!["a", "b", "c"];
        let cursor_info = CursorInfo::new(
            Some("cursor_a".to_string()),
            Some("cursor_c".to_string()),
            false,
            true,
        );
        let response = PaginatedResponse::cursor(items, cursor_info, Some(50));

        assert_eq!(response.len(), 3);
        assert_eq!(response.total_count(), Some(50));
        assert!(response.has_more());
    }

    #[test]
    fn test_paginated_response_map() {
        let items = vec![1, 2, 3];
        let response = PaginatedResponse::offset(items, 1, 10, 3);

        let mapped = response.map(|x| x * 2);
        assert_eq!(mapped.data, vec![2, 4, 6]);
    }

    #[test]
    fn test_paginated_response_empty() {
        let response: PaginatedResponse<i32> = PaginatedResponse::empty();

        assert!(response.is_empty());
        assert_eq!(response.total_count(), Some(0));
        assert!(!response.has_more());
    }

    #[test]
    fn test_pagination_info_has_pages() {
        let page_info = PageInfo::new(2, 10, 30);
        let pagination = PaginationInfo::offset(page_info);

        assert!(pagination.has_previous_page());
        assert!(pagination.has_next_page());
    }

    #[test]
    fn test_connection() {
        let edges = vec![
            Edge::new("item1", "cursor1"),
            Edge::new("item2", "cursor2"),
        ];
        let page_info = CursorInfo::new(
            Some("cursor1".to_string()),
            Some("cursor2".to_string()),
            false,
            true,
        );
        let connection = Connection::new(edges, page_info, Some(10));

        let nodes = connection.nodes();
        assert_eq!(nodes, vec![&"item1", &"item2"]);
    }

    #[test]
    fn test_connection_map() {
        let edges = vec![
            Edge::new(1, "c1"),
            Edge::new(2, "c2"),
        ];
        let connection = Connection::new(edges, CursorInfo::empty(), None);

        let mapped = connection.map(|x| x * 10);
        let nodes: Vec<_> = mapped.edges.iter().map(|e| e.node).collect();
        assert_eq!(nodes, vec![10, 20]);
    }

    #[test]
    fn test_paginated_response_serialization() {
        let response = PaginatedResponse::offset(vec![1, 2, 3], 1, 10, 100);
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"data\""));
        assert!(json.contains("\"pagination\""));
        assert!(json.contains("\"success\":true"));
    }
}
