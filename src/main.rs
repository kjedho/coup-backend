use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex},
    collections::HashMap,
};
use actix_web::{http, App, HttpServer, web};
use actix_cors::Cors;
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
        let cors = Cors::default()
            .allowed_origin("http://localhost:5173")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_header(http::header::CONTENT_TYPE)
            .max_age(3600);
        App::new()
            .wrap(cors)
            .app_data(app_data)
            .service(api::api::create_game)
            .service(api::api::get_game)
            .service(api::api::get_game_sessions)
            .service(api::api::add_player)
    })
        .bind((ip, port))?
        .run()
        .await
}