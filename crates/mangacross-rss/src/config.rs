use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub targets: Vec<String>,
}
