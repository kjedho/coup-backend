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

    pub fn from_str(card: &str) -> Self {
        match card {
            "Assassin" => Self::new(Role::Assassin),
            "Contessa" => Self::new(Role::Contessa),
            "Captain" => Self::new(Role::Captain),
            "Duke" => Self::new(Role::Duke),
            "Ambassador" => Self::new(Role::Ambassador),
            _ => panic!("Invalid card"),
        }
    }
}