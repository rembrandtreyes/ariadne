use serde::Serialize;

pub fn to_json<T: Serialize>(data: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(data)?)
}

pub fn to_json_compact<T: Serialize>(data: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string(data)?)
}
