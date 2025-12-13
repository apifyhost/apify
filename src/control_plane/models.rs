use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiConfigRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub spec: String,
    pub datasource_name: Option<String>,
    pub modules_config: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatasourceConfigRecord {
    pub id: String,
    pub name: String,
    pub config: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthConfigRecord {
    pub id: String,
    pub config: String,
    pub updated_at: i64,
}
