use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, EnumIter, PartialEq)]
pub enum Role {
    Assassin,
    Contessa,
    Captain,
    Duke,
    Ambassador,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Card {
    pub role: Role,
    pub visible: bool, 
}

impl Card {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            visible: false,
        }
    }
}