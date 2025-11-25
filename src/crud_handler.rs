//! CRUD operation handlers

use crate::api_generator::{APIGenerator, OperationType, RoutePattern};
use crate::database::DatabaseManager;
use crate::modules::ConsumerIdentity;
use crate::phases::RequestContext;
use serde_json::Value;
use sqlx::types::chrono::Utc;
use std::collections::HashMap;

#[derive(Debug)]
pub enum CRUDError {
    DatabaseError(crate::database::DatabaseError),
    ValidationError(String),
    NotFoundError(String),
    InvalidParameterError(String),
}

impl std::fmt::Display for CRUDError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CRUDError::DatabaseError(err) => write!(f, "Database error: {err}"),
            CRUDError::ValidationError(err) => write!(f, "Validation error: {err}"),
            CRUDError::NotFoundError(err) => write!(f, "Not found: {err}"),
            CRUDError::InvalidParameterError(err) => write!(f, "Invalid parameter: {err}"),
        }
    }
}

impl std::error::Error for CRUDError {}

impl From<crate::database::DatabaseError> for CRUDError {
    fn from(err: crate::database::DatabaseError) -> Self {
        CRUDError::DatabaseError(err)
    }
}

#[derive(Debug)]
pub struct CRUDHandler {
    db_manager: DatabaseManager,
    pub api_generator: APIGenerator,
}

impl CRUDHandler {
    /// Try to coerce a string into a numeric JSON value when appropriate (i64 or f64), else keep as string
    fn coerce_string_to_json_value(s: &str) -> Value {
        if let Ok(i) = s.parse::<i64>() {
            return Value::Number(i.into());
        }
        if let Ok(f) = s.parse::<f64>()
            && let Some(n) = serde_json::Number::from_f64(f)
        {
            return Value::Number(n);
        }
        Value::String(s.to_string())
    }
    pub fn new(db_manager: DatabaseManager, api_generator: APIGenerator) -> Self {
        Self {
            db_manager,
            api_generator,
        }
    }

    /// Handle CRUD operations based on route pattern
    pub async fn handle_request(
        &self,
        method: &str,
        path: &str,
        path_params: HashMap<String, String>,
        query_params: HashMap<String, String>,
        body: Option<Value>,
        ctx: &RequestContext,
    ) -> Result<Value, CRUDError> {
        // Find matching route pattern
        let pattern = self
            .api_generator
            .match_operation(method, path)
            .ok_or_else(|| {
                CRUDError::NotFoundError(format!("No matching route for {} {}", method, path))
            })?;

        match pattern.operation_type {
            OperationType::List => self.handle_list(pattern, query_params).await,
            OperationType::Get => self.handle_get(pattern, path_params).await,
            OperationType::Create => self.handle_create(pattern, body, ctx).await,
            OperationType::Update => self.handle_update(pattern, path_params, body, ctx).await,
            OperationType::Delete => self.handle_delete(pattern, path_params).await,
        }
    }

    /// Handle GET /table (list all records)
    async fn handle_list(
        &self,
        pattern: &RoutePattern,
        query_params: HashMap<String, String>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        // Extract pagination parameters
        let limit = query_params
            .get("limit")
            .and_then(|s| s.parse::<u32>().ok());
        let offset = query_params
            .get("offset")
            .and_then(|s| s.parse::<u32>().ok());

        // Extract filter parameters (exclude pagination params)
        let mut where_clause = HashMap::new();
        for (key, value) in query_params {
            if key != "limit" && key != "offset" {
                where_clause.insert(key, Value::String(value));
            }
        }

        let results = self
            .db_manager
            .select(
                table,
                None,
                if where_clause.is_empty() {
                    None
                } else {
                    Some(where_clause)
                },
                limit,
                offset,
            )
            .await?;

        // Load nested relations for each record
        let table_schema = self.api_generator.get_table_schema(table);
        if let Some(schema) = table_schema
            && !schema.relations.is_empty()
        {
            let mut enriched_results = Vec::new();
            for record in results {
                let normalized = self.normalize_record_casing(table, record);
                let enriched = self.load_relations_for_record(table, normalized).await?;
                enriched_results.push(enriched);
            }
            return Ok(Value::Array(enriched_results));
        }

        // Normalize results even if no relations
        let normalized_results = results
            .into_iter()
            .map(|r| self.normalize_record_casing(table, r))
            .collect();

        Ok(Value::Array(normalized_results))
    }

    /// Handle GET /table/{id} (get single record)
    async fn handle_get(
        &self,
        pattern: &RoutePattern,
        path_params: HashMap<String, String>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        // Use the first path parameter as the primary key
        let id_param = path_params
            .keys()
            .next()
            .ok_or_else(|| CRUDError::InvalidParameterError("No ID parameter found".to_string()))?;
        let id_value = path_params.get(id_param).ok_or_else(|| {
            CRUDError::InvalidParameterError("ID parameter value not found".to_string())
        })?;

        let id_json = Self::coerce_string_to_json_value(id_value);

        // Check if this table has relations
        let table_schema = self.api_generator.get_table_schema(table);
        if let Some(schema) = table_schema
            && !schema.relations.is_empty()
        {
            // Use fetch_with_relations to get record with nested data
            return self.fetch_with_relations(table, id_json).await;
        }

        // No relations, use regular select
        let mut where_clause = HashMap::new();
        where_clause.insert(id_param.clone(), id_json);

        let results = self
            .db_manager
            .select(table, None, Some(where_clause), Some(1), None)
            .await?;

        let record = results.into_iter().next().ok_or_else(|| {
            CRUDError::NotFoundError(format!("Record with {} = {} not found", id_param, id_value))
        })?;

        Ok(self.normalize_record_casing(table, record))
    }

    /// Handle POST /table (create new record)
    async fn handle_create(
        &self,
        pattern: &RoutePattern,
        body: Option<Value>,
        ctx: &RequestContext,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        let data =
            body.ok_or_else(|| CRUDError::ValidationError("Request body is required".to_string()))?;

        let mut data_map = match data {
            Value::Object(map) => map,
            _ => {
                return Err(CRUDError::ValidationError(
                    "Request body must be a JSON object".to_string(),
                ));
            }
        };

        // Extract nested relations before processing main record
        let table_schema = self.api_generator.get_table_schema(table);
        let mut nested_relations: Vec<(String, Vec<Value>)> = Vec::new();
        let mut nested_single_relations: Vec<(String, Value)> = Vec::new(); // For hasOne

        if let Some(schema) = table_schema {
            tracing::debug!(
                table = %table,
                relations_count = schema.relations.len(),
                relations = ?schema.relations.iter().map(|r| &r.field_name).collect::<Vec<_>>(),
                "Checking for nested relations"
            );

            for relation in &schema.relations {
                match relation.relation_type {
                    crate::schema_generator::RelationType::HasMany => {
                        // Extract array of nested items
                        if let Some(nested_data) = data_map.remove(&relation.field_name)
                            && let Value::Array(items) = nested_data
                        {
                            tracing::info!(
                                relation = %relation.field_name,
                                item_count = items.len(),
                                "Extracted hasMany relation data"
                            );
                            nested_relations.push((relation.field_name.clone(), items));
                        }
                    }
                    crate::schema_generator::RelationType::HasOne => {
                        // Extract single nested object
                        if let Some(nested_data) = data_map.remove(&relation.field_name)
                            && let Value::Object(_) = nested_data
                        {
                            tracing::info!(
                                relation = %relation.field_name,
                                "Extracted hasOne relation data"
                            );
                            nested_single_relations
                                .push((relation.field_name.clone(), nested_data));
                        }
                    }
                    crate::schema_generator::RelationType::BelongsTo => {
                        // For belongsTo, just remove the nested object if present
                        // The foreign key should already be in the data
                        data_map.remove(&relation.field_name);
                    }
                    _ => {}
                }
            }

            // Inject audit fields for create operation
            let now = Utc::now().to_rfc3339();
            for col in &schema.columns {
                if col.auto_field {
                    match col.name.as_str() {
                        "createdBy" | "updatedBy" | "created_by" | "updated_by" => {
                            if let Some(identity) = ctx.extensions.get::<ConsumerIdentity>() {
                                data_map
                                    .insert(col.name.clone(), Value::String(identity.name.clone()));
                            }
                        }
                        "createdAt" | "updatedAt" | "created_at" | "updated_at" => {
                            if !data_map.contains_key(&col.name) {
                                data_map.insert(col.name.clone(), Value::String(now.clone()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Convert serde_json::Map to HashMap<String, Value>
        let mut data_hashmap = HashMap::new();
        for (key, value) in data_map {
            data_hashmap.insert(key, value);
        }

        // Insert main record
        let result = self.db_manager.insert(table, data_hashmap).await?;

        // Normalize the returned record
        let result = if let Value::Object(mut map) = result {
            if let Some(record) = map.remove("record") {
                let normalized = self.normalize_record_casing(table, record);
                map.insert("record".to_string(), normalized);
            }
            Value::Object(map)
        } else {
            result
        };

        let has_nested_data = !nested_relations.is_empty() || !nested_single_relations.is_empty();

        tracing::info!(
            table = %table,
            has_nested = has_nested_data,
            nested_count = nested_relations.len() + nested_single_relations.len(),
            result = ?result,
            "Record inserted"
        );

        // Get the inserted ID for nested relations
        let inserted_id = result.get("id").or_else(|| result.get("last_insert_rowid"));

        // Handle hasMany nested relations
        if !nested_relations.is_empty() || !nested_single_relations.is_empty() {
            if let Some(parent_id) = inserted_id {
                tracing::info!(parent_id = ?parent_id, "Processing nested relations");
                let schema = self.api_generator.get_table_schema(table).unwrap();

                // Process hasMany relations (arrays)
                for (field_name, items) in nested_relations {
                    // Find the relation definition
                    if let Some(relation) =
                        schema.relations.iter().find(|r| r.field_name == field_name)
                    {
                        tracing::info!(
                            relation_field = %field_name,
                            target_table = %relation.target_table,
                            foreign_key = %relation.foreign_key,
                            item_count = items.len(),
                            "Inserting hasMany nested items"
                        );

                        // Insert each nested item
                        for mut item in items {
                            if let Value::Object(ref mut item_map) = item {
                                // Inject foreign key
                                item_map.insert(relation.foreign_key.clone(), parent_id.clone());

                                // Inject audit fields
                                if let Some(target_schema) =
                                    self.api_generator.get_table_schema(&relation.target_table)
                                {
                                    let now = Utc::now().to_rfc3339();
                                    for col in &target_schema.columns {
                                        if col.auto_field {
                                            if matches!(
                                                col.name.as_str(),
                                                "createdBy"
                                                    | "updatedBy"
                                                    | "created_by"
                                                    | "updated_by"
                                            ) {
                                                if let Some(identity) =
                                                    ctx.extensions.get::<ConsumerIdentity>()
                                                {
                                                    item_map.insert(
                                                        col.name.clone(),
                                                        Value::String(identity.name.clone()),
                                                    );
                                                }
                                            } else if matches!(
                                                col.name.as_str(),
                                                "createdAt"
                                                    | "updatedAt"
                                                    | "created_at"
                                                    | "updated_at"
                                            ) {
                                                // Only inject if not already present
                                                if !item_map.contains_key(&col.name) {
                                                    item_map.insert(
                                                        col.name.clone(),
                                                        Value::String(now.clone()),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }

                                let mut item_hashmap = HashMap::new();
                                for (k, v) in item_map.iter() {
                                    item_hashmap.insert(k.clone(), v.clone());
                                }

                                let item_result = self
                                    .db_manager
                                    .insert(&relation.target_table, item_hashmap)
                                    .await?;
                                tracing::info!(item_result = ?item_result, "Nested item inserted");
                            }
                        }
                    }
                }

                // Process hasOne relations (single objects)
                for (field_name, item) in nested_single_relations {
                    if let Some(relation) =
                        schema.relations.iter().find(|r| r.field_name == field_name)
                    {
                        tracing::info!(
                            relation_field = %field_name,
                            target_table = %relation.target_table,
                            foreign_key = %relation.foreign_key,
                            "Inserting hasOne nested object"
                        );

                        if let Value::Object(mut item_map) = item {
                            // Inject foreign key
                            item_map.insert(relation.foreign_key.clone(), parent_id.clone());

                            // Inject audit fields
                            if let Some(target_schema) =
                                self.api_generator.get_table_schema(&relation.target_table)
                            {
                                let now = Utc::now().to_rfc3339();
                                for col in &target_schema.columns {
                                    if col.auto_field {
                                        if matches!(
                                            col.name.as_str(),
                                            "createdBy" | "updatedBy" | "created_by" | "updated_by"
                                        ) {
                                            if let Some(identity) =
                                                ctx.extensions.get::<ConsumerIdentity>()
                                            {
                                                item_map.insert(
                                                    col.name.clone(),
                                                    Value::String(identity.name.clone()),
                                                );
                                            }
                                        } else if matches!(
                                            col.name.as_str(),
                                            "createdAt" | "updatedAt" | "created_at" | "updated_at"
                                        ) {
                                            // Only inject if not already present
                                            if !item_map.contains_key(&col.name) {
                                                item_map.insert(
                                                    col.name.clone(),
                                                    Value::String(now.clone()),
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            let mut item_hashmap = HashMap::new();
                            for (k, v) in item_map.iter() {
                                item_hashmap.insert(k.clone(), v.clone());
                            }

                            let item_result = self
                                .db_manager
                                .insert(&relation.target_table, item_hashmap)
                                .await?;
                            tracing::info!(item_result = ?item_result, "HasOne object inserted");
                        }
                    }
                }

                // Re-fetch the record with nested data to return complete result
                return self.fetch_with_relations(table, parent_id.clone()).await;
            } else {
                tracing::warn!(
                    "No parent ID found in insert result, cannot process nested relations"
                );
            }
        }

        Ok(result)
    }

    /// Fetch a record with its related data
    async fn fetch_with_relations(&self, table: &str, id: Value) -> Result<Value, CRUDError> {
        let mut where_clause = HashMap::new();
        where_clause.insert("id".to_string(), id);

        let results = self
            .db_manager
            .select(table, None, Some(where_clause), Some(1), None)
            .await?;

        let record = results
            .into_iter()
            .next()
            .ok_or_else(|| CRUDError::NotFoundError("Record not found".to_string()))?;

        let normalized = self.normalize_record_casing(table, record);

        // Load relations
        self.load_relations_for_record(table, normalized).await
    }

    /// Load all relations for a single record
    async fn load_relations_for_record(
        &self,
        table: &str,
        mut record: Value,
    ) -> Result<Value, CRUDError> {
        let table_schema = match self.api_generator.get_table_schema(table) {
            Some(schema) => schema,
            None => return Ok(record),
        };

        if table_schema.relations.is_empty() {
            return Ok(record);
        }

        let record_obj = match record.as_object_mut() {
            Some(obj) => obj,
            None => return Ok(record),
        };

        let record_id = match record_obj.get("id") {
            Some(id) => id.clone(),
            None => return Ok(record),
        };

        for relation in &table_schema.relations {
            match relation.relation_type {
                crate::schema_generator::RelationType::HasMany => {
                    // Query child records
                    let mut where_clause = HashMap::new();
                    where_clause.insert(relation.foreign_key.clone(), record_id.clone());

                    let children = self
                        .db_manager
                        .select(&relation.target_table, None, Some(where_clause), None, None)
                        .await
                        .unwrap_or_else(|_| Vec::new());

                    let normalized_children: Vec<Value> = children
                        .into_iter()
                        .map(|c| self.normalize_record_casing(&relation.target_table, c))
                        .collect();

                    record_obj.insert(
                        relation.field_name.clone(),
                        Value::Array(normalized_children),
                    );
                }
                crate::schema_generator::RelationType::HasOne => {
                    // Query single related record
                    let mut where_clause = HashMap::new();
                    where_clause.insert(relation.foreign_key.clone(), record_id.clone());

                    let related = self
                        .db_manager
                        .select(
                            &relation.target_table,
                            None,
                            Some(where_clause),
                            Some(1),
                            None,
                        )
                        .await
                        .unwrap_or_else(|_| Vec::new());

                    if let Some(related_record) = related.into_iter().next() {
                        let normalized =
                            self.normalize_record_casing(&relation.target_table, related_record);
                        record_obj.insert(relation.field_name.clone(), normalized);
                    } else {
                        record_obj.insert(relation.field_name.clone(), Value::Null);
                    }
                }
                crate::schema_generator::RelationType::BelongsTo => {
                    // Get foreign key value from current record
                    if let Some(foreign_id) = record_obj.get(&relation.foreign_key) {
                        let mut where_clause = HashMap::new();
                        where_clause.insert("id".to_string(), foreign_id.clone());

                        let parent = self
                            .db_manager
                            .select(
                                &relation.target_table,
                                None,
                                Some(where_clause),
                                Some(1),
                                None,
                            )
                            .await
                            .unwrap_or_else(|_| Vec::new());

                        if let Some(parent_record) = parent.into_iter().next() {
                            let normalized =
                                self.normalize_record_casing(&relation.target_table, parent_record);
                            record_obj.insert(relation.field_name.clone(), normalized);
                        } else {
                            record_obj.insert(relation.field_name.clone(), Value::Null);
                        }
                    }
                }
                _ => {
                    // BelongsToMany not implemented yet
                }
            }
        }

        Ok(record)
    }

    /// Handle PUT/PATCH /table/{id} (update record)
    async fn handle_update(
        &self,
        pattern: &RoutePattern,
        path_params: HashMap<String, String>,
        body: Option<Value>,
        ctx: &RequestContext,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        let data =
            body.ok_or_else(|| CRUDError::ValidationError("Request body is required".to_string()))?;

        let mut data_map = match data {
            Value::Object(map) => map,
            _ => {
                return Err(CRUDError::ValidationError(
                    "Request body must be a JSON object".to_string(),
                ));
            }
        };

        // Get the record ID
        let id_param = path_params
            .keys()
            .next()
            .ok_or_else(|| CRUDError::InvalidParameterError("No ID parameter found".to_string()))?;
        let id_value = path_params.get(id_param).ok_or_else(|| {
            CRUDError::InvalidParameterError("ID parameter value not found".to_string())
        })?;
        let record_id = Self::coerce_string_to_json_value(id_value);

        // Extract nested relations before processing main record
        let table_schema = self.api_generator.get_table_schema(table);
        let mut nested_relations: Vec<(String, Vec<Value>)> = Vec::new();
        let mut nested_single_relations: Vec<(String, Value)> = Vec::new();

        if let Some(schema) = &table_schema {
            for relation in &schema.relations {
                match relation.relation_type {
                    crate::schema_generator::RelationType::HasMany => {
                        if let Some(nested_data) = data_map.remove(&relation.field_name)
                            && let Value::Array(items) = nested_data
                        {
                            tracing::info!(
                                relation = %relation.field_name,
                                item_count = items.len(),
                                "Extracted hasMany relation for update"
                            );
                            nested_relations.push((relation.field_name.clone(), items));
                        }
                    }
                    crate::schema_generator::RelationType::HasOne => {
                        if let Some(nested_data) = data_map.remove(&relation.field_name)
                            && let Value::Object(_) = nested_data
                        {
                            tracing::info!(
                                relation = %relation.field_name,
                                "Extracted hasOne relation for update"
                            );
                            nested_single_relations
                                .push((relation.field_name.clone(), nested_data));
                        }
                    }
                    crate::schema_generator::RelationType::BelongsTo => {
                        // For belongsTo, just remove the nested object
                        data_map.remove(&relation.field_name);
                    }
                    _ => {}
                }
            }
        }

        // Inject audit fields for update operation
        if let Some(schema) = &table_schema {
            let now = Utc::now().to_rfc3339();
            for col in &schema.columns {
                if col.auto_field {
                    if matches!(col.name.as_str(), "updatedBy" | "updated_by") {
                        if let Some(identity) = ctx.extensions.get::<ConsumerIdentity>() {
                            data_map.insert(col.name.clone(), Value::String(identity.name.clone()));
                        }
                    } else if matches!(col.name.as_str(), "updatedAt" | "updated_at")
                        && !data_map.contains_key(&col.name)
                    {
                        data_map.insert(col.name.clone(), Value::String(now.clone()));
                    }
                }
            }
        }

        // Convert serde_json::Map to HashMap<String, Value>
        let mut data_hashmap = HashMap::new();
        for (key, value) in data_map {
            data_hashmap.insert(key, value);
        }

        let mut where_clause = HashMap::new();
        where_clause.insert(id_param.clone(), record_id.clone());

        // Update main record
        let result = self
            .db_manager
            .update(table, data_hashmap, where_clause)
            .await?;

        // Handle nested relation updates
        if (!nested_relations.is_empty() || !nested_single_relations.is_empty())
            && let Some(schema) = table_schema
        {
            // Process hasMany relations (replace all children)
            for (field_name, new_items) in nested_relations {
                if let Some(relation) = schema.relations.iter().find(|r| r.field_name == field_name)
                {
                    tracing::info!(
                        relation_field = %field_name,
                        target_table = %relation.target_table,
                        new_item_count = new_items.len(),
                        "Updating hasMany relation"
                    );

                    // Delete existing child records
                    let mut child_where = HashMap::new();
                    child_where.insert(relation.foreign_key.clone(), record_id.clone());
                    let deleted = self
                        .db_manager
                        .delete(&relation.target_table, child_where)
                        .await
                        .unwrap_or(0);

                    tracing::info!(deleted_count = deleted, "Deleted old child records");

                    // Insert new child records
                    for mut item in new_items {
                        if let Value::Object(ref mut item_map) = item {
                            item_map.insert(relation.foreign_key.clone(), record_id.clone());

                            // Inject audit fields
                            if let Some(target_schema) =
                                self.api_generator.get_table_schema(&relation.target_table)
                            {
                                let now = Utc::now().to_rfc3339();
                                for col in &target_schema.columns {
                                    if col.auto_field {
                                        if matches!(
                                            col.name.as_str(),
                                            "createdBy" | "updatedBy" | "created_by" | "updated_by"
                                        ) {
                                            if let Some(identity) =
                                                ctx.extensions.get::<ConsumerIdentity>()
                                            {
                                                item_map.insert(
                                                    col.name.clone(),
                                                    Value::String(identity.name.clone()),
                                                );
                                            }
                                        } else if matches!(
                                            col.name.as_str(),
                                            "createdAt" | "updatedAt" | "created_at" | "updated_at"
                                        ) && !item_map.contains_key(&col.name)
                                        {
                                            item_map.insert(
                                                col.name.clone(),
                                                Value::String(now.clone()),
                                            );
                                        }
                                    }
                                }
                            }

                            let mut item_hashmap = HashMap::new();
                            for (k, v) in item_map.iter() {
                                item_hashmap.insert(k.clone(), v.clone());
                            }

                            self.db_manager
                                .insert(&relation.target_table, item_hashmap)
                                .await?;
                        }
                    }
                }
            }

            // Process hasOne relations (replace child)
            for (field_name, new_item) in nested_single_relations {
                if let Some(relation) = schema.relations.iter().find(|r| r.field_name == field_name)
                {
                    tracing::info!(
                        relation_field = %field_name,
                        target_table = %relation.target_table,
                        "Updating hasOne relation"
                    );

                    // Delete existing child record
                    let mut child_where = HashMap::new();
                    child_where.insert(relation.foreign_key.clone(), record_id.clone());
                    self.db_manager
                        .delete(&relation.target_table, child_where)
                        .await
                        .ok();

                    // Insert new child record
                    if let Value::Object(mut item_map) = new_item {
                        item_map.insert(relation.foreign_key.clone(), record_id.clone());

                        // Inject audit fields
                        if let Some(target_schema) =
                            self.api_generator.get_table_schema(&relation.target_table)
                        {
                            let now = Utc::now().to_rfc3339();
                            for col in &target_schema.columns {
                                if col.auto_field {
                                    if matches!(
                                        col.name.as_str(),
                                        "createdBy" | "updatedBy" | "created_by" | "updated_by"
                                    ) {
                                        if let Some(identity) =
                                            ctx.extensions.get::<ConsumerIdentity>()
                                        {
                                            item_map.insert(
                                                col.name.clone(),
                                                Value::String(identity.name.clone()),
                                            );
                                        }
                                    } else if matches!(
                                        col.name.as_str(),
                                        "createdAt" | "updatedAt" | "created_at" | "updated_at"
                                    ) {
                                        // Only inject if not already present
                                        if !item_map.contains_key(&col.name) {
                                            item_map.insert(
                                                col.name.clone(),
                                                Value::String(now.clone()),
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        let mut item_hashmap = HashMap::new();
                        for (k, v) in item_map.iter() {
                            item_hashmap.insert(k.clone(), v.clone());
                        }

                        self.db_manager
                            .insert(&relation.target_table, item_hashmap)
                            .await?;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Handle DELETE /table/{id} (delete record)
    async fn handle_delete(
        &self,
        pattern: &RoutePattern,
        path_params: HashMap<String, String>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        // Use the first path parameter as the primary key for WHERE clause
        let id_param = path_params
            .keys()
            .next()
            .ok_or_else(|| CRUDError::InvalidParameterError("No ID parameter found".to_string()))?;
        let id_value = path_params.get(id_param).ok_or_else(|| {
            CRUDError::InvalidParameterError("ID parameter value not found".to_string())
        })?;

        let id_json = Self::coerce_string_to_json_value(id_value);

        // Check if this table has relations that need cascading delete
        let table_schema = self.api_generator.get_table_schema(table);
        if let Some(schema) = table_schema
            && !schema.relations.is_empty()
        {
            tracing::info!(
                table = %table,
                id = ?id_json,
                relation_count = schema.relations.len(),
                "Checking for cascade delete"
            );

            // Delete child records for hasMany and hasOne relations
            for relation in &schema.relations {
                match relation.relation_type {
                    crate::schema_generator::RelationType::HasMany
                    | crate::schema_generator::RelationType::HasOne => {
                        let mut child_where = HashMap::new();
                        child_where.insert(relation.foreign_key.clone(), id_json.clone());

                        let deleted_count = self
                            .db_manager
                            .delete(&relation.target_table, child_where)
                            .await
                            .unwrap_or(0);

                        if deleted_count > 0 {
                            tracing::info!(
                                relation_type = ?relation.relation_type,
                                target_table = %relation.target_table,
                                deleted_count = deleted_count,
                                "Cascade deleted child records"
                            );
                        }
                    }
                    _ => {
                        // belongsTo doesn't require cascade delete
                    }
                }
            }
        }

        let mut where_clause = HashMap::new();
        where_clause.insert(id_param.clone(), id_json);

        let affected_rows = self.db_manager.delete(table, where_clause).await?;

        if affected_rows == 0 {
            return Err(CRUDError::NotFoundError(format!(
                "Record with {} = {} not found",
                id_param, id_value
            )));
        }

        // Return success response
        let mut response = HashMap::new();
        response.insert(
            "message".to_string(),
            Value::String("Record deleted successfully".to_string()),
        );
        response.insert(
            "affected_rows".to_string(),
            Value::Number(serde_json::Number::from(affected_rows)),
        );

        Ok(Value::Object(response.into_iter().collect()))
    }

    /// Normalize record keys to match schema casing
    fn normalize_record_casing(&self, table: &str, mut record: Value) -> Value {
        let schema = match self.api_generator.get_table_schema(table) {
            Some(s) => s,
            None => return record,
        };

        if let Value::Object(ref mut map) = record {
            // Collect all keys that need renaming
            let mut replacements = Vec::new();

            for (key, _) in map.iter() {
                // Find matching column in schema
                if let Some(col) = schema
                    .columns
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(key))
                    && col.name != *key {
                        replacements.push((key.clone(), col.name.clone()));
                    }
            }

            for (old_key, new_key) in replacements {
                if let Some(val) = map.remove(&old_key) {
                    map.insert(new_key, val);
                }
            }
        }
        record
    }
}
