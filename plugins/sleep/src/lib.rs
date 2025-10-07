use sdk::prelude::*;

create_step!(sleep(rx));

struct Sleep {
    time: u64,
}

impl From<&Value> for Sleep {
    fn from(value: &Value) -> Self {
        let time = if let Some(value) = value.get("milliseconds") {
            value.to_u64().unwrap_or(0)
        } else if let Some(value) = value.get("seconds") {
            value.to_u64().unwrap_or(0) * 1000
        } else if let Some(value) = value.get("minutes") {
            value.to_u64().unwrap_or(0) * 1000 * 60
        } else if let Some(value) = value.get("hours") {
            value.to_u64().unwrap_or(0) * 1000 * 60
        } else {
            0
        };

        Self { time }
    }
}

pub async fn sleep(rx: ModuleReceiver) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    listen!(rx, move |plugin: ModulePackage| async {
        let sleep = match plugin.input() {
            Some(value) => Sleep::from(&value),
            _ => Sleep { time: 0 },
        };

        if sleep.time > 0 {
            log::debug!("Sleeping for {} milliseconds", sleep.time);
            std::thread::sleep(std::time::Duration::from_millis(sleep.time));
        } else {
            log::debug!("No sleep time provided, skipping sleep");
        }

        let payload = plugin.payload().unwrap_or(Value::Null);
        sender_safe!(plugin.sender, payload.into());
    });

    Ok(())
}
