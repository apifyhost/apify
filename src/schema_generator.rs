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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        let mut schemas = Vec::new();

        tracing::debug!(
            has_x_table_schemas = spec.get("x-table-schemas").is_some(),
            has_paths = spec.get("paths").is_some(),
            "Starting schema extraction from OpenAPI spec"
        );

        // Look for x-table-schema extensions in the OpenAPI spec
        if let Some(extensions) = spec.get("x-table-schemas").and_then(|v| v.as_array()) {
            tracing::debug!(schemas_count = extensions.len(), "Found x-table-schemas");
            for schema_value in extensions {
                let schema: TableSchema = serde_json::from_value(schema_value.clone())?;
                tracing::debug!(
                    table = %schema.table_name,
                    columns_count = schema.columns.len(),
                    relations_count = schema.relations.len(),
                    "Loaded table schema from x-table-schemas"
                );
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

        // Extract relations from paths (requestBody and responses)
        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            Self::extract_relations_from_paths(&mut schemas, paths);
        }

        tracing::info!(
            schema_count = schemas.len(),
            "Checking if fallback derivation is needed"
        );

        // Fallback: derive from components.schemas if no explicit schemas found
        if schemas.is_empty()
            && let Some(derived) = Self::derive_from_components(spec)
            && !derived.is_empty()
        {
            tracing::info!(
                derived_count = derived.len(),
                "Derived schemas from components"
            );
            return Ok(derived);
        }

        Ok(schemas)
    }

    /// Extract relation definitions from API paths and merge into table schemas
    fn extract_relations_from_paths(
        schemas: &mut [TableSchema],
        paths: &serde_json::Map<String, Value>,
    ) {
        tracing::info!(
            schema_count = schemas.len(),
            paths_count = paths.len(),
            "[extract_relations_from_paths] START"
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
                        tracing::info!(
                            operation_num = operations_found,
                            path = %path_str,
                            method = %method,
                            table = %table_name,
                            "[extract_relations_from_paths] Found operation"
                        );

                        // Extract relations from requestBody schema
                        if let Some(request_body) = op_obj.get("requestBody") {
                            let before_count: usize =
                                schemas.iter().map(|s| s.relations.len()).sum();
                            tracing::info!(
                                table = %table_name,
                                "[extract_relations_from_paths] Checking requestBody"
                            );
                            Self::extract_relations_from_schema(schemas, &table_name, request_body);
                            let after_count: usize =
                                schemas.iter().map(|s| s.relations.len()).sum();
                            let new_relations = after_count - before_count;
                            if new_relations > 0 {
                                tracing::info!(
                                    new_relations = new_relations,
                                    "[extract_relations_from_paths] Found new relations in requestBody"
                                );
                                relations_found += new_relations;
                            }
                        }

                        // Extract relations from response schema
                        if let Some(responses) = op_obj.get("responses").and_then(|r| r.as_object())
                        {
                            tracing::info!(
                                response_count = responses.len(),
                                table = %table_name,
                                "[extract_relations_from_paths] Checking responses"
                            );
                            for (status, response) in responses.iter() {
                                tracing::info!(
                                    status = %status,
                                    table = %table_name,
                                    "[extract_relations_from_paths] Processing response"
                                );
                                Self::extract_relations_from_schema(schemas, &table_name, response);
                                tracing::info!(
                                    status = %status,
                                    "[extract_relations_from_paths] Finished response"
                                );
                            }
                            tracing::info!(
                                table = %table_name,
                                "[extract_relations_from_paths] Finished all responses"
                            );
                        }
                        tracing::info!(
                            operation_num = operations_found,
                            "[extract_relations_from_paths] Finished operation"
                        );
                    }
                }
            }
        }
        tracing::info!(
            operations_found = operations_found,
            relations_found = relations_found,
            "[extract_relations_from_paths] DONE"
        );
        for schema in schemas.iter() {
            if !schema.relations.is_empty() {
                tracing::info!(
                    table = %schema.table_name,
                    relation_count = schema.relations.len(),
                    relations = ?schema.relations.iter().map(|r| &r.field_name).collect::<Vec<_>>(),
                    "[extract_relations_from_paths] Table has relations"
                );
            }
        }
    }

    /// Extract relation definitions from a schema object
    fn extract_relations_from_schema(
        schemas: &mut [TableSchema],
        table_name: &str,
        schema_container: &Value,
    ) {
        tracing::info!(table = %table_name, "[extract_relations_from_schema] ENTERED");
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
        tracing::info!(
            table = %table_name,
            has_properties = has_properties,
            "[extract_relations_from_schema] Scanning table"
        );

        // Extract properties with x-relation
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            tracing::info!(
                property_count = props.len(),
                table = %table_name,
                "[extract_relations_from_schema] Found properties"
            );
            let mut new_relations = Vec::new();

            for (prop_name, prop_schema) in props.iter() {
                if let Some(relation_obj) =
                    prop_schema.get("x-relation").and_then(|r| r.as_object())
                {
                    tracing::info!(
                        property = %prop_name,
                        table = %table_name,
                        relation_def = ?relation_obj,
                        "[extract_relations_from_schema] Found x-relation"
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
                        tracing::info!(
                            table = %table_name,
                            field = %prop_name,
                            relation_type = ?rel_type,
                            target = %target,
                            foreign_key = %fk,
                            "[extract_relations_from_schema] Adding relation"
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
                tracing::info!(
                    table = %table_name,
                    new_relations_count = new_relations.len(),
                    "[extract_relations_from_schema] Merging relations"
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
                tracing::info!(
                    table = %table_name,
                    total_relations = table_schema.relations.len(),
                    "[extract_relations_from_schema] Merge complete"
                );
            } else if !new_relations.is_empty() {
                tracing::warn!(
                    table = %table_name,
                    relations_count = new_relations.len(),
                    available_tables = ?schemas.iter().map(|s| &s.table_name).collect::<Vec<_>>(),
                    "[extract_relations_from_schema] Table not found, cannot add relations"
                );
            }
        } else {
            tracing::info!(
                table = %table_name,
                "[extract_relations_from_schema] No properties found"
            );
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
        tracing::info!("Attempting to derive schemas from components");
        let components = spec.get("components")?.get("schemas")?.as_object()?;
        tracing::info!(
            schema_count = components.len(),
            "Found schemas in components"
        );
        let mut tables = Vec::new();

        for (schema_name, schema_value) in components.iter() {
            tracing::info!(schema_name = %schema_name, "Processing schema");
            let obj = match schema_value.as_object() {
                Some(o) => o,
                None => {
                    tracing::warn!(schema_name = %schema_name, "Schema is not an object");
                    continue;
                }
            };

            // Only process object schemas or those with properties
            let is_object = obj
                .get("type")
                .and_then(|t| t.as_str())
                .map(|t| t == "object")
                .unwrap_or(false)
                || obj.get("properties").is_some();
            if !is_object {
                tracing::warn!(schema_name = %schema_name, "Schema is not an object type or has no properties");
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
                    let is_audit_field = matches!(
                        prop_name.as_str(),
                        "createdAt"
                            | "updatedAt"
                            | "createdBy"
                            | "updatedBy"
                            | "created_at"
                            | "updated_at"
                            | "created_by"
                            | "updated_by"
                    );

                    let auto_field = prop_schema
                        .as_object()
                        .and_then(|o| o.get("x-auto-field"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        || prop_schema
                            .as_object()
                            .and_then(|o| o.get("readOnly"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        || is_audit_field;

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
        let chars = schema_name.chars().peekable();

        for c in chars {
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
                    "TIMESTAMP".to_string(),
                    Some("CURRENT_TIMESTAMP".to_string()),
                );
            }
            "updatedAt" | "updated_at" => {
                return ("TIMESTAMP".to_string(), None);
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
            ("string", "date-time") => ("TIMESTAMP".to_string(), None),
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

    /// Generate migration SQL statements to update current schema to desired schema
    pub fn generate_migration_sql(
        current: &TableSchema,
        desired: &TableSchema,
        driver: &str,
    ) -> Vec<String> {
        let mut sqls = Vec::new();

        if driver == "sqlite" {
            // Check if we need full table recreation
            // Recreation is needed if:
            // 1. Columns are removed
            // 2. Columns are modified (type, nullability, etc.)
            // 3. Primary key changes (not supported yet but good to know)

            let mut needs_recreation = false;

            // Check for removed columns
            for curr_col in &current.columns {
                if !desired.columns.iter().any(|c| c.name == curr_col.name) {
                    needs_recreation = true;
                    break;
                }
            }

            // Check for modified columns
            if !needs_recreation {
                for col in &desired.columns {
                    if let Some(curr_col) = current.columns.iter().find(|c| c.name == col.name) {
                        // Compare attributes relevant for SQLite
                        // Note: SQLite types are loose, but we check if the definition changed
                        let desired_type = Self::map_type_to_sqlite(&col.column_type);
                        // Current type comes from PRAGMA table_info, which might be normalized differently.
                        // We do a loose comparison.
                        let curr_type_norm = curr_col.column_type.to_uppercase();
                        let desired_type_norm = desired_type.to_uppercase();

                        if curr_type_norm != desired_type_norm
                            || curr_col.nullable != col.nullable
                            || curr_col.primary_key != col.primary_key
                            || curr_col.default_value != col.default_value
                        {
                            needs_recreation = true;
                            break;
                        }
                    }
                }
            }

            if needs_recreation {
                return Self::generate_sqlite_recreate_sql(current, desired);
            }
        }

        // 1. Add missing columns
        for col in &desired.columns {
            if !current.columns.iter().any(|c| c.name == col.name) {
                let sql = if driver == "postgres" {
                    Self::generate_add_column_sql_postgres(&desired.table_name, col)
                } else {
                    Self::generate_add_column_sql_sqlite(&desired.table_name, col)
                };
                sqls.push(sql);
            }
        }

        // 2. Modify existing columns (Type changes, Nullability) - Postgres only mostly
        if driver == "postgres" {
            for col in &desired.columns {
                if let Some(curr_col) = current.columns.iter().find(|c| c.name == col.name) {
                    // Check nullability
                    if curr_col.nullable != col.nullable {
                        let sql = format!(
                            "ALTER TABLE {} ALTER COLUMN {} {} NOT NULL",
                            desired.table_name,
                            col.name,
                            if col.nullable { "DROP" } else { "SET" }
                        );
                        sqls.push(sql);
                    }
                }
            }
        }

        sqls
    }

    fn generate_sqlite_recreate_sql(current: &TableSchema, desired: &TableSchema) -> Vec<String> {
        let mut sqls = Vec::new();
        let table = &desired.table_name;
        let temp_table = format!("{}_old_{}", table, uuid::Uuid::new_v4().simple());

        // 1. Rename current table
        sqls.push(format!("ALTER TABLE {} RENAME TO {}", table, temp_table));

        // 2. Create new table
        sqls.push(Self::generate_create_table_sql_sqlite(desired));

        // 3. Copy data
        // Only copy columns that exist in both schemas
        let common_columns: Vec<String> = desired
            .columns
            .iter()
            .filter(|col| current.columns.iter().any(|c| c.name == col.name))
            .map(|col| col.name.clone())
            .collect();

        if !common_columns.is_empty() {
            let cols = common_columns.join(", ");
            sqls.push(format!(
                "INSERT INTO {} ({}) SELECT {} FROM {}",
                table, cols, cols, temp_table
            ));
        }

        // 4. Drop old table
        sqls.push(format!("DROP TABLE {}", temp_table));

        sqls
    }

    fn generate_add_column_sql_postgres(table: &str, col: &ColumnDefinition) -> String {
        let mut sql = format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            table,
            col.name,
            Self::map_type_to_postgres(&col.column_type)
        );
        if !col.nullable {
            sql.push_str(" NOT NULL");
        }
        if let Some(default) = &col.default_value {
            sql.push_str(&format!(" DEFAULT {}", default));
        }
        sql
    }

    fn generate_add_column_sql_sqlite(table: &str, col: &ColumnDefinition) -> String {
        let mut sql = format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            table,
            col.name,
            Self::map_type_to_sqlite(&col.column_type)
        );
        if !col.nullable {
            // SQLite ADD COLUMN with NOT NULL requires a DEFAULT value
            if col.default_value.is_some() {
                sql.push_str(" NOT NULL");
            }
        }
        if let Some(default) = &col.default_value {
            sql.push_str(&format!(" DEFAULT {}", default));
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
            "datetime" | "timestamp" => "TIMESTAMPTZ".to_string(),
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

        assert!(sql.contains("CREATE TABLE IF NOT EXISTS users"));
        assert!(sql.contains("id INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(sql.contains("name TEXT NOT NULL"));
        assert!(sql.contains("email TEXT NOT NULL UNIQUE"));
        assert!(sql.contains("CREATE INDEX IF NOT EXISTS idx_users_email"));
    }
}
