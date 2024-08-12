use serde::{Deserialize, Serialize};
use super::player::Player;

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    players: Vec<Player>,
}

impl Game {
    fn new() -> Self {
        Self {
            players: vec![Player::new("Player 1"), Player::new("Player 2")],
        }
    }

    fn add_player(&mut self, player: Player) {
        self.players.push(player);
    }
}