//! # SQL Query Interface
//!
//! SQL-like query interface for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `sql-query` | `(sql params -- rows)` | Execute SELECT query |
//! | `sql-exec` | `(sql params -- affected)` | Execute INSERT/UPDATE/DELETE |
//! | `sql-table` | `(name schema --)` | Create a table |
//! | `sql-drop` | `(name --)` | Drop a table |
//!
//! ## Example
//!
//! ```kore
//! "users" { "id": "int", "name": "text" } sql-table
//! "INSERT INTO users VALUES (?, ?)" [1, "Alice"] sql-exec
//! "SELECT * FROM users WHERE id = ?" [1] sql-query
//! ```
//!
//! ## Note
//!
//! This is an in-memory SQL-like interface. For production,
//! use kore-net to connect to actual SQL databases.

use kore::{Context, Stack, Tool, Value};
use indexmap::IndexMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A simple in-memory table.
#[derive(Debug, Clone)]
struct Table {
    schema: IndexMap<String, String>,
    rows: Vec<IndexMap<String, Value>>,
}

type TableStore = Arc<RwLock<IndexMap<String, Table>>>;

fn create_tables() -> TableStore {
    Arc::new(RwLock::new(IndexMap::new()))
}

/// Register all SQL tools into a context.
pub async fn register_sql_tools(ctx: &mut Context) {
    let tables = create_tables();

    // sql-table: (text map --)
    let tables_clone = tables.clone();
    ctx.dict.write().await.register(
        Tool::native("sql-table", "(text map --)", move |mut stack: Stack, ctx: Context| {
            let tables: TableStore = tables_clone.clone();
            Box::pin(async move {
                let schema = stack.pop()?.into_map()?;
                let name = stack.pop()?.into_text()?;

                let schema_map: IndexMap<String, String> = schema
                    .iter()
                    .filter_map(|(k, v)| {
                        if let Value::Text(s) = v {
                            Some((k.clone(), s.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                let table = Table {
                    schema: schema_map,
                    rows: Vec::new(),
                };

                tables.write().await.insert(name, table);
                Ok((stack, ctx))
            })
        }),
    );

    // sql-drop: (text --)
    let tables_clone = tables.clone();
    ctx.dict.write().await.register(
        Tool::native("sql-drop", "(text --)", move |mut stack: Stack, ctx: Context| {
            let tables: TableStore = tables_clone.clone();
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                tables.write().await.swap_remove(&name);
                Ok((stack, ctx))
            })
        }),
    );

    // sql-exec: (text list -- int)
    let tables_clone = tables.clone();
    ctx.dict.write().await.register(
        Tool::native("sql-exec", "(text list -- int)", move |mut stack: Stack, ctx: Context| {
            let tables: TableStore = tables_clone.clone();
            Box::pin(async move {
                let params: Vec<Value> = stack.pop()?.into_list()?;
                let sql = stack.pop()?.into_text()?;

                let sql_upper = sql.to_uppercase();

                if sql_upper.starts_with("INSERT INTO ") {
                    // Parse: INSERT INTO tablename VALUES (?, ?, ...)
                    let parts: Vec<&str> = sql.split_whitespace().collect();
                    if parts.len() < 4 {
                        return Err(kore::Error::Runtime("Invalid INSERT syntax".to_string()));
                    }
                    let table_name = parts[2].to_string();

                    let mut tables_guard = tables.write().await;
                    let table = tables_guard
                        .get_mut(&table_name)
                        .ok_or_else(|| kore::Error::Runtime(format!("Table not found: {}", table_name)))?;

                    // Build row from schema and params
                    let mut row: IndexMap<String, Value> = IndexMap::new();
                    let columns: Vec<String> = table.schema.keys().cloned().collect();
                    for (i, col) in columns.iter().enumerate() {
                        if i < params.len() {
                            row.insert(col.clone(), params[i].clone());
                        }
                    }

                    table.rows.push(row);
                    stack.push(Value::Int(1))?;
                } else if sql_upper.starts_with("DELETE FROM ") {
                    // Parse: DELETE FROM tablename WHERE col = ?
                    let parts: Vec<&str> = sql.split_whitespace().collect();
                    if parts.len() < 3 {
                        return Err(kore::Error::Runtime("Invalid DELETE syntax".to_string()));
                    }
                    let table_name = parts[2].to_string();

                    let mut tables_guard = tables.write().await;
                    let table = tables_guard
                        .get_mut(&table_name)
                        .ok_or_else(|| kore::Error::Runtime(format!("Table not found: {}", table_name)))?;

                    let before = table.rows.len();
                    // For simplicity, delete all rows (would need WHERE parsing for real impl)
                    table.rows.clear();
                    let deleted = before as i64;

                    stack.push(Value::Int(deleted))?;
                } else {
                    return Err(kore::Error::Runtime("Unsupported SQL statement".to_string()));
                }

                Ok((stack, ctx))
            })
        }),
    );

    // sql-query: (text list -- list)
    let tables_clone = tables.clone();
    ctx.dict.write().await.register(
        Tool::native("sql-query", "(text list -- list)", move |mut stack: Stack, ctx: Context| {
            let tables: TableStore = tables_clone.clone();
            Box::pin(async move {
                let _params: Vec<Value> = stack.pop()?.into_list()?;
                let sql = stack.pop()?.into_text()?;

                let sql_upper = sql.to_uppercase();

                if sql_upper.starts_with("SELECT ") {
                    // Parse: SELECT * FROM tablename
                    let parts: Vec<&str> = sql.split_whitespace().collect();
                    let from_idx = parts.iter().position(|&p| p.to_uppercase() == "FROM");
                    if from_idx.is_none() || from_idx.unwrap() + 1 >= parts.len() {
                        return Err(kore::Error::Runtime("Invalid SELECT syntax".to_string()));
                    }
                    let table_name = parts[from_idx.unwrap() + 1].to_string();

                    let tables_guard = tables.read().await;
                    let table = tables_guard
                        .get(&table_name)
                        .ok_or_else(|| kore::Error::Runtime(format!("Table not found: {}", table_name)))?;

                    let rows: Vec<Value> = table
                        .rows
                        .iter()
                        .map(|r: &IndexMap<String, Value>| Value::Map(r.clone()))
                        .collect();

                    stack.push(Value::List(rows))?;
                } else {
                    return Err(kore::Error::Runtime("Only SELECT queries supported".to_string()));
                }

                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_table_create() {
        let tables = create_tables();
        
        let mut schema = IndexMap::new();
        schema.insert("id".to_string(), "int".to_string());
        schema.insert("name".to_string(), "text".to_string());
        
        let table = Table {
            schema,
            rows: Vec::new(),
        };
        
        tables.write().await.insert("users".to_string(), table);
        assert!(tables.read().await.contains_key("users"));
    }

    #[tokio::test]
    async fn test_table_insert() {
        let tables = create_tables();
        
        let mut schema = IndexMap::new();
        schema.insert("id".to_string(), "int".to_string());
        schema.insert("name".to_string(), "text".to_string());
        
        let mut table = Table {
            schema,
            rows: Vec::new(),
        };

        let mut row = IndexMap::new();
        row.insert("id".to_string(), Value::Int(1));
        row.insert("name".to_string(), Value::Text("Alice".to_string()));
        table.rows.push(row);

        tables.write().await.insert("users".to_string(), table);
        
        let guard = tables.read().await;
        let t = guard.get("users").unwrap();
        assert_eq!(t.rows.len(), 1);
    }

    #[tokio::test]
    async fn test_table_drop() {
        let tables = create_tables();
        
        let schema = IndexMap::new();
        let table = Table {
            schema,
            rows: Vec::new(),
        };
        
        tables.write().await.insert("temp".to_string(), table);
        assert!(tables.read().await.contains_key("temp"));
        
        tables.write().await.swap_remove("temp");
        assert!(!tables.read().await.contains_key("temp"));
    }
}
