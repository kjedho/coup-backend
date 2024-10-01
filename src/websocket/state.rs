use crate::game::{game::Game, player::Player};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct GameState {
    pub title: String,
    pub subtitle: String,
    pub players: Vec<(Player, bool)>,
    pub coins: u8,
    pub current_player: String,
}

impl GameState {
    pub fn new(player_uuid: &Uuid, game: &Game) -> Self {
        let mut game_state = Self {
            title: format!("{}'s turn", game.players[game.current_player].name),
            subtitle: "Choosing an action".to_string(),
            players: vec![],
            coins: game.coins,
            current_player: game.players[game.current_player].name.clone(),
        };
        for player in game.players.iter() {
            let is_current_player = player.uuid == *player_uuid;
            let visible_cards ;
            if is_current_player {
                visible_cards = player.cards.clone();
            } else {
                visible_cards = player.cards.iter().filter(|card| card.visible).cloned().collect();
            }
            game_state.players.push((Player {
                uuid: player.uuid,
                name: player.name.clone(),
                is_alive: player.is_alive,
                coins: player.coins,
                cards: visible_cards,
            }, (player.name == game_state.current_player && is_current_player)));
        }
        game_state
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LobbyState {
    pub room_uuid: Uuid,
    pub num_players: usize,
    pub players: Vec<String>,
}

impl LobbyState {
    pub fn new(room_uuid: Uuid, game: &Game) -> Self {
        Self {
            room_uuid,
            num_players: game.players.capacity(),
            players: game.players.iter().map(|player| player.name.clone()).collect::<Vec<String>>(),
        }
    }
}