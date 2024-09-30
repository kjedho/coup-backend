use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::AtomicUsize,
        Arc,
    },
};

use uuid::Uuid;
use actix::prelude::*;
use rand::rngs::ThreadRng;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub addr: Recipient<Message>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub addr: Recipient<Message>,
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
    pub client_name: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Create {
    pub number_of_players: usize,
    pub client_name: String,
}

#[derive(Debug)]
pub struct ChatServer {
    sessions: HashMap<Uuid, Recipient<Message>>,
    rooms: HashMap<Uuid, HashSet<Uuid>>,
    rng: ThreadRng,
    visitor_count: Arc<AtomicUsize>,
}

impl ChatServer {
    pub fn new(visitor_count: Arc<AtomicUsize>) -> ChatServer {
        ChatServer {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
            rng: rand::thread_rng(),
            visitor_count,
        }
    }

    fn send_message(&self, room: &Uuid, message: &str, skip_uuid: Uuid) {
        if let Some(sessions) = self.rooms.get(room) {
            for id in sessions {
                if *id != skip_uuid {
                    if let Some(addr) = self.sessions.get(id) {
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
        let uuid = Uuid::new_v4();
        self.sessions.insert(uuid, msg.addr);
    }
}

impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let uuid = self.sessions.iter().find_map(|(uuid, addr)| if addr == &msg.addr { Some(*uuid) } else { None });
        if let Some(uuid) = uuid {
            self.sessions.remove(&uuid);
            self.rooms.iter_mut().for_each(|(_, sessions)| {
                sessions.remove(&uuid);
            });
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
        let Join { room_uuid, client_name } = msg;

    }
}

impl Handler<Create> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Create, _: &mut Context<Self>) {
        let Create { number_of_players, client_name } = msg;
        self.rooms.insert(name.clone(), HashSet::new());
    }
}