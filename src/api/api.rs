use actix_web::error::{Error, ErrorNotFound};
use actix_web::{get, post, web, HttpResponse, Responder};
use uuid::Uuid;

use crate::objects::game::Game;
use crate::GameDb;

#[post("/create_game")]
async fn create_game(
    // game_data: web::Json<Game>,
    game_sessions: web::Data<GameDb>,
) -> impl Responder {
    let mut game_sessions = game_sessions.lock().unwrap();
    let game_uuid: Uuid = Uuid::new_v4();
    // game_sessions.insert(game_uuid, game_data.into_inner());
    game_sessions.insert(game_uuid, Game::new());
    HttpResponse::Created().json(game_uuid)
}

#[get("/get_game/{uuid}")]
async fn get_game(
    game_uuid: web::Path<Uuid>,
    game_sessions: web::Data<GameDb>,
) -> Result<impl Responder, Error> {
    let game_uuid = game_uuid.into_inner();
    let mut game_sessions = game_sessions.lock().unwrap();

    match game_sessions.get(&game_uuid) {
        Some(game) => Ok(HttpResponse::Ok().json(game)),
        None => Err(ErrorNotFound("Game not found")),
    }
}

#[get("/get_game_sessions")]
async fn get_game_sessions(
    game_sessions: web::Data<GameDb>
) -> impl Responder {
    let num_keys = game_sessions.lock().unwrap().keys().len();
    HttpResponse::Ok().body("Number of game sessions: ".to_string() + &num_keys.to_string())
}