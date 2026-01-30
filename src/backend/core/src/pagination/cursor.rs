//! Cursor-based pagination for efficient streaming of large datasets.
//!
//! This module provides:
//! - Opaque cursor tokens using Base64 encoding
//! - Support for multiple sort fields
//! - Type-safe cursor encoding/decoding
//! - Bidirectional navigation (forward/backward)

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::{ApexError, ErrorCode};

// ═══════════════════════════════════════════════════════════════════════════════
// Sort Direction
// ═══════════════════════════════════════════════════════════════════════════════

/// Sort direction for cursor-based pagination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Ascending order (A-Z, 0-9, oldest first).
    Asc,
    /// Descending order (Z-A, 9-0, newest first).
    Desc,
}

impl SortDirection {
    /// Get the SQL keyword for this direction.
    pub fn sql_keyword(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }

    /// Get the opposite direction.
    pub fn opposite(&self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    /// Get the comparison operator for "after" queries.
    pub fn after_operator(&self) -> &'static str {
        match self {
            Self::Asc => ">",
            Self::Desc => "<",
        }
    }

    /// Get the comparison operator for "before" queries.
    pub fn before_operator(&self) -> &'static str {
        match self {
            Self::Asc => "<",
            Self::Desc => ">",
        }
    }
}

impl Default for SortDirection {
    fn default() -> Self {
        Self::Asc
    }
}

impl std::fmt::Display for SortDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asc => write!(f, "asc"),
            Self::Desc => write!(f, "desc"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sort Field
// ═══════════════════════════════════════════════════════════════════════════════

/// A field used for sorting in cursor-based pagination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortField {
    /// The name of the field (column name).
    pub name: String,
    /// The sort direction.
    pub direction: SortDirection,
}

impl SortField {
    /// Create a new sort field with ascending order.
    pub fn asc(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            direction: SortDirection::Asc,
        }
    }

    /// Create a new sort field with descending order.
    pub fn desc(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            direction: SortDirection::Desc,
        }
    }

    /// Create a new sort field with the specified direction.
    pub fn new(name: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            name: name.into(),
            direction,
        }
    }

    /// Get the SQL ORDER BY clause for this field.
    pub fn order_by_clause(&self) -> String {
        format!("{} {}", self.name, self.direction.sql_keyword())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cursor Value
// ═══════════════════════════════════════════════════════════════════════════════

/// A value stored in a cursor, supporting common database types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CursorValue {
    /// String value.
    String(String),
    /// Integer value (i64).
    Integer(i64),
    /// Floating point value.
    Float(f64),
    /// Boolean value.
    Boolean(bool),
    /// UUID value (stored as string).
    Uuid(String),
    /// Timestamp value (ISO 8601 string).
    Timestamp(String),
    /// Null value.
    Null,
}

impl CursorValue {
    /// Create a cursor value from a UUID.
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self::Uuid(uuid.to_string())
    }

    /// Create a cursor value from a timestamp.
    pub fn from_timestamp(ts: chrono::DateTime<chrono::Utc>) -> Self {
        Self::Timestamp(ts.to_rfc3339())
    }

    /// Try to convert to a UUID.
    pub fn as_uuid(&self) -> Option<uuid::Uuid> {
        match self {
            Self::Uuid(s) | Self::String(s) => uuid::Uuid::parse_str(s).ok(),
            _ => None,
        }
    }

    /// Try to convert to a timestamp.
    pub fn as_timestamp(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        match self {
            Self::Timestamp(s) | Self::String(s) => {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            }
            _ => None,
        }
    }

    /// Get the value as a string (for SQL query building).
    pub fn to_sql_value(&self) -> String {
        match self {
            Self::String(s) => format!("'{}'", s.replace('\'', "''")),
            Self::Integer(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::Uuid(u) => format!("'{}'", u),
            Self::Timestamp(t) => format!("'{}'", t),
            Self::Null => "NULL".to_string(),
        }
    }
}

impl From<String> for CursorValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for CursorValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<i64> for CursorValue {
    fn from(i: i64) -> Self {
        Self::Integer(i)
    }
}

impl From<i32> for CursorValue {
    fn from(i: i32) -> Self {
        Self::Integer(i as i64)
    }
}

impl From<f64> for CursorValue {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<bool> for CursorValue {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

impl From<uuid::Uuid> for CursorValue {
    fn from(u: uuid::Uuid) -> Self {
        Self::Uuid(u.to_string())
    }
}

impl From<chrono::DateTime<chrono::Utc>> for CursorValue {
    fn from(ts: chrono::DateTime<chrono::Utc>) -> Self {
        Self::Timestamp(ts.to_rfc3339())
    }
}

impl<T> From<Option<T>> for CursorValue
where
    T: Into<CursorValue>,
{
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cursor
// ═══════════════════════════════════════════════════════════════════════════════

/// An opaque cursor for pagination.
///
/// The cursor contains the values of the sort fields for a specific record,
/// allowing efficient "seek" pagination without offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    /// Version for forward compatibility.
    #[serde(default = "default_cursor_version")]
    pub version: u8,
    /// The field values that define this cursor position.
    pub values: BTreeMap<String, CursorValue>,
    /// Optional metadata (e.g., for sharding).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

fn default_cursor_version() -> u8 {
    1
}

impl Cursor {
    /// Create a new empty cursor.
    pub fn new() -> Self {
        Self {
            version: 1,
            values: BTreeMap::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Create a cursor with a single field value.
    pub fn with_value(field: impl Into<String>, value: impl Into<CursorValue>) -> Self {
        let mut cursor = Self::new();
        cursor.values.insert(field.into(), value.into());
        cursor
    }

    /// Add a field value to the cursor.
    pub fn add_value(&mut self, field: impl Into<String>, value: impl Into<CursorValue>) {
        self.values.insert(field.into(), value.into());
    }

    /// Add metadata to the cursor.
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get a field value from the cursor.
    pub fn get_value(&self, field: &str) -> Option<&CursorValue> {
        self.values.get(field)
    }

    /// Get metadata from the cursor.
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Check if the cursor is empty (no values).
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Encode the cursor to an opaque string token.
    pub fn encode(&self) -> Result<String, ApexError> {
        let json = serde_json::to_string(self).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::SerializationError,
                "Failed to encode cursor",
                e.to_string(),
            )
        })?;
        Ok(URL_SAFE_NO_PAD.encode(json.as_bytes()))
    }

    /// Decode a cursor from an opaque string token.
    pub fn decode(token: &str) -> Result<Self, ApexError> {
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::InvalidInput,
                "Invalid cursor format",
                e.to_string(),
            )
        })?;

        let json = String::from_utf8(bytes).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::InvalidInput,
                "Invalid cursor encoding",
                e.to_string(),
            )
        })?;

        serde_json::from_str(&json).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::DeserializationError,
                "Failed to decode cursor",
                e.to_string(),
            )
        })
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cursor Builder
// ═══════════════════════════════════════════════════════════════════════════════

/// Builder for creating cursors from records.
#[derive(Debug, Clone)]
pub struct CursorBuilder {
    /// The sort fields to extract values from.
    fields: Vec<SortField>,
}

impl CursorBuilder {
    /// Create a new cursor builder.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a sort field to extract.
    pub fn with_field(mut self, name: impl Into<String>, direction: SortDirection) -> Self {
        self.fields.push(SortField::new(name, direction));
        self
    }

    /// Add an ascending sort field.
    pub fn asc(self, name: impl Into<String>) -> Self {
        self.with_field(name, SortDirection::Asc)
    }

    /// Add a descending sort field.
    pub fn desc(self, name: impl Into<String>) -> Self {
        self.with_field(name, SortDirection::Desc)
    }

    /// Get the sort fields.
    pub fn fields(&self) -> &[SortField] {
        &self.fields
    }

    /// Build a cursor from a serializable record.
    ///
    /// The record must be serializable to JSON with fields matching the sort field names.
    pub fn build_from<T: Serialize>(&self, record: &T) -> Result<Cursor, ApexError> {
        let value = serde_json::to_value(record).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::SerializationError,
                "Failed to serialize record for cursor",
                e.to_string(),
            )
        })?;

        let mut cursor = Cursor::new();

        if let serde_json::Value::Object(map) = value {
            for field in &self.fields {
                if let Some(field_value) = map.get(&field.name) {
                    let cursor_value = json_to_cursor_value(field_value);
                    cursor.add_value(&field.name, cursor_value);
                }
            }
        }

        Ok(cursor)
    }

    /// Generate the ORDER BY clause for SQL queries.
    pub fn order_by_clause(&self) -> String {
        self.fields
            .iter()
            .map(|f| f.order_by_clause())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Generate a WHERE clause for seeking after a cursor.
    ///
    /// This creates a tuple comparison for efficient multi-column pagination.
    pub fn where_after_clause(&self, cursor: &Cursor, params_offset: usize) -> (String, Vec<CursorValue>) {
        if cursor.is_empty() || self.fields.is_empty() {
            return (String::new(), Vec::new());
        }

        let mut values = Vec::new();
        let mut field_names = Vec::new();
        let mut param_placeholders = Vec::new();

        for (i, field) in self.fields.iter().enumerate() {
            if let Some(value) = cursor.get_value(&field.name) {
                field_names.push(field.name.clone());
                param_placeholders.push(format!("${}", params_offset + i + 1));
                values.push(value.clone());
            }
        }

        if field_names.is_empty() {
            return (String::new(), Vec::new());
        }

        // Determine operator based on primary sort direction
        let operator = self.fields[0].direction.after_operator();

        // For single field, simple comparison
        if field_names.len() == 1 {
            let clause = format!("{} {} {}", field_names[0], operator, param_placeholders[0]);
            return (clause, values);
        }

        // For multiple fields, use tuple comparison
        let fields_str = field_names.join(", ");
        let params_str = param_placeholders.join(", ");
        let clause = format!("({}) {} ({})", fields_str, operator, params_str);

        (clause, values)
    }

    /// Generate a WHERE clause for seeking before a cursor.
    pub fn where_before_clause(&self, cursor: &Cursor, params_offset: usize) -> (String, Vec<CursorValue>) {
        if cursor.is_empty() || self.fields.is_empty() {
            return (String::new(), Vec::new());
        }

        let mut values = Vec::new();
        let mut field_names = Vec::new();
        let mut param_placeholders = Vec::new();

        for (i, field) in self.fields.iter().enumerate() {
            if let Some(value) = cursor.get_value(&field.name) {
                field_names.push(field.name.clone());
                param_placeholders.push(format!("${}", params_offset + i + 1));
                values.push(value.clone());
            }
        }

        if field_names.is_empty() {
            return (String::new(), Vec::new());
        }

        // Determine operator based on primary sort direction
        let operator = self.fields[0].direction.before_operator();

        // For single field, simple comparison
        if field_names.len() == 1 {
            let clause = format!("{} {} {}", field_names[0], operator, param_placeholders[0]);
            return (clause, values);
        }

        // For multiple fields, use tuple comparison
        let fields_str = field_names.join(", ");
        let params_str = param_placeholders.join(", ");
        let clause = format!("({}) {} ({})", fields_str, operator, params_str);

        (clause, values)
    }
}

impl Default for CursorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a JSON value to a CursorValue.
fn json_to_cursor_value(value: &serde_json::Value) -> CursorValue {
    match value {
        serde_json::Value::Null => CursorValue::Null,
        serde_json::Value::Bool(b) => CursorValue::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CursorValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                CursorValue::Float(f)
            } else {
                CursorValue::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => {
            // Try to detect UUID or timestamp
            if uuid::Uuid::parse_str(s).is_ok() {
                CursorValue::Uuid(s.clone())
            } else if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
                CursorValue::Timestamp(s.clone())
            } else {
                CursorValue::String(s.clone())
            }
        }
        // Arrays and objects become strings
        _ => CursorValue::String(value.to_string()),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cursor Pagination
// ═══════════════════════════════════════════════════════════════════════════════

/// High-level cursor-based pagination helper.
#[derive(Debug, Clone)]
pub struct CursorPagination {
    /// The cursor builder for creating/extracting cursors.
    pub builder: CursorBuilder,
    /// Maximum number of items per page.
    pub limit: u64,
    /// Cursor for forward pagination ("after").
    pub after: Option<Cursor>,
    /// Cursor for backward pagination ("before").
    pub before: Option<Cursor>,
}

impl CursorPagination {
    /// Create a new cursor pagination with default settings.
    pub fn new() -> Self {
        Self {
            builder: CursorBuilder::new(),
            limit: super::DEFAULT_PAGE_SIZE,
            after: None,
            before: None,
        }
    }

    /// Set the page limit.
    pub fn with_limit(mut self, limit: u64) -> Self {
        self.limit = limit.min(super::MAX_PAGE_SIZE);
        self
    }

    /// Add a sort field.
    pub fn with_field(mut self, name: impl Into<String>, direction: SortDirection) -> Self {
        self.builder = self.builder.with_field(name, direction);
        self
    }

    /// Set the "after" cursor for forward pagination.
    pub fn after(mut self, cursor: Cursor) -> Self {
        self.after = Some(cursor);
        self.before = None; // Clear before if setting after
        self
    }

    /// Set the "after" cursor from an encoded token.
    pub fn after_token(mut self, token: &str) -> Result<Self, ApexError> {
        self.after = Some(Cursor::decode(token)?);
        self.before = None;
        Ok(self)
    }

    /// Set the "before" cursor for backward pagination.
    pub fn before(mut self, cursor: Cursor) -> Self {
        self.before = Some(cursor);
        self.after = None; // Clear after if setting before
        self
    }

    /// Set the "before" cursor from an encoded token.
    pub fn before_token(mut self, token: &str) -> Result<Self, ApexError> {
        self.before = Some(Cursor::decode(token)?);
        self.after = None;
        Ok(self)
    }

    /// Check if this is forward pagination.
    pub fn is_forward(&self) -> bool {
        self.before.is_none()
    }

    /// Check if this is backward pagination.
    pub fn is_backward(&self) -> bool {
        self.before.is_some()
    }

    /// Get the ORDER BY clause.
    pub fn order_by(&self) -> String {
        if self.is_backward() {
            // Reverse order for backward pagination
            self.builder
                .fields()
                .iter()
                .map(|f| format!("{} {}", f.name, f.direction.opposite().sql_keyword()))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            self.builder.order_by_clause()
        }
    }

    /// Get the WHERE clause and parameters for the cursor condition.
    pub fn cursor_condition(&self, params_offset: usize) -> (String, Vec<CursorValue>) {
        if let Some(ref cursor) = self.after {
            self.builder.where_after_clause(cursor, params_offset)
        } else if let Some(ref cursor) = self.before {
            self.builder.where_before_clause(cursor, params_offset)
        } else {
            (String::new(), Vec::new())
        }
    }

    /// Build cursors for the first and last items in a result set.
    pub fn build_edge_cursors<T: Serialize>(
        &self,
        items: &[T],
    ) -> Result<(Option<String>, Option<String>), ApexError> {
        if items.is_empty() {
            return Ok((None, None));
        }

        let start_cursor = self.builder.build_from(&items[0])?.encode()?;
        let end_cursor = self
            .builder
            .build_from(items.last().unwrap())?
            .encode()?;

        Ok((Some(start_cursor), Some(end_cursor)))
    }
}

impl Default for CursorPagination {
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

    #[derive(Debug, Serialize, Deserialize)]
    struct TestRecord {
        id: String,
        name: String,
        created_at: String,
        score: i64,
    }

    #[test]
    fn test_cursor_encode_decode() {
        let mut cursor = Cursor::new();
        cursor.add_value("id", "test-123");
        cursor.add_value("created_at", "2024-01-01T00:00:00Z");
        cursor.add_value("score", 42i64);

        let encoded = cursor.encode().unwrap();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(cursor, decoded);
    }

    #[test]
    fn test_cursor_builder() {
        let record = TestRecord {
            id: "abc-123".to_string(),
            name: "Test".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            score: 100,
        };

        let builder = CursorBuilder::new()
            .desc("created_at")
            .asc("id");

        let cursor = builder.build_from(&record).unwrap();

        assert_eq!(
            cursor.get_value("created_at"),
            Some(&CursorValue::Timestamp("2024-01-01T00:00:00Z".to_string()))
        );
        assert_eq!(
            cursor.get_value("id"),
            Some(&CursorValue::String("abc-123".to_string()))
        );
    }

    #[test]
    fn test_sort_direction() {
        assert_eq!(SortDirection::Asc.sql_keyword(), "ASC");
        assert_eq!(SortDirection::Desc.sql_keyword(), "DESC");
        assert_eq!(SortDirection::Asc.opposite(), SortDirection::Desc);
        assert_eq!(SortDirection::Asc.after_operator(), ">");
        assert_eq!(SortDirection::Desc.after_operator(), "<");
    }

    #[test]
    fn test_order_by_clause() {
        let builder = CursorBuilder::new()
            .desc("created_at")
            .asc("id");

        assert_eq!(builder.order_by_clause(), "created_at DESC, id ASC");
    }

    #[test]
    fn test_where_after_clause() {
        let builder = CursorBuilder::new()
            .desc("created_at")
            .asc("id");

        let mut cursor = Cursor::new();
        cursor.add_value("created_at", "2024-01-01T00:00:00Z");
        cursor.add_value("id", "abc-123");

        let (clause, values) = builder.where_after_clause(&cursor, 0);

        assert_eq!(clause, "(created_at, id) < ($1, $2)");
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_cursor_value_conversions() {
        let uuid = uuid::Uuid::new_v4();
        let ts = chrono::Utc::now();

        let cv_uuid: CursorValue = uuid.into();
        let cv_ts: CursorValue = ts.into();
        let cv_str: CursorValue = "test".into();
        let cv_int: CursorValue = 42i64.into();

        assert!(matches!(cv_uuid, CursorValue::Uuid(_)));
        assert!(matches!(cv_ts, CursorValue::Timestamp(_)));
        assert!(matches!(cv_str, CursorValue::String(_)));
        assert!(matches!(cv_int, CursorValue::Integer(42)));
    }

    #[test]
    fn test_cursor_pagination_directions() {
        let pagination = CursorPagination::new()
            .with_field("created_at", SortDirection::Desc)
            .with_field("id", SortDirection::Asc);

        assert!(pagination.is_forward());
        assert!(!pagination.is_backward());

        let mut cursor = Cursor::new();
        cursor.add_value("created_at", "2024-01-01T00:00:00Z");

        let backward = pagination.clone().before(cursor);
        assert!(backward.is_backward());
        assert!(!backward.is_forward());
    }

    #[test]
    fn test_cursor_with_metadata() {
        let mut cursor = Cursor::new();
        cursor.add_value("id", "test");
        cursor.add_metadata("shard", "shard-1");
        cursor.add_metadata("version", "2");

        let encoded = cursor.encode().unwrap();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(decoded.get_metadata("shard"), Some("shard-1"));
        assert_eq!(decoded.get_metadata("version"), Some("2"));
    }
}
