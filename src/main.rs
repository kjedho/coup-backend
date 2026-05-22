use std::{
    env,
    net::Ipv4Addr,
    sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
    time::Instant,
};
use actix::*;
use actix_web::{middleware::Logger, Error, HttpRequest, HttpResponse, Responder, App, HttpServer, http, web};
use actix_web_actors::ws;
use actix_cors::Cors;
use uuid::Uuid;

mod websocket;
mod game;

use websocket::server;
use websocket::session;

async fn chat_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::ChatServer>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        session::WsChatSession {
            uuid: Uuid::new_v4(),
            hb: Instant::now(),
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

async fn get_count(count: web::Data<AtomicUsize>) -> impl Responder {
    let current_count = count.load(Ordering::SeqCst);
    format!("Visitors: {current_count}")
}

fn build_cors() -> Cors {
    // Read allowed origins from CORS_ORIGINS env var (comma-separated)
    // Default: http://localhost:5173 for local development
    let origins_str = env::var("CORS_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    
    let origins: Vec<&str> = origins_str.split(',').map(|s| s.trim()).collect();
    
    let mut cors = Cors::default()
        .allowed_methods(vec!["GET", "POST"])
        .allowed_header(http::header::CONTENT_TYPE)
        .max_age(3600);
    
    for origin in origins {
        if !origin.is_empty() {
            cors = cors.allowed_origin(origin);
        }
    }
    
    cors
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Read host and port from environment variables with defaults
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a valid number");
    
    let ip: Ipv4Addr = host.parse().expect("HOST must be a valid IPv4 address");
    
    let app_state = Arc::new(AtomicUsize::new(0));
    let server = server::ChatServer::new(app_state.clone()).start();

    println!("Starting server at {}:{}", ip, port);

    HttpServer::new(move || {
        App::new()
            .wrap(build_cors())
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(server.clone()))
            .route("/count", web::get().to(get_count))
            .route("/ws", web::get().to(chat_route))
            .wrap(Logger::default())
    })
        .bind((ip, port))?
        .run()
        .await
}