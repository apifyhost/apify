//! Schema generator for dynamic table creation from OpenAPI specifications

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Table schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnDefinition>,
    pub indexes: Vec<IndexDefinition>,
}

/// Column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    pub name: String,
    pub column_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub auto_increment: bool,
    pub default_value: Option<String>,
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

pub struct SchemaGenerator;

impl SchemaGenerator {
    /// Extract table schemas from OpenAPI specification
    pub fn extract_schemas_from_openapi(spec: &Value) -> Result<Vec<TableSchema>, Box<dyn std::error::Error + Send + Sync>> {
        let mut schemas = Vec::new();

        // Look for x-table-schema extensions in the OpenAPI spec
        if let Some(extensions) = spec.get("x-table-schemas").and_then(|v| v.as_array()) {
            for schema_value in extensions {
                let schema: TableSchema = serde_json::from_value(schema_value.clone())?;
                schemas.push(schema);
            }
        }

        // Also try to extract from paths (alternative approach)
        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            for (_path, path_item) in paths.iter() {
                if let Some(table_schema) = path_item.get("x-table-schema") {
                    let schema: TableSchema = serde_json::from_value(table_schema.clone())?;
                    // Avoid duplicates
                    if !schemas.iter().any(|s| s.table_name == schema.table_name) {
                        schemas.push(schema);
                    }
                }
            }
        }

        Ok(schemas)
    }

    /// Generate CREATE TABLE SQL statement for SQLite
    pub fn generate_create_table_sql_sqlite(schema: &TableSchema) -> String {
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", schema.table_name);
        
        let mut column_defs = Vec::new();
        for col in &schema.columns {
            let mut col_def = format!("    {} {}", col.name, Self::map_type_to_sqlite(&col.column_type));
            
            if col.primary_key {
                col_def.push_str(" PRIMARY KEY");
                if col.auto_increment {
                    col_def.push_str(" AUTOINCREMENT");
                }
            }
            
            if !col.nullable && !col.primary_key {
                col_def.push_str(" NOT NULL");
            }
            
            if col.unique && !col.primary_key {
                col_def.push_str(" UNIQUE");
            }
            
            if let Some(default) = &col.default_value {
                col_def.push_str(&format!(" DEFAULT {}", default));
            }
            
            column_defs.push(col_def);
        }
        
        sql.push_str(&column_defs.join(",\n"));
        sql.push_str("\n);\n");
        
        // Generate index statements
        for index in &schema.indexes {
            let index_type = if index.unique { "UNIQUE INDEX" } else { "INDEX" };
            sql.push_str(&format!(
                "CREATE {} IF NOT EXISTS {} ON {} ({});\n",
                index_type,
                index.name,
                schema.table_name,
                index.columns.join(", ")
            ));
        }
        
        sql
    }

    /// Generate CREATE TABLE SQL statement for PostgreSQL
    pub fn generate_create_table_sql_postgres(schema: &TableSchema) -> String {
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", schema.table_name);
        
        let mut column_defs = Vec::new();
        for col in &schema.columns {
            let mut col_def = format!("    {} {}", col.name, Self::map_type_to_postgres(&col.column_type));
            
            if col.primary_key {
                // For PostgreSQL, SERIAL already includes NOT NULL
                if col.auto_increment && Self::is_integer_type(&col.column_type) {
                    // Already handled by SERIAL type
                } else {
                    col_def.push_str(" PRIMARY KEY");
                }
            }
            
            if !col.nullable && !col.primary_key {
                col_def.push_str(" NOT NULL");
            }
            
            if col.unique && !col.primary_key {
                col_def.push_str(" UNIQUE");
            }
            
            if let Some(default) = &col.default_value {
                col_def.push_str(&format!(" DEFAULT {}", default));
            }
            
            column_defs.push(col_def);
        }
        
        sql.push_str(&column_defs.join(",\n"));
        sql.push_str("\n);\n");
        
        // Generate index statements
        for index in &schema.indexes {
            let index_type = if index.unique { "UNIQUE INDEX" } else { "INDEX" };
            sql.push_str(&format!(
                "CREATE {} IF NOT EXISTS {} ON {} ({});\n",
                index_type,
                index.name,
                schema.table_name,
                index.columns.join(", ")
            ));
        }
        
        sql
    }

    /// Map generic type to SQLite type
    fn map_type_to_sqlite(type_name: &str) -> &str {
        match type_name.to_lowercase().as_str() {
            "integer" | "int" | "bigint" | "smallint" => "INTEGER",
            "text" | "string" | "varchar" | "char" => "TEXT",
            "real" | "float" | "double" | "decimal" | "numeric" => "REAL",
            "boolean" | "bool" => "INTEGER", // SQLite uses INTEGER for boolean
            "blob" | "binary" => "BLOB",
            "datetime" | "timestamp" => "DATETIME",
            "date" => "DATE",
            "time" => "TIME",
            _ => "TEXT", // Default to TEXT for unknown types
        }
    }

    /// Map generic type to PostgreSQL type
    fn map_type_to_postgres(type_name: &str) -> String {
        match type_name.to_lowercase().as_str() {
            "integer" | "int" => "INTEGER".to_string(),
            "bigint" => "BIGINT".to_string(),
            "smallint" => "SMALLINT".to_string(),
            "text" | "string" => "TEXT".to_string(),
            "varchar" => "VARCHAR(255)".to_string(),
            "char" => "CHAR(1)".to_string(),
            "real" | "float" => "REAL".to_string(),
            "double" => "DOUBLE PRECISION".to_string(),
            "decimal" | "numeric" => "NUMERIC".to_string(),
            "boolean" | "bool" => "BOOLEAN".to_string(),
            "blob" | "binary" => "BYTEA".to_string(),
            "datetime" | "timestamp" => "TIMESTAMP".to_string(),
            "date" => "DATE".to_string(),
            "time" => "TIME".to_string(),
            _ => "TEXT".to_string(), // Default to TEXT for unknown types
        }
    }

    /// Check if a type is an integer type
    fn is_integer_type(type_name: &str) -> bool {
        matches!(
            type_name.to_lowercase().as_str(),
            "integer" | "int" | "bigint" | "smallint"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_create_table_sql_sqlite() {
        let schema = TableSchema {
            table_name: "users".to_string(),
            columns: vec![
                ColumnDefinition {
                    name: "id".to_string(),
                    column_type: "INTEGER".to_string(),
                    nullable: false,
                    primary_key: true,
                    unique: false,
                    auto_increment: true,
                    default_value: None,
                },
                ColumnDefinition {
                    name: "name".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                },
                ColumnDefinition {
                    name: "email".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: true,
                    auto_increment: false,
                    default_value: None,
                },
            ],
            indexes: vec![
                IndexDefinition {
                    name: "idx_users_email".to_string(),
                    columns: vec!["email".to_string()],
                    unique: false,
                },
            ],
        };

        let sql = SchemaGenerator::generate_create_table_sql_sqlite(&schema);
        println!("{}", sql);
        
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS users"));
        assert!(sql.contains("id INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(sql.contains("name TEXT NOT NULL"));
        assert!(sql.contains("email TEXT NOT NULL UNIQUE"));
        assert!(sql.contains("CREATE INDEX IF NOT EXISTS idx_users_email"));
    }
}
