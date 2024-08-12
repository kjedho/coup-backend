use serde::{Deserialize, Serialize};
use super::card::Card;

#[derive(Debug, Serialize, Deserialize)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn new() -> Self {
        Self {
            cards: vec![],
        }
    }
}