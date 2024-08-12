use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Player {
    name: String,
}

impl Player {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}