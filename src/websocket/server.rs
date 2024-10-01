use std::{
    collections::HashMap,
    sync::{
        atomic::AtomicUsize,
        Arc,
    },
};

use crate::game::game::Game;
use crate::game::player::Player;

use uuid::Uuid;
use actix::prelude::*;

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

#[derive(Debug)]
pub struct ChatServer {
    sessions: HashMap<Uuid, Recipient<Message>>,
    rooms: HashMap<Uuid, Game>,
    visitor_count: Arc<AtomicUsize>,
}

impl ChatServer {
    pub fn new(visitor_count: Arc<AtomicUsize>) -> ChatServer {
        ChatServer {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
            visitor_count,
        }
    }

    fn send_message(&self, room: &Uuid, message: &str, skip_uuid: Uuid) {
        if let Some(game) = self.rooms.get(room) {
            for player in game.players.iter() {
                if player.uuid != skip_uuid {
                    if let Some(addr) = self.sessions.get(&player.uuid) {
                        addr.do_send(Message(message.to_owned()));
                    }
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
        self.send_message(&msg.room_uuid, msg.msg.as_str(), msg.client_uuid);
    }
}

impl Handler<Join> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join { room_uuid, client_uuid, client_name } = msg;
        if let Some(game) = self.rooms.get_mut(&room_uuid) {
            let player = Player::new(&client_uuid, &client_name);
            game.add_player(player);
        }
        for recipient in self.sessions.values() {
            recipient.do_send(Message(
                format!("{{\"lobbyState\":{{\"sessionUUID\":\"{}\",\"numPlayers\":{},\"players\":{:?}}}}}",
                room_uuid,
                self.rooms.get(&room_uuid).unwrap().players.capacity(),
                self.rooms.get(&room_uuid).unwrap().players.iter().map(|player| player.name.clone()).collect::<Vec<String>>(),
            )));
        }
    }
}

impl Handler<Create> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Create, _: &mut Context<Self>) {
        let Create { number_of_players, client_uuid, client_name } = msg;
        let room_uuid = Uuid::new_v4();
        self.rooms.insert(room_uuid, Game::new(&client_uuid, &client_name, number_of_players));
        self.sessions.get(&client_uuid).unwrap().do_send(Message(
            format!("{{\"lobbyState\":{{\"sessionUUID\":\"{}\",\"numPlayers\":{},\"players\":{:?}}}}}",
            room_uuid,
            self.rooms.get(&room_uuid).unwrap().players.capacity(),
            self.rooms.get(&room_uuid).unwrap().players.iter().map(|player| player.name.clone()).collect::<Vec<String>>(),
        )));
    }
}