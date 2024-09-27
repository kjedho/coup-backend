use std::{
    net::Ipv4Addr,
    sync::{
        Arc,
        Mutex,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
    collections::HashMap,
    time::Instant,
};
use actix::*;
use actix_web::{middleware::Logger, Error, HttpRequest, HttpResponse, Responder, http, App, HttpServer, web};
use actix_web_actors::ws;
use actix_cors::Cors;
use uuid::Uuid;

mod api;
mod websocket;
use websocket::server;
use websocket::session;

mod game;
use game::game::Game;

type GameDb = Arc<Mutex<HashMap<Uuid, Game>>>;

/// Entry point for our websocket route
async fn chat_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::ChatServer>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        session::WsChatSession {
            id: 0,
            hb: Instant::now(),
            room: "main".to_owned(),
            name: None,
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

/// Displays state
async fn get_count(count: web::Data<AtomicUsize>) -> impl Responder {
    let current_count = count.load(Ordering::SeqCst);
    format!("Visitors: {current_count}")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let ip: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
    let port: u16 = 8080;
    let game_sessions: GameDb = Arc::new(Mutex::new(HashMap::<Uuid, Game>::new()));
    // set up applications state
    // keep a count of the number of visitors
    let app_state = Arc::new(AtomicUsize::new(0));

    // start chat server actor
    let server = server::ChatServer::new(app_state.clone()).start();

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
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(server.clone()))
            .service(api::api::create_game)
            .service(api::api::get_game)
            .service(api::api::get_game_sessions)
            .service(api::api::add_player)
            .route("/count", web::get().to(get_count))
            .route("/ws", web::get().to(chat_route))
            .wrap(Logger::default())
    })
        .bind((ip, port))?
        .run()
        .await
}