use std::vec;

use serde::{Deserialize, Serialize};
use super::card::Card;

enum Action {
    Income,
    ForeignAid,
    Coup,
    Tax,
    Assassinate,
    Exchange,
    Steal,
}

enum CounterAction {
    CallBluff,
    BlockForeignAid,
    BlockAssassinate,
    BlockSteal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Player {
    name: String,
    takes_turn: bool,
    is_alive: bool,
    cards: Vec<Card>,
    coins: u8,
}

impl Player {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            takes_turn: false,
            is_alive: true,
            cards: vec![],
            coins: 2,
        }
    }
}