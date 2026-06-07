//! SQLite database models

/// Database table info
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub table_type: String,
    pub sql: Option<String>,
}

/// Column info for a table
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnInfo {
    pub cid: i32,
    pub name: String,
    pub column_type: String,
    pub notnull: bool,
    pub dflt_value: Option<String>,
    pub pk: bool,
}

/// Database statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseStats {
    pub path: String,
    pub size_bytes: u64,
    pub size_formatted: String,
    pub table_count: usize,
    pub total_rows: usize,
}

/// Query result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
}

/// Table data with pagination
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableData {
    pub table_name: String,
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total_rows: usize,
    pub page: usize,
    pub page_size: usize,
}
