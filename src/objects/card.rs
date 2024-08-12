use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
enum Role {
    Assassin,
    Contessa,
    Captain,
    Duke,
    Ambassador,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Card {
    role: Role,
    visible: bool, 
}

impl Card {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            visible: false,
        }
    }
}