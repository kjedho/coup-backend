use serde::{Deserialize, Serialize};
use rand::Rng;
use uuid::Uuid;
use super::player::Player;
use super::deck::Deck;

const MAX_PLAYERS: usize = 6;
const MIN_PLAYERS: usize = 2;
const MAX_COINS: u8 = 50;

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    pub players: Vec<Player>,
    pub deck: Deck,
    pub coins: u8,
    pub current_player: usize,
    pub started: bool,
}

impl Game {
    pub fn new(creator_uuid: &Uuid, creator_name: &String, num_players: usize) -> Self {
        let mut players = Vec::with_capacity(num_players);
        players.push(Player::new(creator_uuid, creator_name));
        Self {
            players,
            deck: Deck::new(),
            coins: MAX_COINS,
            current_player: 0,
            started: false,
        }
    }

    pub fn add_player(&mut self, player: Player) {
        self.players.push(player);
    }

    pub fn start_game(&mut self) -> Result<&Player, &'static str> {
        if self.players.len() < MIN_PLAYERS {
            return Err("Not enough players");
        }
        if self.players.len() > MAX_PLAYERS {
            return Err("Too many players");
        }
        self.started = true;
        self.deck.shuffle();
        for player in self.players.iter_mut() {
            player.cards.push(self.deck.draw().unwrap());
            player.cards.push(self.deck.draw().unwrap());
        }
        let mut rng = rand::thread_rng();
        self.current_player = rng.gen_range(0..self.players.len());
        Ok(&self.players[self.current_player])
    }

    pub fn next_player(&mut self) -> &Player {
        self.current_player = (self.current_player + 1) % self.players.len();
        &self.players[self.current_player]
    }

    pub fn check_game_over(&self) -> Option<Player> {
        let mut alive_players = self.players.iter().filter(|player| player.is_alive).collect::<Vec<&Player>>();
        if alive_players.len() == 1 {
            return Some(alive_players.pop().unwrap().clone());
        }
        None
    }
}