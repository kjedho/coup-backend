use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex},
    collections::HashMap,
};
use actix_web::{App, HttpServer, web};
use uuid::Uuid;

mod api;

mod objects;
use objects::game::Game;

type GameDb = Arc<Mutex<HashMap<Uuid, Game>>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let ip: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
    let port: u16 = 8080;
    let game_sessions: GameDb = Arc::new(Mutex::new(HashMap::<Uuid, Game>::new()));

    println!("Starting server at {}:{}", ip, port);

    HttpServer::new(move || {
        let app_data: web::Data<GameDb> = web::Data::new(game_sessions.clone());
        App::new()
            .app_data(app_data)
            .service(api::api::create_game)
            .service(api::api::get_game)
    })
        .bind((ip, port))?
        .run()
        .await
}