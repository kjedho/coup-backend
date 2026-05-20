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

    pub fn from_str(card: &str) -> Result<Self, &'static str> {
        match card {
            "Assassin" => Ok(Self::new(Role::Assassin)),
            "Contessa" => Ok(Self::new(Role::Contessa)),
            "Captain" => Ok(Self::new(Role::Captain)),
            "Duke" => Ok(Self::new(Role::Duke)),
            "Ambassador" => Ok(Self::new(Role::Ambassador)),
            _ => Err("Invalid card role"),
        }
    }
}