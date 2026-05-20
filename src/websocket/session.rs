use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web_actors::ws;
use uuid::Uuid;

use super::server;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct WsChatSession {
    pub uuid: Uuid,
    pub hb: Instant,
    pub addr: Addr<server::ChatServer>,
}

impl WsChatSession {
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Websocket Client heartbeat failed, disconnecting!");
                act.addr.do_send(server::Disconnect { uuid: act.uuid });
                ctx.stop();
                return;
            }

            ctx.ping(b"PING");
        });
    }
}

impl Actor for WsChatSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr = ctx.address();
        let uuid = Uuid::new_v4();
        self.addr
            .send(server::Connect {
                addr: addr.recipient(),
                uuid,
            })
            .into_actor(self)
            .then(move |res, act, ctx| {
                match res {
                    Ok(_res) => act.uuid = uuid,
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.addr.do_send(server::Disconnect { uuid: self.uuid });
        Running::Stop
    }
}

impl Handler<server::Message> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsChatSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                let m = text.trim();
                if m.starts_with('/') {
                    let v: Vec<&str> = m.splitn(5, ' ').collect();
                    match v[0] {
                        "/join_lobby" => {
                            if v.len() == 3 {
                                let room_uuid = match Uuid::parse_str(v[1]) {
                                    Ok(uuid) => uuid,
                                    Err(_) => {
                                        ctx.text("Invalid room UUID format.");
                                        return;
                                    }
                                };
                                self.addr.do_send(server::Join {
                                    room_uuid,
                                    client_uuid: self.uuid,
                                    client_name: v[2].to_owned(),
                                });
                            } else {
                                ctx.text("Could not join lobby: game UUID and player name required.");
                            }
                        }
                        "/create_lobby" => {
                            if v.len() == 3 {
                                let num_players = match v[1].parse::<usize>() {
                                    Ok(n) => n,
                                    Err(_) => {
                                        ctx.text("Invalid number of players.");
                                        return;
                                    }
                                };
                                self.addr.do_send(server::Create {
                                    number_of_players: num_players,
                                    client_uuid: self.uuid,
                                    client_name: v[2].to_owned(),
                                });
                            } else {
                                ctx.text(
                                    "Could not create lobby: number of players and player name required.",
                                );
                            }
                        }
                        "/start_game" => {
                            if v.len() >= 2 {
                                let room_uuid = match Uuid::parse_str(v[1]) {
                                    Ok(uuid) => uuid,
                                    Err(_) => {
                                        ctx.text("Invalid room UUID format.");
                                        return;
                                    }
                                };
                                self.addr.do_send(server::StartGame { room_uuid });
                            } else {
                                ctx.text("Room UUID required.");
                            }
                        }
                        "/action" => {
                            if v.len() >= 2 {
                                if v[1] == "exchange_confirm" {
                                    self.addr.do_send(server::Action {
                                        client_uuid: self.uuid,
                                        action: v[1].to_owned(),
                                        target_name: None,
                                        selected_card1: v.get(2).map(|s| s.to_string()),
                                        selected_card2: v.get(3).map(|s| s.to_string()),
                                    });
                                } else {
                                    self.addr.do_send(server::Action {
                                        client_uuid: self.uuid,
                                        action: v[1].to_owned(),
                                        target_name: v.get(2).map(|s| s.to_string()),
                                        selected_card1: None,
                                        selected_card2: None,
                                    });
                                }
                            } else {
                                ctx.text("Action name required.");
                            }
                        }
                        "/lose_influence" => {
                            if v.len() >= 2 {
                                self.addr.do_send(server::LoseInfluence {
                                    client_uuid: self.uuid,
                                    card_role: v[1].to_owned(),
                                });
                            } else {
                                ctx.text("Card role required for lose_influence.");
                            }
                        }
                        "/challenge" => {
                            self.addr.do_send(server::ChallengeAction {
                                client_uuid: self.uuid,
                            });
                        }
                        "/allow" => {
                            self.addr.do_send(server::AllowAction {
                                client_uuid: self.uuid,
                            });
                        }
                        "/block" => {
                            if v.len() >= 2 {
                                self.addr.do_send(server::BlockAction {
                                    client_uuid: self.uuid,
                                    claimed_role: v[1].to_owned(),
                                });
                            } else {
                                ctx.text("Role required for block.");
                            }
                        }
                        _ => ctx.text(format!("Unknown command: {m:?}")),
                    }
                }
            }
            ws::Message::Binary(_) => println!("Unexpected binary"),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}
