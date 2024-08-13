use actix_web::error::{Error, ErrorNotFound};
use actix_web::{get, post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::objects::game::Game;
use crate::objects::player::Player;
use crate::GameDb;

#[derive(Serialize, Deserialize)]
struct GameCreation {
    creator: String,
    num_players: usize,
}

#[post("/create_game")]
async fn create_game(
    creation_data: web::Json<GameCreation>,
    game_sessions: web::Data<GameDb>,
) -> impl Responder {
    let mut game_sessions = game_sessions.lock().unwrap();
    let game_uuid: Uuid = Uuid::new_v4();
    let new_game = Game::new(&creation_data.creator, creation_data.num_players);
    game_sessions.insert(game_uuid, new_game);
    HttpResponse::Created().json(game_uuid)
}

#[derive(Serialize, Deserialize)]
struct AddPlayer {
    uuid: Uuid,
    player_name: String,
}

#[post("/add_player")]
async fn add_player(
    add_player_data: web::Json<AddPlayer>,
    game_sessions: web::Data<GameDb>,
) -> impl Responder {
    let mut game_sessions = game_sessions.lock().unwrap();
    let game = game_sessions.get_mut(&add_player_data.uuid).unwrap();
    game.add_player(Player::new(&add_player_data.player_name));
    HttpResponse::Created().json(&add_player_data.player_name)
}

#[post("/start_game/{uuid}")]
async fn start_game(
    game_uuid: web::Path<Uuid>,
    game_sessions: web::Data<GameDb>,
) -> impl Responder {
    let mut game_sessions = game_sessions.lock().unwrap();
    let game = game_sessions.get_mut(&game_uuid).unwrap();
    let start_player = game.start_game();
    HttpResponse::Created().json(start_player)
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