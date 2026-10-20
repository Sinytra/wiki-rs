use serde::de::DeserializeOwned;
use serde_json::error::Category;

#[derive(Debug, thiserror::Error)]
pub enum JsonError {
    #[error("Malformed {name} JSON: {message}")]
    Malformed { name: String, message: String },
    #[error("Invalid {name} JSON format: {message}")]
    Invalid { name: String, message: String },
}

pub fn strip_bom(text: &str) -> &str {
    text.trim_start_matches('\u{FEFF}')
}

pub fn parse_json<T: DeserializeOwned>(name: &str, text: &str) -> Result<T, JsonError> {
    let de = &mut serde_json::Deserializer::from_str(strip_bom(text));
    serde_path_to_error::deserialize(de).map_err(|e| {
        let name = name.to_owned();
        let message = e.to_string();
        match e.inner().classify() {
            Category::Syntax | Category::Eof => JsonError::Malformed { name, message },
            _ => JsonError::Invalid { name, message },
        }
    })
}
