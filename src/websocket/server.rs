use std::{
    collections::HashMap, sync::{
        atomic::AtomicUsize,
        Arc,
    }
};

use crate::game::game::Game;
use crate::game::player::Player;
use crate::game::card::Card;
use super::state::{ GameState, LobbyState };

use uuid::Uuid;
use actix::prelude::*;
use serde_json;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub addr: Recipient<Message>,
    pub uuid: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub uuid: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    pub client_uuid: Uuid,
    pub room_uuid: Uuid,
    pub msg: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Join {
    pub room_uuid: Uuid,
    pub client_uuid: Uuid,
    pub client_name: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Create {
    pub number_of_players: usize,
    pub client_uuid: Uuid,
    pub client_name: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct StartGame {
    pub room_uuid: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Action {
    pub client_uuid: Uuid,
    pub action: String,
    pub target_name: Option<String>,
    pub selected_card1: Option<String>,
    pub selected_card2: Option<String>,
}

#[derive(Debug)]
pub struct ChatServer {
    sessions: HashMap<Uuid, Recipient<Message>>,
    rooms: HashMap<Uuid, Game>,
    visitor_count: Arc<AtomicUsize>,
}

fn player_to_game(player_uuid: &Uuid, server: &ChatServer) -> Option<Uuid> {
    for (room_uuid, game) in server.rooms.iter() {
        if game.players.iter().any(|player| player.uuid == *player_uuid) {
            return Some(room_uuid.clone());
        }
    }
    None
}

impl ChatServer {
    pub fn new(visitor_count: Arc<AtomicUsize>) -> ChatServer {
        ChatServer {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
            visitor_count,
        }
    }

    fn send_message(&self, room: &Uuid, message: &str) {
        if let Some(game) = self.rooms.get(room) {
            for player in game.players.iter() {
                if let Some(addr) = self.sessions.get(&player.uuid) {
                    addr.do_send(Message(message.to_owned()));
                }
            }
        }
    }
}

impl Actor for ChatServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        msg.addr.do_send(Message(format!("{{\"player_uuid\":\"{}\"}}", msg.uuid.to_string())));
        self.sessions.insert(msg.uuid, msg.addr);
        self.visitor_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        if self.sessions.contains_key(&msg.uuid) {
            self.sessions.remove(&msg.uuid);
            self.visitor_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl Handler<ClientMessage> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        self.send_message(&msg.room_uuid, msg.msg.as_str());
    }
}

impl Handler<Join> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join { room_uuid, client_uuid, client_name } = msg;
        if let Some(game) = self.rooms.get_mut(&room_uuid) {
            if game.started {
                return;
            }
            let player = Player::new(&client_uuid, &client_name);
            game.add_player(player);
        }
        for recipient in self.sessions.values() {
            let lobby_state = LobbyState {
                room_uuid: room_uuid,
                num_players: self.rooms.get(&room_uuid).unwrap().players.capacity(),
                players: self.rooms.get(&room_uuid).unwrap().players.iter().map(|player| player.name.clone()).collect::<Vec<String>>(),
            };
            recipient.do_send(Message(serde_json::to_string(&lobby_state).unwrap()));
        }
    }
}

impl Handler<Create> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Create, _: &mut Context<Self>) {
        let Create { number_of_players, client_uuid, client_name } = msg;
        let room_uuid = Uuid::new_v4();
        self.rooms.insert(room_uuid, Game::new(&client_uuid, &client_name, number_of_players));
        let lobby_state = LobbyState {
            room_uuid: room_uuid,
            num_players: self.rooms.get(&room_uuid).unwrap().players.capacity(),
            players: self.rooms.get(&room_uuid).unwrap().players.iter().map(|player| player.name.clone()).collect::<Vec<String>>(),
        };
        self.sessions.get(&client_uuid).unwrap().do_send(Message(serde_json::to_string(&lobby_state).unwrap()));
    }
}

impl Handler<StartGame> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: StartGame, _: &mut Context<Self>) {
        if let Some(game) = self.rooms.get_mut(&msg.room_uuid) {
            if let Ok(_) = game.start_game() {
                for player in game.players.iter() {
                    let game_state = GameState::new(&player.uuid, game);
                    self.sessions.get(&player.uuid).unwrap().do_send(Message(serde_json::to_string(&game_state).unwrap()));
                }
            }
        }
    }
}

impl Handler<Action> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Action, _: &mut Context<Self>) {
        let room_uuid = player_to_game(&msg.client_uuid, self).unwrap();
        if let Some(game) = self.rooms.get_mut(&room_uuid) {
            let player_uuid = &msg.client_uuid;
            let player_index = game.players.iter().position(|player| player.uuid == *player_uuid).unwrap();
            let result;
            if let Some(target_name) = msg.target_name {
                let target_index = game.players.iter().position(|player| player.name == target_name).unwrap();
                let mut temp_player = game.players.get_mut(player_index).unwrap().clone();
                let mut temp_target = game.players.get_mut(target_index).unwrap().clone();

                result = match msg.action.as_str() {
                    "steal" => temp_player.steal(&mut temp_target),
                    "assassinate" => temp_player.assassinate(game, &mut temp_target),
                    "coup" => temp_player.coup(game, &mut temp_target),
                    _ => Err("Invalid targeted action"),
                };
                if result.is_ok() {
                    game.players[player_index] = temp_player;
                    game.players[target_index] = temp_target;
                }
            } else {
                let mut temp_player = game.players.get_mut(player_index).unwrap().clone();
                let mut selected_cards = vec![];

                if let Some(selected_card1) = msg.selected_card1 {
                    selected_cards.push(Card::from_str(selected_card1.as_str()));
                }

                if let Some(selected_card2) = msg.selected_card2 {
                    selected_cards.push(Card::from_str(selected_card2.as_str()));
                }

                let result1 = match msg.action.as_str() {
                    "income" => temp_player.income(game),
                    "foreign_aid" => temp_player.foreign_aid(game),
                    "tax" => temp_player.tax(game),
                    "exchange_confirm" => temp_player.exchange_confirm(game, &selected_cards),
                    _ => Ok(false),
                };

                let result2 = match msg.action.as_str() {
                    "exchange_draw" => temp_player.exchange_draw(game),
                    _ => Ok(vec![]),
                };

                if result1.unwrap() || !result2.unwrap().is_empty() {
                    game.players[player_index] = temp_player;
                }
            }
            if let Some(game) = self.rooms.get(&room_uuid) {
                for player in game.players.iter() {
                    let mut game_state = GameState::new(&player.uuid, game);
                    for p in game_state.players.iter_mut() {
                        if p.0.uuid != player.uuid {
                            p.0.cards = p.0.cards.iter().filter(|card| card.visible).cloned().collect();
                        }
                    }
                    self.sessions.get(&player.uuid).unwrap().do_send(Message(serde_json::to_string(&game_state).unwrap()));
                }
            }
        }
    }
}