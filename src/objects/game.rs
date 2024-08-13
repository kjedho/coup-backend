use serde::{Deserialize, Serialize};
use super::player::Player;
use super::deck::Deck;

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    players: Vec<Player>,
    deck: Deck,
}

impl Game {
    pub fn new() -> Self {
        Self {
            players: vec![],
            deck: Deck::new(),
        }
    }

    pub fn add_player(&mut self, player: Player) {
        self.players.push(player);
    }
}