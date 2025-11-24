//! Schema generator for dynamic table creation from OpenAPI specifications

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Table schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnDefinition>,
    pub indexes: Vec<IndexDefinition>,
    #[serde(default)]
    pub relations: Vec<RelationDefinition>,
}

/// Relation definition for nested object support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationDefinition {
    pub field_name: String,          // Property name in the schema (e.g., "items")
    pub relation_type: RelationType, // hasMany, belongsTo, hasOne, belongsToMany
    pub target_table: String,        // Related table name
    pub foreign_key: String,         // Foreign key column name
    pub local_key: Option<String>,   // Local key (default: "id")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RelationType {
    HasMany,
    BelongsTo,
    HasOne,
    BelongsToMany, // For future many-to-many support
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
    #[serde(default)]
    pub auto_field: bool, // For audit fields like createdBy, updatedBy
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
    pub fn extract_schemas_from_openapi(
        spec: &Value,
    ) -> Result<Vec<TableSchema>, Box<dyn std::error::Error + Send + Sync>> {
        use std::io::Write;
        eprintln!("    extract_schemas_from_openapi: START");
        let _ = std::io::stderr().flush();
        let mut schemas = Vec::new();

        tracing::debug!(
            has_x_table_schemas = spec.get("x-table-schemas").is_some(),
            has_paths = spec.get("paths").is_some(),
            "Starting schema extraction from OpenAPI spec"
        );
        eprintln!("    extract_schemas_from_openapi: After debug log");
        let _ = std::io::stderr().flush();

        // Look for x-table-schema extensions in the OpenAPI spec
        eprintln!("    extract_schemas_from_openapi: Checking x-table-schemas");
        let _ = std::io::stderr().flush();
        if let Some(extensions) = spec.get("x-table-schemas").and_then(|v| v.as_array()) {
            eprintln!(
                "    extract_schemas_from_openapi: Found {} x-table-schemas",
                extensions.len()
            );
            let _ = std::io::stderr().flush();
            tracing::debug!(schemas_count = extensions.len(), "Found x-table-schemas");
            for schema_value in extensions {
                eprintln!("    extract_schemas_from_openapi: Parsing schema...");
                let _ = std::io::stderr().flush();
                let schema: TableSchema = serde_json::from_value(schema_value.clone())?;
                eprintln!(
                    "    extract_schemas_from_openapi: Parsed schema: {}",
                    schema.table_name
                );
                let _ = std::io::stderr().flush();
                tracing::debug!(
                    table = %schema.table_name,
                    columns_count = schema.columns.len(),
                    relations_count = schema.relations.len(),
                    "Loaded table schema from x-table-schemas"
                );
                schemas.push(schema);
            }
        } else {
            eprintln!("    extract_schemas_from_openapi: NO x-table-schemas found");
            let _ = std::io::stderr().flush();
        }
        eprintln!(
            "    extract_schemas_from_openapi: After x-table-schemas, schemas.len()={}",
            schemas.len()
        );
        let _ = std::io::stderr().flush();

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

        // Extract relations from paths (requestBody and responses)
        eprintln!("    extract_schemas_from_openapi: About to extract relations from paths");
        let _ = std::io::stderr().flush();
        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            eprintln!(
                "    extract_schemas_from_openapi: Calling extract_relations_from_paths with {} paths",
                paths.len()
            );
            let _ = std::io::stderr().flush();
            Self::extract_relations_from_paths(&mut schemas, paths);
            eprintln!("    extract_schemas_from_openapi: After extract_relations_from_paths");
            let _ = std::io::stderr().flush();
        }
        eprintln!("    extract_schemas_from_openapi: After relations extraction");
        let _ = std::io::stderr().flush();

        // Fallback: derive from components.schemas if no explicit schemas found
        eprintln!(
            "    extract_schemas_from_openapi: Checking fallback, schemas.len()={}",
            schemas.len()
        );
        let _ = std::io::stderr().flush();
        if schemas.is_empty()
            && let Some(derived) = Self::derive_from_components(spec)
            && !derived.is_empty()
        {
            eprintln!("    extract_schemas_from_openapi: Using fallback derived schemas");
            return Ok(derived);
        }

        eprintln!(
            "    extract_schemas_from_openapi: DONE, returning {} schemas",
            schemas.len()
        );
        let _ = std::io::stderr().flush();
        Ok(schemas)
    }

    /// Extract relation definitions from API paths and merge into table schemas
    fn extract_relations_from_paths(
        schemas: &mut [TableSchema],
        paths: &serde_json::Map<String, Value>,
    ) {
        use std::io::Write;
        eprintln!(
            "      [extract_relations_from_paths] START: {} schemas, {} paths",
            schemas.len(),
            paths.len()
        );
        let _ = std::io::stderr().flush();
        tracing::debug!(
            schema_count = schemas.len(),
            paths_count = paths.len(),
            "Extracting relations from paths"
        );

        let mut operations_found = 0;
        let mut relations_found = 0;

        for (path_str, path_item) in paths.iter() {
            if let Some(path_obj) = path_item.as_object() {
                for (method, operation) in path_obj.iter() {
                    if let Some(op_obj) = operation.as_object() {
                        // Get table name from operation
                        let table_name = match op_obj.get("x-table-name").and_then(|v| v.as_str()) {
                            Some(name) => name.to_string(),
                            None => continue,
                        };

                        operations_found += 1;
                        eprintln!(
                            "      [extract_relations_from_paths] Found operation #{}: {} {} -> table '{}'",
                            operations_found, method, path_str, table_name
                        );
                        let _ = std::io::stderr().flush();

                        tracing::debug!(
                            path = %path_str,
                            method = %method,
                            table = %table_name,
                            "Found operation with table name"
                        );

                        // Extract relations from requestBody schema
                        if let Some(request_body) = op_obj.get("requestBody") {
                            eprintln!(
                                "      [extract_relations_from_paths] Checking requestBody for table '{}'",
                                table_name
                            );
                            let _ = std::io::stderr().flush();
                            let before_count: usize =
                                schemas.iter().map(|s| s.relations.len()).sum();
                            tracing::debug!(
                                table = %table_name,
                                "Extracting relations from requestBody"
                            );
                            Self::extract_relations_from_schema(schemas, &table_name, request_body);
                            let after_count: usize =
                                schemas.iter().map(|s| s.relations.len()).sum();
                            let new_relations = after_count - before_count;
                            if new_relations > 0 {
                                eprintln!(
                                    "      [extract_relations_from_paths] Found {} new relation(s) in requestBody",
                                    new_relations
                                );
                                let _ = std::io::stderr().flush();
                                relations_found += new_relations;
                            }
                        }

                        // Extract relations from response schema
                        if let Some(responses) = op_obj.get("responses").and_then(|r| r.as_object())
                        {
                            eprintln!(
                                "      [extract_relations_from_paths] Checking {} response(s) for table '{}'",
                                responses.len(),
                                table_name
                            );
                            let _ = std::io::stderr().flush();
                            for (status, response) in responses.iter() {
                                eprintln!(
                                    "      [extract_relations_from_paths] Processing response {} for table '{}'",
                                    status, table_name
                                );
                                let _ = std::io::stderr().flush();
                                tracing::debug!(
                                    table = %table_name,
                                    status = %status,
                                    "Extracting relations from response"
                                );
                                Self::extract_relations_from_schema(schemas, &table_name, response);
                                eprintln!(
                                    "      [extract_relations_from_paths] Finished processing response {} for table '{}'",
                                    status, table_name
                                );
                                let _ = std::io::stderr().flush();
                            }
                            eprintln!(
                                "      [extract_relations_from_paths] Finished all responses for table '{}'",
                                table_name
                            );
                            let _ = std::io::stderr().flush();
                        }
                        eprintln!(
                            "      [extract_relations_from_paths] Finished operation #{}",
                            operations_found
                        );
                        let _ = std::io::stderr().flush();
                    }
                }
            }
        }

        eprintln!(
            "      [extract_relations_from_paths] DONE: processed {} operations, found {} relations",
            operations_found, relations_found
        );
        for schema in schemas.iter() {
            if !schema.relations.is_empty() {
                eprintln!(
                    "      [extract_relations_from_paths] Table '{}' has {} relation(s): {:?}",
                    schema.table_name,
                    schema.relations.len(),
                    schema
                        .relations
                        .iter()
                        .map(|r| &r.field_name)
                        .collect::<Vec<_>>()
                );
            }
        }
        let _ = std::io::stderr().flush();

        tracing::debug!(
            schemas = ?schemas.iter().map(|s| (&s.table_name, s.relations.len())).collect::<Vec<_>>(),
            "Finished extracting relations"
        );
    }

    /// Extract relation definitions from a schema object
    fn extract_relations_from_schema(
        schemas: &mut [TableSchema],
        table_name: &str,
        schema_container: &Value,
    ) {
        use std::io::Write;
        // Navigate to the actual schema (might be in content.application/json.schema)
        let schema = if let Some(content) = schema_container.get("content") {
            content
                .get("application/json")
                .and_then(|v| v.get("schema"))
                .unwrap_or(schema_container)
        } else if let Some(schema) = schema_container.get("schema") {
            schema
        } else {
            schema_container
        };

        let has_properties = schema.get("properties").is_some();
        eprintln!(
            "        [extract_relations_from_schema] Scanning table '{}', has_properties={}",
            table_name, has_properties
        );
        let _ = std::io::stderr().flush();

        tracing::debug!(
            table = %table_name,
            has_properties = schema.get("properties").is_some(),
            "Extracting relations from schema"
        );

        // Extract properties with x-relation
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            eprintln!(
                "        [extract_relations_from_schema] Found {} properties in table '{}'",
                props.len(),
                table_name
            );
            let _ = std::io::stderr().flush();
            let mut new_relations = Vec::new();

            for (prop_name, prop_schema) in props.iter() {
                if let Some(relation_obj) =
                    prop_schema.get("x-relation").and_then(|r| r.as_object())
                {
                    eprintln!(
                        "        [extract_relations_from_schema] Found x-relation on property '{}' in table '{}'",
                        prop_name, table_name
                    );
                    let _ = std::io::stderr().flush();
                    tracing::debug!(
                        table = %table_name,
                        field = %prop_name,
                        relation_def = ?relation_obj,
                        "Found x-relation"
                    );

                    let relation_type =
                        relation_obj
                            .get("type")
                            .and_then(|t| t.as_str())
                            .and_then(|t| match t {
                                "hasMany" => Some(RelationType::HasMany),
                                "belongsTo" => Some(RelationType::BelongsTo),
                                "hasOne" => Some(RelationType::HasOne),
                                "belongsToMany" => Some(RelationType::BelongsToMany),
                                _ => None,
                            });

                    let target_table = relation_obj
                        .get("target")
                        .and_then(|t| t.as_str())
                        .map(Self::to_table_name);

                    let foreign_key = relation_obj
                        .get("foreignKey")
                        .and_then(|fk| fk.as_str())
                        .map(|s| s.to_string());

                    let local_key = relation_obj
                        .get("localKey")
                        .and_then(|lk| lk.as_str())
                        .map(|s| s.to_string());

                    if let (Some(rel_type), Some(target), Some(fk)) =
                        (relation_type, target_table, foreign_key)
                    {
                        eprintln!(
                            "        [extract_relations_from_schema] Adding relation: {}.{} -> {} (type={:?}, fk={})",
                            table_name, prop_name, target, rel_type, fk
                        );
                        let _ = std::io::stderr().flush();
                        tracing::info!(
                            table = %table_name,
                            field = %prop_name,
                            relation_type = ?rel_type,
                            target = %target,
                            foreign_key = %fk,
                            "Adding relation"
                        );

                        new_relations.push(RelationDefinition {
                            field_name: prop_name.clone(),
                            relation_type: rel_type,
                            target_table: target,
                            foreign_key: fk,
                            local_key,
                        });
                    }
                }
            }

            // Merge relations into the corresponding table schema
            if !new_relations.is_empty()
                && let Some(table_schema) = schemas.iter_mut().find(|s| s.table_name == table_name)
            {
                eprintln!(
                    "        [extract_relations_from_schema] Merging {} new relation(s) into table '{}'",
                    new_relations.len(),
                    table_name
                );
                let _ = std::io::stderr().flush();
                tracing::info!(
                    table = %table_name,
                    new_relations_count = new_relations.len(),
                    "Merging relations into table schema"
                );

                // Add new relations, avoiding duplicates
                for new_rel in new_relations {
                    if !table_schema
                        .relations
                        .iter()
                        .any(|r| r.field_name == new_rel.field_name)
                    {
                        table_schema.relations.push(new_rel);
                    }
                }
                eprintln!(
                    "        [extract_relations_from_schema] Table '{}' now has {} total relation(s)",
                    table_name,
                    table_schema.relations.len()
                );
                let _ = std::io::stderr().flush();
            } else if !new_relations.is_empty() {
                eprintln!(
                    "        [extract_relations_from_schema] WARNING: Table '{}' not found in schemas, cannot add {} relation(s)",
                    table_name,
                    new_relations.len()
                );
                eprintln!(
                    "        [extract_relations_from_schema] Available tables: {:?}",
                    schemas.iter().map(|s| &s.table_name).collect::<Vec<_>>()
                );
                let _ = std::io::stderr().flush();
                tracing::warn!(
                    table = %table_name,
                    relations_count = new_relations.len(),
                    available_tables = ?schemas.iter().map(|s| &s.table_name).collect::<Vec<_>>(),
                    "Could not find table schema to merge relations"
                );
            }
        } else {
            eprintln!(
                "        [extract_relations_from_schema] No properties found in schema for table '{}'",
                table_name
            );
            let _ = std::io::stderr().flush();
        }
    }

    /// Derive table schemas from OpenAPI components.schemas (best-effort)
    /// Rules:
    /// - Each object schema becomes a table, table name = lowercased schema name + 's' (if not ending with 's')
    /// - Properties map to columns; required => NOT NULL; infer basic SQL types
    /// - Ensure an 'id INTEGER PRIMARY KEY AUTOINCREMENT' if not present
    /// - Special cases: created_at -> DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
    /// - Extensions: x-unique => UNIQUE; x-index => create index
    fn derive_from_components(spec: &Value) -> Option<Vec<TableSchema>> {
        let components = spec.get("components")?.get("schemas")?.as_object()?;
        let mut tables = Vec::new();

        for (schema_name, schema_value) in components.iter() {
            let obj = match schema_value.as_object() {
                Some(o) => o,
                None => continue,
            };

            // Only process object schemas or those with properties
            let is_object = obj
                .get("type")
                .and_then(|t| t.as_str())
                .map(|t| t == "object")
                .unwrap_or(false)
                || obj.get("properties").is_some();
            if !is_object {
                continue;
            }

            let mut columns: Vec<ColumnDefinition> = Vec::new();
            let mut indexes: Vec<IndexDefinition> = Vec::new();

            let required: std::collections::HashSet<String> = obj
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_schema) in props.iter() {
                    // special-case primary key id
                    if prop_name == "id" {
                        columns.push(ColumnDefinition {
                            name: prop_name.clone(),
                            column_type: "INTEGER".to_string(),
                            nullable: false,
                            primary_key: true,
                            unique: false,
                            auto_increment: true,
                            default_value: None,
                            auto_field: false,
                        });
                        continue;
                    }

                    // infer type
                    let (col_type, default_value) =
                        Self::infer_sql_type_and_default(prop_name, prop_schema);
                    let nullable = !required.contains(prop_name);

                    let unique = prop_schema
                        .as_object()
                        .and_then(|o| o.get("x-unique"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    // Check for auto-field markers (x-auto-field or readOnly)
                    let auto_field = prop_schema
                        .as_object()
                        .and_then(|o| o.get("x-auto-field"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        || prop_schema
                            .as_object()
                            .and_then(|o| o.get("readOnly"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                    // indexes via x-index
                    let index = prop_schema
                        .as_object()
                        .and_then(|o| o.get("x-index"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    columns.push(ColumnDefinition {
                        name: prop_name.clone(),
                        column_type: col_type,
                        nullable,
                        primary_key: false,
                        unique,
                        auto_increment: false,
                        default_value,
                        auto_field,
                    });

                    if index {
                        indexes.push(IndexDefinition {
                            name: format!("idx_{}_{}", Self::to_table_name(schema_name), prop_name),
                            columns: vec![prop_name.clone()],
                            unique: false,
                        });
                    }
                }
            }

            // Ensure id column exists
            if !columns.iter().any(|c| c.primary_key) {
                columns.insert(
                    0,
                    ColumnDefinition {
                        name: "id".to_string(),
                        column_type: "INTEGER".to_string(),
                        nullable: false,
                        primary_key: true,
                        unique: false,
                        auto_increment: true,
                        default_value: None,
                        auto_field: false,
                    },
                );
            }

            // Extract relations from properties with x-relation
            let mut relations = Vec::new();
            if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_schema) in props.iter() {
                    if let Some(relation_obj) =
                        prop_schema.get("x-relation").and_then(|r| r.as_object())
                    {
                        // Extract relation configuration
                        let relation_type = relation_obj
                            .get("type")
                            .and_then(|t| t.as_str())
                            .and_then(|t| match t {
                                "hasMany" => Some(RelationType::HasMany),
                                "belongsTo" => Some(RelationType::BelongsTo),
                                "hasOne" => Some(RelationType::HasOne),
                                "belongsToMany" => Some(RelationType::BelongsToMany),
                                _ => None,
                            });

                        let target_table = relation_obj
                            .get("target")
                            .and_then(|t| t.as_str())
                            .map(Self::to_table_name);

                        let foreign_key = relation_obj
                            .get("foreignKey")
                            .and_then(|fk| fk.as_str())
                            .map(|s| s.to_string());

                        let local_key = relation_obj
                            .get("localKey")
                            .and_then(|lk| lk.as_str())
                            .map(|s| s.to_string());

                        if let (Some(rel_type), Some(target), Some(fk)) =
                            (relation_type, target_table, foreign_key)
                        {
                            relations.push(RelationDefinition {
                                field_name: prop_name.clone(),
                                relation_type: rel_type,
                                target_table: target,
                                foreign_key: fk,
                                local_key,
                            });
                        }
                    }
                }
            }

            let table_name = Self::to_table_name(schema_name);
            tables.push(TableSchema {
                table_name,
                columns,
                indexes,
                relations,
            });
        }

        Some(tables)
    }

    fn to_table_name(schema_name: &str) -> String {
        // Convert PascalCase to snake_case (e.g., "UserProfile" -> "user_profiles")
        let mut result = String::new();
        let mut chars = schema_name.chars().peekable();

        while let Some(c) = chars.next() {
            if c.is_uppercase() && !result.is_empty() {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        }

        // Pluralize if not already plural
        if !result.ends_with('s') {
            result.push('s');
        }

        result
    }

    fn infer_sql_type_and_default(
        prop_name: &str,
        prop_schema: &Value,
    ) -> (String, Option<String>) {
        // Special cases for audit fields
        match prop_name {
            "createdBy" | "updatedBy" => {
                return ("TEXT".to_string(), None);
            }
            "createdAt" | "created_at" => {
                return (
                    "DATETIME".to_string(),
                    Some("CURRENT_TIMESTAMP".to_string()),
                );
            }
            "updatedAt" | "updated_at" => {
                return ("DATETIME".to_string(), None);
            }
            _ => {}
        }

        let t = prop_schema
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let format = prop_schema
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match (t, format) {
            ("string", "date-time") => ("DATETIME".to_string(), None),
            ("string", "date") => ("DATE".to_string(), None),
            ("string", _) => ("TEXT".to_string(), None),
            ("integer", _) => ("INTEGER".to_string(), None),
            ("number", _) => ("REAL".to_string(), None),
            ("boolean", _) => ("INTEGER".to_string(), None),
            ("array", _) => ("TEXT".to_string(), None),
            ("object", _) => ("TEXT".to_string(), None),
            _ => ("TEXT".to_string(), None),
        }
    }

    /// Generate CREATE TABLE SQL statement for SQLite
    pub fn generate_create_table_sql_sqlite(schema: &TableSchema) -> String {
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", schema.table_name);

        let mut column_defs = Vec::new();
        for col in &schema.columns {
            let mut col_def = format!(
                "    {} {}",
                col.name,
                Self::map_type_to_sqlite(&col.column_type)
            );

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
            let index_type = if index.unique {
                "UNIQUE INDEX"
            } else {
                "INDEX"
            };
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
            // Special handling: INTEGER primary key with auto_increment -> SERIAL PRIMARY KEY
            let col_def =
                if col.primary_key && col.auto_increment && Self::is_integer_type(&col.column_type)
                {
                    format!("    {} SERIAL PRIMARY KEY", col.name)
                } else {
                    let mut tmp = format!(
                        "    {} {}",
                        col.name,
                        Self::map_type_to_postgres(&col.column_type)
                    );

                    if col.primary_key {
                        tmp.push_str(" PRIMARY KEY");
                    }

                    if !col.nullable && !col.primary_key {
                        tmp.push_str(" NOT NULL");
                    }

                    if col.unique && !col.primary_key {
                        tmp.push_str(" UNIQUE");
                    }

                    if let Some(default) = &col.default_value {
                        tmp.push_str(&format!(" DEFAULT {}", default));
                    }

                    tmp
                };

            column_defs.push(col_def);
        }

        sql.push_str(&column_defs.join(",\n"));
        sql.push_str("\n);\n");

        // Generate index statements
        for index in &schema.indexes {
            let index_type = if index.unique {
                "UNIQUE INDEX"
            } else {
                "INDEX"
            };
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
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "name".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "email".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: true,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
            ],
            indexes: vec![IndexDefinition {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string()],
                unique: false,
            }],
            relations: vec![],
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
