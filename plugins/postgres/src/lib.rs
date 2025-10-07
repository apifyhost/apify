mod input;
mod postgres;
mod response;
use std::sync::Arc;

use input::Input;
use postgres::PostgresConfig;
use response::QueryResult;
use sdk::prelude::*;
use tokio_postgres::types::ToSql;

create_step!(postgres(setup));

pub async fn postgres(setup: ModuleSetup) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rx = module_channel!(setup);
    let config = PostgresConfig::try_from(setup.with.clone())?;
    let pool = Arc::new(config.create_pool()?);

    let mut handles = Vec::new();

    for plugin in rx {
        let pool = pool.clone();
        let config = config.clone();

        let handle = tokio::spawn(async move {
            let input = match Input::try_from((plugin.input, &config)) {
                Ok(input) => input,
                Err(e) => {
                    let response =
                        ModuleResponse::from_error(format!("Failed to parse input: {e}"));

                    sender_safe!(plugin.sender, response);
                    return;
                }
            };

            let client = match pool.get().await {
                Ok(client) => client,
                Err(e) => {
                    let response = ModuleResponse::from_error(format!(
                        "Failed to get client from pool: {e}"
                    ));

                    sender_safe!(plugin.sender, response);
                    return;
                }
            };

            if input.batch {
                let stmt = if input.cache_query {
                    match client.prepare_cached(input.query.as_str()).await {
                        Ok(stmt) => stmt,
                        Err(e) => {
                            let response = ModuleResponse::from_error(format!(
                                "Failed to prepare statement: {e}"
                            ));

                            sender_safe!(plugin.sender, response);
                            return;
                        }
                    }
                } else {
                    match client.prepare(input.query.as_str()).await {
                        Ok(stmt) => stmt,
                        Err(e) => {
                            let response = ModuleResponse::from_error(format!(
                                "Failed to prepare statement: {e}"
                            ));

                            sender_safe!(plugin.sender, response);
                            return;
                        }
                    }
                };

                let param_refs: Vec<&(dyn ToSql + Sync)> = input
                    .params
                    .iter()
                    .map(|p| p.as_ref() as &(dyn ToSql + Sync))
                    .collect();

                match client.query(&stmt, &param_refs[..]).await {
                    Ok(rows) => {
                        let result = QueryResult::from(rows);

                        sender_safe!(plugin.sender, result.to_value().into());
                    }
                    Err(e) => {
                        let response =
                            ModuleResponse::from_error(format!("Query execution failed: {e}"));

                        sender_safe!(plugin.sender, response);
                    }
                }
            } else {
                match client.batch_execute(&input.query).await {
                    Ok(_) => {
                        let response = "OK".to_value().into();
                        sender_safe!(plugin.sender, response);
                    }
                    Err(e) => {
                        let response =
                            ModuleResponse::from_error(format!("Batch execution failed: {e}"));
                        sender_safe!(plugin.sender, response);
                    }
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("Error in task: {e:?}");
        }
    }

    Ok(())
}
