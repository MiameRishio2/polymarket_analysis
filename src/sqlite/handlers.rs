//! SQLite database API handlers

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, patch, post},
    Json, Router,
};
use rusqlite::OpenFlags;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path as StdPath;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

use super::models::*;

#[derive(Clone)]
pub struct SqliteState {
    pub db_path: String,
    pub conn: Arc<Mutex<rusqlite::Connection>>,
}

#[derive(serde::Serialize)]
pub struct ApiResponse<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(msg: String) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg),
        }
    }
}

// === Database Info Handlers ===

/// Get database statistics
pub async fn db_stats_handler(State(state): State<SqliteState>) -> Json<ApiResponse<DatabaseStats>> {
    match get_db_stats(&state) {
        Ok(stats) => Json(ApiResponse::success(stats)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Get list of all tables
pub async fn tables_handler(State(state): State<SqliteState>) -> Json<ApiResponse<Vec<TableInfo>>> {
    match get_tables(&state) {
        Ok(tables) => Json(ApiResponse::success(tables)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Get table schema (columns)
pub async fn table_schema_handler(
    Path(table_name): Path<String>,
    State(state): State<SqliteState>,
) -> Json<ApiResponse<Vec<ColumnInfo>>> {
    match get_table_schema(&state, &table_name) {
        Ok(columns) => Json(ApiResponse::success(columns)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Query parameters for table data
#[derive(Debug, Deserialize)]
pub struct TableQuery {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

/// Get table data with pagination
pub async fn table_data_handler(
    Path(table_name): Path<String>,
    Query(query): Query<TableQuery>,
    State(state): State<SqliteState>,
) -> Json<ApiResponse<TableData>> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).min(100).max(1);
    
    match get_table_data(&state, &table_name, page, page_size) {
        Ok(data) => Json(ApiResponse::success(data)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

// === Write Operations ===

/// Execute SQL statement
#[derive(Debug, Deserialize)]
pub struct SqlQuery {
    pub sql: String,
}

/// Execute a SQL query (SELECT only for safety)
pub async fn execute_query_handler(
    State(state): State<SqliteState>,
    Json(query): Json<SqlQuery>,
) -> Json<ApiResponse<QueryResult>> {
    // Security: only allow SELECT statements
    let sql_trimmed = query.sql.trim().to_uppercase();
    if !sql_trimmed.starts_with("SELECT") && !sql_trimmed.starts_with("PRAGMA") {
        return Json(ApiResponse::error(
            "Only SELECT and PRAGMA statements are allowed".to_string(),
        ));
    }
    
    match execute_query(&state, &query.sql) {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Request to delete a row
#[derive(Debug, Deserialize)]
pub struct DeleteRowRequest {
    pub conditions: HashMap<String, serde_json::Value>,
}

/// Delete rows from a table
pub async fn delete_row_handler(
    Path(table_name): Path<String>,
    State(state): State<SqliteState>,
    Json(request): Json<DeleteRowRequest>,
) -> Json<ApiResponse<DeleteResult>> {
    match delete_row(&state, &table_name, &request.conditions) {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Request to update a row
#[derive(Debug, Deserialize)]
pub struct UpdateRowRequest {
    pub set_values: HashMap<String, serde_json::Value>,
    pub conditions: HashMap<String, serde_json::Value>,
}

/// Update rows in a table
pub async fn update_row_handler(
    Path(table_name): Path<String>,
    State(state): State<SqliteState>,
    Json(request): Json<UpdateRowRequest>,
) -> Json<ApiResponse<UpdateResult>> {
    match update_row(&state, &table_name, &request.set_values, &request.conditions) {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Result of delete operation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeleteResult {
    pub deleted: usize,
}

/// Result of update operation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateResult {
    pub updated: usize,
}

// === Internal Functions ===

fn get_db_stats(state: &SqliteState) -> Result<DatabaseStats, rusqlite::Error> {
    let conn = state.conn.lock().unwrap();
    
    // Get file size
    let metadata = std::fs::metadata(&state.db_path).ok();
    let size_bytes = metadata.map(|m| m.len()).unwrap_or(0);
    let size_formatted = format_file_size(size_bytes);
    
    // Get table count
    let table_count: usize = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    
    // Get total row count
    let total_rows: usize = conn.query_row(
        "SELECT COUNT(*) FROM category_cache",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    
    Ok(DatabaseStats {
        path: state.db_path.clone(),
        size_bytes,
        size_formatted,
        table_count,
        total_rows,
    })
}

fn get_tables(state: &SqliteState) -> Result<Vec<TableInfo>, rusqlite::Error> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT name, type, sql FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )?;
    
    let tables = stmt.query_map([], |row| {
        Ok(TableInfo {
            name: row.get(0)?,
            table_type: row.get(1)?,
            sql: row.get::<_, Option<String>>(2)?,
        })
    })?;
    
    Ok(tables.filter_map(|r| r.ok()).collect())
}

fn get_table_schema(state: &SqliteState, table_name: &str) -> Result<Vec<ColumnInfo>, rusqlite::Error> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    
    let columns = stmt.query_map([], |row| {
        Ok(ColumnInfo {
            cid: row.get(0)?,
            name: row.get(1)?,
            column_type: row.get(2)?,
            notnull: row.get::<_, i32>(3)? != 0,
            dflt_value: row.get::<_, Option<String>>(4)?,
            pk: row.get::<_, i32>(5)? != 0,
        })
    })?;
    
    Ok(columns.filter_map(|r| r.ok()).collect())
}

fn get_table_data(
    state: &SqliteState,
    table_name: &str,
    page: usize,
    page_size: usize,
) -> Result<TableData, rusqlite::Error> {
    let conn = state.conn.lock().unwrap();
    
    // Get columns
    let columns = get_table_schema_internal(&conn, table_name)?;
    
    // Get total count
    let total_rows: usize = conn.query_row(
        &format!("SELECT COUNT(*) FROM {}", table_name),
        [],
        |row| row.get(0),
    )?;
    
    // Get paginated data
    let offset = (page - 1) * page_size;
    let mut stmt = conn.prepare(&format!(
        "SELECT * FROM {} LIMIT {} OFFSET {}",
        table_name, page_size, offset
    ))?;
    
    let column_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
    let rows = stmt.query_map([], |row| {
        let mut values = Vec::new();
        for i in 0..column_names.len() {
            let value = match row.get_ref(i) {
                Ok(rusqlite::types::ValueRef::Null) => serde_json::Value::Null,
                Ok(rusqlite::types::ValueRef::Integer(i)) => serde_json::json!(i),
                Ok(rusqlite::types::ValueRef::Real(f)) => serde_json::json!(f),
                Ok(rusqlite::types::ValueRef::Text(s)) => {
                    serde_json::json!(String::from_utf8_lossy(s))
                }
                Ok(rusqlite::types::ValueRef::Blob(b)) => {
                    serde_json::json!(format!("[BLOB: {} bytes]", b.len()))
                }
                Err(_) => serde_json::Value::Null,
            };
            values.push(value);
        }
        Ok(values)
    })?;
    
    let rows: Vec<Vec<serde_json::Value>> = rows.filter_map(|r| r.ok()).collect();
    
    Ok(TableData {
        table_name: table_name.to_string(),
        columns,
        rows,
        total_rows,
        page,
        page_size,
    })
}

fn get_table_schema_internal(conn: &rusqlite::Connection, table_name: &str) -> Result<Vec<ColumnInfo>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    
    let columns = stmt.query_map([], |row| {
        Ok(ColumnInfo {
            cid: row.get(0)?,
            name: row.get(1)?,
            column_type: row.get(2)?,
            notnull: row.get::<_, i32>(3)? != 0,
            dflt_value: row.get::<_, Option<String>>(4)?,
            pk: row.get::<_, i32>(5)? != 0,
        })
    })?;
    
    Ok(columns.filter_map(|r| r.ok()).collect())
}

fn execute_query(state: &SqliteState, sql: &str) -> Result<QueryResult, rusqlite::Error> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(sql)?;
    
    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let column_count = columns.len();
    
    let rows = stmt.query_map([], |row| {
        let mut values = Vec::new();
        for i in 0..column_count {
            let value = match row.get_ref(i) {
                Ok(rusqlite::types::ValueRef::Null) => serde_json::Value::Null,
                Ok(rusqlite::types::ValueRef::Integer(i)) => serde_json::json!(i),
                Ok(rusqlite::types::ValueRef::Real(f)) => serde_json::json!(f),
                Ok(rusqlite::types::ValueRef::Text(s)) => {
                    serde_json::json!(String::from_utf8_lossy(s))
                }
                Ok(rusqlite::types::ValueRef::Blob(b)) => {
                    serde_json::json!(format!("[BLOB: {} bytes]", b.len()))
                }
                Err(_) => serde_json::Value::Null,
            };
            values.push(value);
        }
        Ok(values)
    })?;
    
    let rows: Vec<Vec<serde_json::Value>> = rows.filter_map(|r| r.ok()).collect();
    let row_count = rows.len();
    
    Ok(QueryResult {
        columns,
        rows,
        row_count,
    })
}

/// Convert serde_json::Value to a type that implements ToSql
fn json_to_sqlite(value: &serde_json::Value) -> rusqlite::types::Value {
    match value {
        serde_json::Value::Null => rusqlite::types::Value::Null,
        serde_json::Value::Bool(b) => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                rusqlite::types::Value::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        serde_json::Value::Array(arr) => rusqlite::types::Value::Text(serde_json::to_string(arr).unwrap_or_default()),
        serde_json::Value::Object(obj) => rusqlite::types::Value::Text(serde_json::to_string(obj).unwrap_or_default()),
    }
}

fn delete_row(
    state: &SqliteState,
    table_name: &str,
    conditions: &HashMap<String, serde_json::Value>,
) -> Result<DeleteResult, rusqlite::Error> {
    if conditions.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "At least one condition is required".to_string(),
        ));
    }
    
    let conn = state.conn.lock().unwrap();
    
    // Build WHERE clause
    let where_parts: Vec<String> = conditions
        .keys()
        .map(|k| format!("{} = ?", k))
        .collect();
    let where_clause = where_parts.join(" AND ");
    let sql = format!("DELETE FROM {} WHERE {}", table_name, where_clause);
    
    // Bind parameters
    let params: Vec<rusqlite::types::Value> = conditions
        .values()
        .map(json_to_sqlite)
        .collect();
    
    let deleted = conn.execute(&sql, rusqlite::params_from_iter(&params))?;
    
    Ok(DeleteResult { deleted })
}

fn update_row(
    state: &SqliteState,
    table_name: &str,
    set_values: &HashMap<String, serde_json::Value>,
    conditions: &HashMap<String, serde_json::Value>,
) -> Result<UpdateResult, rusqlite::Error> {
    if set_values.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "At least one SET value is required".to_string(),
        ));
    }
    if conditions.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "At least one condition is required".to_string(),
        ));
    }
    
    let conn = state.conn.lock().unwrap();
    
    // Build SET clause
    let set_parts: Vec<String> = set_values
        .keys()
        .map(|k| format!("{} = ?", k))
        .collect();
    let set_clause = set_parts.join(", ");
    
    // Build WHERE clause
    let where_parts: Vec<String> = conditions
        .keys()
        .map(|k| format!("{} = ?", k))
        .collect();
    let where_clause = where_parts.join(" AND ");
    
    let sql = format!(
        "UPDATE {} SET {} WHERE {}",
        table_name, set_clause, where_clause
    );
    
    // Bind parameters: SET values first, then conditions
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    for v in set_values.values() {
        params.push(json_to_sqlite(v));
    }
    for v in conditions.values() {
        params.push(json_to_sqlite(v));
    }
    
    let updated = conn.execute(&sql, rusqlite::params_from_iter(&params))?;
    
    Ok(UpdateResult { updated })
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn create_sqlite_router(db_path: String) -> Router {
    // Open database with read-write access
    let conn = rusqlite::Connection::open_with_flags(
        StdPath::new(&db_path),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    ).expect("Failed to open SQLite database");
    
    let state = SqliteState {
        db_path,
        conn: Arc::new(Mutex::new(conn)),
    };
    
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);
    
    Router::new()
        .route("/api/sqlite/stats", get(db_stats_handler))
        .route("/api/sqlite/tables", get(tables_handler))
        .route("/api/sqlite/schema/:table", get(table_schema_handler))
        .route("/api/sqlite/data/:table", get(table_data_handler))
        .route("/api/sqlite/query", post(execute_query_handler))
        .route("/api/sqlite/data/:table", delete(delete_row_handler))
        .route("/api/sqlite/data/:table", patch(update_row_handler))
        .with_state(state)
        .layer(cors)
}
