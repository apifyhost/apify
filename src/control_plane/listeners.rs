use crate::database::DatabaseManager;

pub async fn load_listeners(
    db: &DatabaseManager,
) -> Result<Option<Vec<crate::config::ListenerConfig>>, Box<dyn std::error::Error + Send + Sync>> {
    let records = db.select("_meta_listeners", None, None, None, None).await?;

    if records.is_empty() {
        return Ok(None);
    }

    let mut listeners = Vec::new();
    for record in records {
        if let Some(config_val) = record.get("config")
            && let Some(config_str) = config_val.as_str()
        {
            let listener: crate::config::ListenerConfig = serde_json::from_str(config_str)?;
            listeners.push(listener);
        }
    }

    Ok(Some(listeners))
}
