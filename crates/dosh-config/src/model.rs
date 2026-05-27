use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoshConfig {
    pub prompt_style: Option<String>,
}
