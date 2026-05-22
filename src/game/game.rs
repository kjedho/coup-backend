use rand::Rng;
use serde::Serialize;
use uuid::Uuid;

use super::deck::Deck;
use super::player::Player;
use super::turn_phase::TurnContext;

const MAX_PLAYERS: usize = 6;
const MIN_PLAYERS: usize = 2;
const MAX_COINS: u8 = 50;

#[derive(Debug, Serialize, Clone)]
pub struct AvailableAction {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct Game {
    pub players: Vec<Player>,
    pub deck: Deck,
    pub coins: u8,
    pub current_player: usize,
    pub started: bool,
    pub max_players: usize,
    pub turn_context: Option<TurnContext>,
    pub pending_influence_loss: Option<Uuid>,
    pub room_code: String,
}

const CODE_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_LENGTH: usize = 4;

fn generate_room_code() -> String {
    let mut rng = rand::thread_rng();
    (0..CODE_LENGTH)
        .map(|_| CODE_CHARS[rng.gen_range(0..CODE_CHARS.len())] as char)
        .collect()
}

impl Game {
    pub fn new(creator_uuid: &Uuid, creator_name: &str, num_players: usize) -> Self {
        let mut players = Vec::with_capacity(num_players);
        players.push(Player::new(creator_uuid, creator_name));
        Self {
            players,
            deck: Deck::new(),
            coins: MAX_COINS,
            current_player: 0,
            started: false,
            max_players: num_players,
            turn_context: None,
            pending_influence_loss: None,
            room_code: generate_room_code(),
        }
    }

    pub fn add_player(&mut self, player: Player) -> Result<(), &'static str> {
        if self.players.len() >= self.max_players {
            return Err("Lobby is full");
        }
        if self.players.iter().any(|p| p.name == player.name) {
            return Err("A player with that name is already in the lobby");
        }
        self.players.push(player);
        Ok(())
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
        loop {
            self.current_player = (self.current_player + 1) % self.players.len();
            if self.players[self.current_player].is_alive {
                break;
            }
        }
        &self.players[self.current_player]
    }

    pub fn check_game_over(&self) -> Option<&Player> {
        let alive: Vec<&Player> = self.players.iter().filter(|p| p.is_alive).collect();
        if alive.len() == 1 {
            Some(alive[0])
        } else {
            None
        }
    }

    pub fn available_actions(&self, player_uuid: &Uuid) -> Vec<AvailableAction> {
        if self.players[self.current_player].uuid != *player_uuid {
            return vec![];
        }
        if self.pending_influence_loss.is_some() {
            return vec![];
        }
        if self.turn_context.is_some() {
            return vec![];
        }

        let player = &self.players[self.current_player];

        if !player.exchange_cards.is_empty() {
            return vec![];
        }

        let alive_targets: Vec<String> = self
            .players
            .iter()
            .filter(|p| p.is_alive && p.uuid != *player_uuid)
            .map(|p| p.name.clone())
            .collect();

        // Forced coup with 10+ coins
        if player.coins >= 10 {
            return vec![AvailableAction {
                action: "coup".to_string(),
                targets: Some(alive_targets),
            }];
        }

        let mut actions = vec![];

        // Always available
        if self.coins > 0 {
            actions.push(AvailableAction {
                action: "income".to_string(),
                targets: None,
            });
        }
        if self.coins >= 2 {
            actions.push(AvailableAction {
                action: "foreign_aid".to_string(),
                targets: None,
            });
        }

        // Role-based (no challenge verification in phase 1)
        if self.coins >= 3 {
            actions.push(AvailableAction {
                action: "tax".to_string(),
                targets: None,
            });
        }

        actions.push(AvailableAction {
            action: "exchange_draw".to_string(),
            targets: None,
        });

        if player.coins >= 7 {
            actions.push(AvailableAction {
                action: "coup".to_string(),
                targets: Some(alive_targets.clone()),
            });
        }

        if player.coins >= 3 {
            actions.push(AvailableAction {
                action: "assassinate".to_string(),
                targets: Some(alive_targets.clone()),
            });
        }

        let stealable: Vec<String> = self
            .players
            .iter()
            .filter(|p| p.is_alive && p.uuid != *player_uuid && p.coins > 0)
            .map(|p| p.name.clone())
            .collect();
        if !stealable.is_empty() {
            actions.push(AvailableAction {
                action: "steal".to_string(),
                targets: Some(stealable),
            });
        }

        actions
    }
}
