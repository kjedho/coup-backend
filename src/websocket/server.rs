use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use crate::game::action_properties::{display_action_name, get_action_properties};
use crate::game::card::{Card, Role};
use crate::game::game::Game;
use crate::game::player::Player;
use crate::game::turn_phase::{AfterInfluenceLoss, BlockInfo, TurnContext, TurnPhase};

use super::state::{CardView, GameState, LobbyState, ServerMessage};

use actix::prelude::*;
use uuid::Uuid;

const PHASE_DEADLINE_SECS: u64 = 10;

// --- Message types ---

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
    pub room_uuid: Uuid,
    pub msg: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Join {
    pub room_code: String,
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
    pub room_code: String,
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

#[derive(Message)]
#[rtype(result = "()")]
pub struct LoseInfluence {
    pub client_uuid: Uuid,
    pub card_role: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ChallengeAction {
    pub client_uuid: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AllowAction {
    pub client_uuid: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BlockAction {
    pub client_uuid: Uuid,
    pub claimed_role: String,
}

// --- Server ---

#[derive(Debug)]
pub struct ChatServer {
    sessions: HashMap<Uuid, Recipient<Message>>,
    rooms: HashMap<Uuid, Game>,
    visitor_count: Arc<AtomicUsize>,
}

fn player_to_game(player_uuid: &Uuid, server: &ChatServer) -> Option<Uuid> {
    for (room_uuid, game) in server.rooms.iter() {
        if game.players.iter().any(|player| player.uuid == *player_uuid) {
            return Some(*room_uuid);
        }
    }
    None
}

fn parse_role(s: &str) -> Option<Role> {
    match s {
        "Assassin" => Some(Role::Assassin),
        "Contessa" => Some(Role::Contessa),
        "Captain" => Some(Role::Captain),
        "Duke" => Some(Role::Duke),
        "Ambassador" => Some(Role::Ambassador),
        _ => None,
    }
}

// --- Action processing ---

enum ActionOutcome {
    TurnComplete,
    WaitingForInfluenceChoice(Uuid),
    WaitingForExchange,
    PhaseStarted,
}

/// Declare an action. For challengeable/blockable actions, creates a TurnContext
/// instead of executing immediately. Cost is always deducted at declaration time.
fn process_action(
    game: &mut Game,
    client_uuid: &Uuid,
    action: &str,
    target_name: Option<&str>,
    selected_cards: Vec<Card>,
) -> Result<ActionOutcome, String> {
    if game.pending_influence_loss.is_some() {
        return Err("Waiting for a player to choose which influence to lose".to_string());
    }
    if game.turn_context.is_some() {
        return Err("A turn phase is already in progress".to_string());
    }

    let player_index = game.current_player;
    if game.players[player_index].uuid != *client_uuid {
        return Err("It's not your turn".to_string());
    }

    // Exchange confirm is allowed during an exchange
    if action == "exchange_confirm" {
        if game.players[player_index].exchange_cards.is_empty() {
            return Err("No exchange in progress".to_string());
        }
        let mut temp = game.players[player_index].clone();
        temp.exchange_confirm(game, &selected_cards)
            .map_err(|e| e.to_string())?;
        game.players[player_index] = temp;
        return Ok(ActionOutcome::TurnComplete);
    }

    // Block other actions during exchange
    if !game.players[player_index].exchange_cards.is_empty() {
        return Err("Must complete exchange first".to_string());
    }

    // Forced coup
    if game.players[player_index].coins >= 10 && action != "coup" {
        return Err("You must coup when you have 10 or more coins".to_string());
    }

    let props = get_action_properties(action)
        .ok_or_else(|| "Unknown action".to_string())?;

    if game.players[player_index].coins < props.cost {
        return Err("Not enough coins".to_string());
    }

    // Resolve target
    let (target_uuid, target_name_owned) = if props.requires_target {
        let name = target_name.ok_or_else(|| "Target required".to_string())?;
        let target = game
            .players
            .iter()
            .find(|p| p.name == name && p.is_alive)
            .ok_or_else(|| "Target not found".to_string())?;
        if target.uuid == *client_uuid {
            return Err("Cannot target yourself".to_string());
        }
        (Some(target.uuid), Some(name.to_string()))
    } else {
        (None, None)
    };

    // Deduct cost immediately (assassinate 3, coup 7)
    if props.cost > 0 {
        game.players[player_index].coins -= props.cost;
        game.coins += props.cost;
    }

    let is_challengeable = props.claimed_role.is_some();
    let is_blockable = !props.blockable_by.is_empty();

    // Actions that are neither challengeable nor blockable execute immediately
    if !is_challengeable && !is_blockable {
        return execute_action_effect(game, action, *client_uuid, target_uuid);
    }

    // Create TurnContext for deferred resolution
    let first_phase = if is_challengeable {
        TurnPhase::AwaitingChallengeResponses
    } else {
        TurnPhase::AwaitingBlockResponses
    };

    let actor = &game.players[player_index];
    game.turn_context = Some(TurnContext::new(
        action.to_string(),
        actor.uuid,
        actor.name.clone(),
        target_uuid,
        target_name_owned,
        props.claimed_role,
        first_phase,
    ));

    Ok(ActionOutcome::PhaseStarted)
}

/// Apply the effect of an action after challenge/block resolution.
/// Cost has already been deducted in process_action.
fn execute_action_effect(
    game: &mut Game,
    action: &str,
    actor_uuid: Uuid,
    target_uuid: Option<Uuid>,
) -> Result<ActionOutcome, String> {
    let actor_index = game
        .players
        .iter()
        .position(|p| p.uuid == actor_uuid)
        .ok_or_else(|| "Actor not found".to_string())?;

    match action {
        "income" => {
            if game.coins == 0 {
                return Err("No coins left".to_string());
            }
            game.players[actor_index].coins += 1;
            game.coins -= 1;
            Ok(ActionOutcome::TurnComplete)
        }
        "foreign_aid" => {
            let coins = std::cmp::min(game.coins, 2);
            game.players[actor_index].coins += coins;
            game.coins -= coins;
            Ok(ActionOutcome::TurnComplete)
        }
        "tax" => {
            let coins = std::cmp::min(game.coins, 3);
            game.players[actor_index].coins += coins;
            game.coins -= coins;
            Ok(ActionOutcome::TurnComplete)
        }
        "exchange_draw" => {
            let mut temp = game.players[actor_index].clone();
            temp.exchange_draw(game).map_err(|e| e.to_string())?;
            game.players[actor_index] = temp;
            Ok(ActionOutcome::WaitingForExchange)
        }
        "coup" | "assassinate" => {
            let target_uuid = target_uuid.ok_or_else(|| "Target required".to_string())?;
            let target_index = game
                .players
                .iter()
                .position(|p| p.uuid == target_uuid)
                .ok_or_else(|| "Target not found".to_string())?;

            if !game.players[target_index].is_alive {
                return Ok(ActionOutcome::TurnComplete);
            }

            let hidden = game.players[target_index]
                .cards
                .iter()
                .filter(|c| !c.visible)
                .count();
            if hidden <= 1 {
                game.players[target_index]
                    .lose_influence(None)
                    .map_err(|e| e.to_string())?;
                Ok(ActionOutcome::TurnComplete)
            } else {
                game.pending_influence_loss = Some(target_uuid);
                if let Some(ref mut ctx) = game.turn_context {
                    ctx.after_influence_loss = Some(AfterInfluenceLoss::ActionComplete);
                }
                Ok(ActionOutcome::WaitingForInfluenceChoice(target_uuid))
            }
        }
        "steal" => {
            let target_uuid = target_uuid.ok_or_else(|| "Target required".to_string())?;
            let target_index = game
                .players
                .iter()
                .position(|p| p.uuid == target_uuid)
                .ok_or_else(|| "Target not found".to_string())?;
            let coins = std::cmp::min(game.players[target_index].coins, 2);
            game.players[actor_index].coins += coins;
            game.players[target_index].coins -= coins;
            Ok(ActionOutcome::TurnComplete)
        }
        _ => Err("Unknown action".to_string()),
    }
}

/// Swap a revealed card back into the deck and draw a new one.
/// Used when a player successfully defends against a challenge.
fn swap_revealed_card(game: &mut Game, player_uuid: Uuid, role: Role) {
    let player_index = match game.players.iter().position(|p| p.uuid == player_uuid) {
        Some(i) => i,
        None => return,
    };
    if let Some(card) = game.players[player_index]
        .cards
        .iter_mut()
        .find(|c| c.role == role && !c.visible)
    {
        let old_card = *card;
        if game.deck.return_card(old_card).is_ok() {
            if let Some(new_card) = game.deck.draw() {
                *card = new_card;
            }
        }
    }
}

// --- ChatServer implementation ---

impl ChatServer {
    pub fn new(visitor_count: Arc<AtomicUsize>) -> ChatServer {
        ChatServer {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
            visitor_count,
        }
    }

    /// Find a room UUID by its short room code.
    fn find_room_by_code(&self, code: &str) -> Option<Uuid> {
        let code_upper = code.to_uppercase();
        for (room_uuid, game) in self.rooms.iter() {
            if game.room_code == code_upper {
                return Some(*room_uuid);
            }
        }
        None
    }

    /// Remove a player from any room they are currently in.
    /// Cleans up empty rooms.
    fn remove_from_rooms(&mut self, player_uuid: &Uuid) {
        let room_uuid = player_to_game(player_uuid, self);
        if let Some(room_uuid) = room_uuid {
            let should_remove = {
                let game = match self.rooms.get_mut(&room_uuid) {
                    Some(g) => g,
                    None => return,
                };
                game.players.retain(|p| p.uuid != *player_uuid);
                game.players.is_empty()
            };
            if should_remove {
                self.rooms.remove(&room_uuid);
            }
        }
    }

    fn send_to_player(&self, player_uuid: &Uuid, message: &ServerMessage) {
        if let Some(addr) = self.sessions.get(player_uuid) {
            if let Ok(json) = serde_json::to_string(message) {
                addr.do_send(Message(json));
            }
        }
    }

    fn send_error(&self, player_uuid: &Uuid, msg: &str) {
        self.send_to_player(
            player_uuid,
            &ServerMessage::Error {
                message: msg.to_string(),
            },
        );
    }

    fn broadcast_game_state(&self, room_uuid: &Uuid) {
        if let Some(game) = self.rooms.get(room_uuid) {
            for player in game.players.iter() {
                let game_state = GameState::for_player(&player.uuid, game);
                self.send_to_player(&player.uuid, &ServerMessage::GameState(game_state));
            }
        }
    }

    fn broadcast_to_room(&self, room_uuid: &Uuid, message: &ServerMessage) {
        if let Some(game) = self.rooms.get(room_uuid) {
            if let Ok(json) = serde_json::to_string(message) {
                for player in game.players.iter() {
                    if let Some(addr) = self.sessions.get(&player.uuid) {
                        addr.do_send(Message(json.clone()));
                    }
                }
            }
        }
    }

    /// Check game over, advance turn if not over. Returns true if game is over.
    fn check_game_over_and_advance(&mut self, room_uuid: &Uuid) -> bool {
        let game_over_winner = {
            let game = match self.rooms.get(room_uuid) {
                Some(g) => g,
                None => return false,
            };
            game.check_game_over().map(|w| w.name.clone())
        };

        if let Some(winner) = game_over_winner {
            self.broadcast_to_room(
                room_uuid,
                &ServerMessage::GameOver {
                    winner: winner.clone(),
                },
            );
            self.broadcast_game_state(room_uuid);
            return true;
        }

        if let Some(game) = self.rooms.get_mut(room_uuid) {
            game.next_player();
        }

        false
    }

    fn finish_turn(&mut self, room_uuid: &Uuid) {
        let game_over = self.check_game_over_and_advance(room_uuid);
        if !game_over {
            self.broadcast_game_state(room_uuid);
        }
    }

    // --- Phase management ---

    /// Get UUIDs of players eligible to respond in the current phase.
    fn get_eligible_responders(&self, room_uuid: &Uuid) -> Vec<Uuid> {
        let game = match self.rooms.get(room_uuid) {
            Some(g) => g,
            None => return vec![],
        };
        let turn_ctx = match &game.turn_context {
            Some(c) => c,
            None => return vec![],
        };

        match &turn_ctx.phase {
            TurnPhase::AwaitingChallengeResponses => game
                .players
                .iter()
                .filter(|p| p.is_alive && p.uuid != turn_ctx.actor_uuid)
                .map(|p| p.uuid)
                .collect(),
            TurnPhase::AwaitingBlockResponses => {
                let block_target_only = get_action_properties(&turn_ctx.action)
                    .map(|p| p.block_target_only)
                    .unwrap_or(false);

                if block_target_only {
                    turn_ctx
                        .target_uuid
                        .iter()
                        .filter(|uuid| game.players.iter().any(|p| p.uuid == **uuid && p.is_alive))
                        .copied()
                        .collect()
                } else {
                    game.players
                        .iter()
                        .filter(|p| p.is_alive && p.uuid != turn_ctx.actor_uuid)
                        .map(|p| p.uuid)
                        .collect()
                }
            }
            TurnPhase::AwaitingBlockChallengeResponses => {
                let blocker_uuid = turn_ctx.block_info.as_ref().map(|b| b.blocker_uuid);
                game.players
                    .iter()
                    .filter(|p| p.is_alive && Some(p.uuid) != blocker_uuid)
                    .map(|p| p.uuid)
                    .collect()
            }
        }
    }

    /// Send prompts for the current phase and schedule a timeout.
    fn start_current_phase(&mut self, room_uuid: &Uuid, ctx: &mut Context<Self>) {
        // Bump generation and clear responses
        let generation = {
            let game = match self.rooms.get_mut(room_uuid) {
                Some(g) => g,
                None => return,
            };
            let turn_ctx = match game.turn_context.as_mut() {
                Some(c) => c,
                None => return,
            };
            turn_ctx.responded.clear();
            turn_ctx.next_generation()
        };

        // If no one is eligible to respond, auto-resolve
        let eligible = self.get_eligible_responders(room_uuid);
        if eligible.is_empty() {
            self.on_all_allowed(room_uuid, ctx);
            return;
        }

        // Broadcast game state first, then send prompts.
        // This ordering lets the frontend clear stale prompts on game_state,
        // then set new ones from the prompt messages that follow.
        self.broadcast_game_state(room_uuid);

        // Send prompts to eligible players
        let game = match self.rooms.get(room_uuid) {
            Some(g) => g,
            None => return,
        };
        let turn_ctx = match game.turn_context.as_ref() {
            Some(c) => c,
            None => return,
        };

        let action_display = display_action_name(&turn_ctx.action);

        match &turn_ctx.phase {
            TurnPhase::AwaitingChallengeResponses => {
                let msg = ServerMessage::ChallengePrompt {
                    actor: turn_ctx.actor_name.clone(),
                    action: action_display.clone(),
                    claimed_role: turn_ctx
                        .claimed_role
                        .map(|r| format!("{:?}", r))
                        .unwrap_or_default(),
                    target: turn_ctx.target_name.clone(),
                    deadline_secs: PHASE_DEADLINE_SECS,
                };
                let actor_uuid = turn_ctx.actor_uuid;
                for player in game.players.iter() {
                    if player.is_alive && player.uuid != actor_uuid {
                        self.send_to_player(&player.uuid, &msg);
                    }
                }
            }
            TurnPhase::AwaitingBlockResponses => {
                let props = get_action_properties(&turn_ctx.action);
                let blockable_by: Vec<String> = props
                    .as_ref()
                    .map(|p| p.blockable_by.iter().map(|r| format!("{:?}", r)).collect())
                    .unwrap_or_default();
                let block_target_only = props
                    .as_ref()
                    .map(|p| p.block_target_only)
                    .unwrap_or(false);

                let msg = ServerMessage::BlockPrompt {
                    actor: turn_ctx.actor_name.clone(),
                    action: action_display.clone(),
                    blockable_by,
                    target: turn_ctx.target_name.clone(),
                    deadline_secs: PHASE_DEADLINE_SECS,
                };

                let actor_uuid = turn_ctx.actor_uuid;
                let target_uuid = turn_ctx.target_uuid;

                if block_target_only {
                    if let Some(target_uuid) = target_uuid {
                        if game.players.iter().any(|p| p.uuid == target_uuid && p.is_alive) {
                            self.send_to_player(&target_uuid, &msg);
                        }
                    }
                } else {
                    for player in game.players.iter() {
                        if player.is_alive && player.uuid != actor_uuid {
                            self.send_to_player(&player.uuid, &msg);
                        }
                    }
                }
            }
            TurnPhase::AwaitingBlockChallengeResponses => {
                if let Some(ref block_info) = turn_ctx.block_info {
                    let msg = ServerMessage::BlockChallengePrompt {
                        blocker: block_info.blocker_name.clone(),
                        claimed_role: format!("{:?}", block_info.claimed_role),
                        original_action: action_display.clone(),
                        deadline_secs: PHASE_DEADLINE_SECS,
                    };
                    let blocker_uuid = block_info.blocker_uuid;
                    for player in game.players.iter() {
                        if player.is_alive && player.uuid != blocker_uuid {
                            self.send_to_player(&player.uuid, &msg);
                        }
                    }
                }
            }
        }

        // Schedule timeout
        let room = *room_uuid;
        ctx.run_later(
            Duration::from_secs(PHASE_DEADLINE_SECS),
            move |act, inner_ctx| {
                act.handle_phase_timeout(room, generation, inner_ctx);
            },
        );
    }

    fn handle_phase_timeout(
        &mut self,
        room_uuid: Uuid,
        generation: u64,
        ctx: &mut Context<Self>,
    ) {
        let game = match self.rooms.get(&room_uuid) {
            Some(g) => g,
            None => return,
        };

        // Don't fire if waiting for a player to choose which card to lose
        if game.pending_influence_loss.is_some() {
            return;
        }

        let current_gen = match &game.turn_context {
            Some(c) => c.timer_generation,
            None => return,
        };

        if current_gen != generation {
            return; // Stale timer, a new phase has started since
        }

        // Release the borrow before calling on_all_allowed
        let _ = game;
        self.on_all_allowed(&room_uuid, ctx);
    }

    /// Called when all eligible players have allowed (or timer expired).
    fn on_all_allowed(&mut self, room_uuid: &Uuid, ctx: &mut Context<Self>) {
        let (phase, action) = {
            let game = match self.rooms.get(room_uuid) {
                Some(g) => g,
                None => return,
            };
            let turn_ctx = match &game.turn_context {
                Some(c) => c,
                None => return,
            };
            (turn_ctx.phase.clone(), turn_ctx.action.clone())
        };

        match phase {
            TurnPhase::AwaitingChallengeResponses => {
                // No challenge. If blockable, move to block phase; otherwise execute.
                let is_blockable = get_action_properties(&action)
                    .map(|p| !p.blockable_by.is_empty())
                    .unwrap_or(false);

                if is_blockable {
                    if let Some(game) = self.rooms.get_mut(room_uuid) {
                        if let Some(ref mut turn_ctx) = game.turn_context {
                            turn_ctx.phase = TurnPhase::AwaitingBlockResponses;
                        }
                    }
                    self.start_current_phase(room_uuid, ctx);
                } else {
                    self.execute_and_finish(room_uuid, ctx);
                }
            }
            TurnPhase::AwaitingBlockResponses => {
                // No block. Execute the action.
                self.execute_and_finish(room_uuid, ctx);
            }
            TurnPhase::AwaitingBlockChallengeResponses => {
                // No challenge to the block. Block succeeds, turn ends.
                if let Some(game) = self.rooms.get_mut(room_uuid) {
                    game.turn_context = None;
                }
                self.finish_turn(room_uuid);
            }
        }
    }

    /// Execute the action effect and handle the result.
    fn execute_and_finish(&mut self, room_uuid: &Uuid, _ctx: &mut Context<Self>) {
        let (action, actor_uuid, target_uuid) = {
            let game = match self.rooms.get(room_uuid) {
                Some(g) => g,
                None => return,
            };
            let turn_ctx = match &game.turn_context {
                Some(c) => c,
                None => return,
            };
            (
                turn_ctx.action.clone(),
                turn_ctx.actor_uuid,
                turn_ctx.target_uuid,
            )
        };

        let outcome = {
            let game = match self.rooms.get_mut(room_uuid) {
                Some(g) => g,
                None => return,
            };
            execute_action_effect(game, &action, actor_uuid, target_uuid)
        };

        match outcome {
            Ok(ActionOutcome::TurnComplete) => {
                if let Some(game) = self.rooms.get_mut(room_uuid) {
                    game.turn_context = None;
                }
                self.finish_turn(room_uuid);
            }
            Ok(ActionOutcome::WaitingForInfluenceChoice(target_uuid)) => {
                // Broadcast game state first so frontend clears stale state,
                // then send the choice prompt which sets new state.
                self.broadcast_game_state(room_uuid);
                if let Some(game) = self.rooms.get(room_uuid) {
                    if let Some(target) = game.players.iter().find(|p| p.uuid == target_uuid) {
                        let cards: Vec<CardView> = target
                            .cards
                            .iter()
                            .filter(|c| !c.visible)
                            .map(|c| CardView {
                                role: Some(c.role),
                                visible: false,
                            })
                            .collect();
                        self.send_to_player(
                            &target_uuid,
                            &ServerMessage::LoseInfluenceChoice { cards },
                        );
                    }
                }
            }
            Ok(ActionOutcome::WaitingForExchange) => {
                if let Some(game) = self.rooms.get_mut(room_uuid) {
                    game.turn_context = None;
                }
                self.broadcast_game_state(room_uuid);
            }
            _ => {
                if let Some(game) = self.rooms.get_mut(room_uuid) {
                    game.turn_context = None;
                }
                self.finish_turn(room_uuid);
            }
        }
    }

    // --- Challenge / block resolution ---

    /// Resolve a challenge to the actor's action claim.
    fn resolve_challenge(
        &mut self,
        room_uuid: &Uuid,
        challenger_uuid: Uuid,
        ctx: &mut Context<Self>,
    ) {
        let (actor_uuid, claimed_role, actor_name, challenger_name) = {
            let game = match self.rooms.get(room_uuid) {
                Some(g) => g,
                None => return,
            };
            let turn_ctx = match &game.turn_context {
                Some(c) => c,
                None => return,
            };
            let challenger_name = game
                .players
                .iter()
                .find(|p| p.uuid == challenger_uuid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (
                turn_ctx.actor_uuid,
                turn_ctx.claimed_role,
                turn_ctx.actor_name.clone(),
                challenger_name,
            )
        };

        let claimed_role = match claimed_role {
            Some(r) => r,
            None => return,
        };

        let actor_has_role = {
            let game = self.rooms.get(room_uuid).unwrap();
            game.players
                .iter()
                .find(|p| p.uuid == actor_uuid)
                .map(|p| p.cards.iter().any(|c| c.role == claimed_role && !c.visible))
                .unwrap_or(false)
        };

        if actor_has_role {
            // Challenge failed: actor had the card
            self.broadcast_to_room(
                room_uuid,
                &ServerMessage::ActionResult {
                    message: format!(
                        "{} challenged but {} revealed {:?}! {} loses influence.",
                        challenger_name, actor_name, claimed_role, challenger_name
                    ),
                },
            );

            if let Some(game) = self.rooms.get_mut(room_uuid) {
                swap_revealed_card(game, actor_uuid, claimed_role);
            }

            self.require_influence_loss(
                room_uuid,
                challenger_uuid,
                AfterInfluenceLoss::ProceedAfterFailedChallenge,
                ctx,
            );
        } else {
            // Challenge succeeded: actor was bluffing, challenger earns a coin
            if let Some(game) = self.rooms.get_mut(room_uuid) {
                if let Some(player) = game.players.iter_mut().find(|p| p.uuid == challenger_uuid) {
                    player.coins += 1;
                    if game.coins > 0 {
                        game.coins -= 1;
                    }
                }
            }

            self.broadcast_to_room(
                room_uuid,
                &ServerMessage::ActionResult {
                    message: format!(
                        "{} challenged and {} was bluffing! {} loses influence. {} earns 1 coin.",
                        challenger_name, actor_name, actor_name, challenger_name
                    ),
                },
            );

            self.require_influence_loss(
                room_uuid,
                actor_uuid,
                AfterInfluenceLoss::TurnEnds,
                ctx,
            );
        }
    }

    /// Resolve a challenge to a blocker's claim.
    fn resolve_block_challenge(
        &mut self,
        room_uuid: &Uuid,
        challenger_uuid: Uuid,
        ctx: &mut Context<Self>,
    ) {
        let (blocker_uuid, blocker_name, claimed_role, challenger_name) = {
            let game = match self.rooms.get(room_uuid) {
                Some(g) => g,
                None => return,
            };
            let turn_ctx = match &game.turn_context {
                Some(c) => c,
                None => return,
            };
            let block_info = match &turn_ctx.block_info {
                Some(b) => b,
                None => return,
            };
            let challenger_name = game
                .players
                .iter()
                .find(|p| p.uuid == challenger_uuid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (
                block_info.blocker_uuid,
                block_info.blocker_name.clone(),
                block_info.claimed_role,
                challenger_name,
            )
        };

        let blocker_has_role = {
            let game = self.rooms.get(room_uuid).unwrap();
            game.players
                .iter()
                .find(|p| p.uuid == blocker_uuid)
                .map(|p| p.cards.iter().any(|c| c.role == claimed_role && !c.visible))
                .unwrap_or(false)
        };

        if blocker_has_role {
            // Block-challenge failed: blocker had the card, block succeeds
            self.broadcast_to_room(
                room_uuid,
                &ServerMessage::ActionResult {
                    message: format!(
                        "{} challenged the block but {} revealed {:?}! {} loses influence.",
                        challenger_name, blocker_name, claimed_role, challenger_name
                    ),
                },
            );

            if let Some(game) = self.rooms.get_mut(room_uuid) {
                swap_revealed_card(game, blocker_uuid, claimed_role);
            }

            self.require_influence_loss(
                room_uuid,
                challenger_uuid,
                AfterInfluenceLoss::BlockSucceeds,
                ctx,
            );
        } else {
            // Block-challenge succeeded: blocker was bluffing, challenger earns a coin
            if let Some(game) = self.rooms.get_mut(room_uuid) {
                if let Some(player) = game.players.iter_mut().find(|p| p.uuid == challenger_uuid) {
                    player.coins += 1;
                    if game.coins > 0 {
                        game.coins -= 1;
                    }
                }
            }

            self.broadcast_to_room(
                room_uuid,
                &ServerMessage::ActionResult {
                    message: format!(
                        "{} challenged the block and {} was bluffing! {} loses influence. {} earns 1 coin.",
                        challenger_name, blocker_name, blocker_name, challenger_name
                    ),
                },
            );

            self.require_influence_loss(
                room_uuid,
                blocker_uuid,
                AfterInfluenceLoss::ExecuteAction,
                ctx,
            );
        }
    }

    // --- Influence loss ---

    /// Make a player lose influence. If they have 2 hidden cards, prompt them to choose.
    /// If they have 1 or 0, auto-lose and immediately proceed.
    fn require_influence_loss(
        &mut self,
        room_uuid: &Uuid,
        player_uuid: Uuid,
        after: AfterInfluenceLoss,
        ctx: &mut Context<Self>,
    ) {
        let hidden_count = {
            let game = match self.rooms.get(room_uuid) {
                Some(g) => g,
                None => return,
            };
            game.players
                .iter()
                .find(|p| p.uuid == player_uuid)
                .map(|p| p.cards.iter().filter(|c| !c.visible).count())
                .unwrap_or(0)
        };

        if hidden_count <= 1 {
            // Auto-lose the last card
            if let Some(game) = self.rooms.get_mut(room_uuid) {
                if let Some(player) = game.players.iter_mut().find(|p| p.uuid == player_uuid) {
                    let _ = player.lose_influence(None);
                }
            }
            self.after_influence_lost(room_uuid, after, ctx);
        } else {
            // Player must choose which card to lose
            if let Some(game) = self.rooms.get_mut(room_uuid) {
                game.pending_influence_loss = Some(player_uuid);
                if let Some(ref mut turn_ctx) = game.turn_context {
                    turn_ctx.after_influence_loss = Some(after);
                    // Bump generation to invalidate any pending phase timer,
                    // preventing it from firing during the influence choice period.
                    turn_ctx.next_generation();
                }
            }
            // Broadcast game state first, then send choice prompt
            self.broadcast_game_state(room_uuid);
            if let Some(game) = self.rooms.get(room_uuid) {
                if let Some(player) = game.players.iter().find(|p| p.uuid == player_uuid) {
                    let cards: Vec<CardView> = player
                        .cards
                        .iter()
                        .filter(|c| !c.visible)
                        .map(|c| CardView {
                            role: Some(c.role),
                            visible: false,
                        })
                        .collect();
                    self.send_to_player(
                        &player_uuid,
                        &ServerMessage::LoseInfluenceChoice { cards },
                    );
                }
            }
        }
    }

    /// Called after a player has lost influence. Determines what happens next
    /// based on the AfterInfluenceLoss value.
    fn after_influence_lost(
        &mut self,
        room_uuid: &Uuid,
        after: AfterInfluenceLoss,
        ctx: &mut Context<Self>,
    ) {
        match after {
            AfterInfluenceLoss::TurnEnds
            | AfterInfluenceLoss::BlockSucceeds
            | AfterInfluenceLoss::ActionComplete => {
                if let Some(game) = self.rooms.get_mut(room_uuid) {
                    game.turn_context = None;
                }
                self.finish_turn(room_uuid);
            }
            AfterInfluenceLoss::ProceedAfterFailedChallenge => {
                // Challenger lost. Move to block phase if blockable, or execute.
                let is_blockable = {
                    let game = match self.rooms.get(room_uuid) {
                        Some(g) => g,
                        None => return,
                    };
                    match &game.turn_context {
                        Some(c) => get_action_properties(&c.action)
                            .map(|p| !p.blockable_by.is_empty())
                            .unwrap_or(false),
                        None => return,
                    }
                };

                if is_blockable {
                    if let Some(game) = self.rooms.get_mut(room_uuid) {
                        if let Some(ref mut turn_ctx) = game.turn_context {
                            turn_ctx.phase = TurnPhase::AwaitingBlockResponses;
                            turn_ctx.after_influence_loss = None;
                        }
                    }
                    self.start_current_phase(room_uuid, ctx);
                } else {
                    self.execute_and_finish(room_uuid, ctx);
                }
            }
            AfterInfluenceLoss::ExecuteAction => {
                // Blocker was bluffing. Execute the original action.
                self.execute_and_finish(room_uuid, ctx);
            }
        }
    }
}

// --- Actor + Handlers ---

impl Actor for ChatServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        let response = ServerMessage::Connected {
            player_uuid: msg.uuid,
        };
        msg.addr.do_send(Message(
            serde_json::to_string(&response).unwrap_or_default(),
        ));
        self.sessions.insert(msg.uuid, msg.addr);
        self.visitor_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        if self.sessions.remove(&msg.uuid).is_none() {
            return;
        }
        self.visitor_count
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        let room_uuid = match player_to_game(&msg.uuid, self) {
            Some(uuid) => uuid,
            None => return,
        };

        // Eliminate the disconnected player
        let should_cleanup = {
            let game = match self.rooms.get_mut(&room_uuid) {
                Some(g) => g,
                None => return,
            };

            if let Some(player) = game.players.iter_mut().find(|p| p.uuid == msg.uuid) {
                player.is_alive = false;
                for card in player.cards.iter_mut() {
                    card.visible = true;
                }
            }

            let alive_count = game.players.iter().filter(|p| p.is_alive).count();
            alive_count == 0
        };

        if should_cleanup {
            self.rooms.remove(&room_uuid);
            return;
        }

        // Check if game is over after elimination
        let game_over_winner = {
            let game = match self.rooms.get(&room_uuid) {
                Some(g) => g,
                None => return,
            };
            if game.started {
                game.check_game_over().map(|w| w.name.clone())
            } else {
                None
            }
        };

        if let Some(winner) = game_over_winner {
            self.broadcast_to_room(
                &room_uuid,
                &ServerMessage::GameOver {
                    winner: winner.clone(),
                },
            );
        }
        self.broadcast_game_state(&room_uuid);
    }
}

impl Handler<ClientMessage> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        self.broadcast_to_room(
            &msg.room_uuid,
            &ServerMessage::Error {
                message: msg.msg.clone(),
            },
        );
    }
}

impl Handler<Join> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join {
            room_code,
            client_uuid,
            client_name,
        } = msg;

        let room_uuid = match self.find_room_by_code(&room_code) {
            Some(uuid) => uuid,
            None => {
                self.send_error(&client_uuid, "Room not found");
                return;
            }
        };

        self.remove_from_rooms(&client_uuid);

        {
            let game = match self.rooms.get_mut(&room_uuid) {
                Some(g) => g,
                None => {
                    self.send_error(&client_uuid, "Room not found");
                    return;
                }
            };
            if game.started {
                self.send_error(&client_uuid, "Game already started");
                return;
            }
            let player = Player::new(&client_uuid, &client_name);
            if let Err(e) = game.add_player(player) {
                self.send_error(&client_uuid, e);
                return;
            }
        }

        let lobby_state = {
            let game = self.rooms.get(&room_uuid).unwrap();
            LobbyState::from_game(game)
        };
        self.broadcast_to_room(&room_uuid, &ServerMessage::LobbyState(lobby_state));
    }
}

impl Handler<Create> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Create, _: &mut Context<Self>) {
        let Create {
            number_of_players,
            client_uuid,
            client_name,
        } = msg;

        self.remove_from_rooms(&client_uuid);

        let room_uuid = Uuid::new_v4();
        self.rooms.insert(
            room_uuid,
            Game::new(&client_uuid, &client_name, number_of_players),
        );

        let lobby_state = {
            let game = self.rooms.get(&room_uuid).unwrap();
            LobbyState::from_game(game)
        };
        self.send_to_player(&client_uuid, &ServerMessage::LobbyState(lobby_state));
    }
}

impl Handler<StartGame> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: StartGame, _: &mut Context<Self>) {
        let room_uuid = match self.find_room_by_code(&msg.room_code) {
            Some(uuid) => uuid,
            None => return,
        };
        {
            let game = match self.rooms.get_mut(&room_uuid) {
                Some(g) => g,
                None => return,
            };
            if let Err(e) = game.start_game() {
                if let Some(player) = game.players.first() {
                    let uuid = player.uuid;
                    let _ = game;
                    self.send_error(&uuid, e);
                }
                return;
            }
        }
        self.broadcast_game_state(&room_uuid);
    }
}

impl Handler<Action> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Action, ctx: &mut Context<Self>) {
        let room_uuid = match player_to_game(&msg.client_uuid, self) {
            Some(uuid) => uuid,
            None => {
                self.send_error(&msg.client_uuid, "You are not in a game");
                return;
            }
        };

        let selected_cards: Vec<Card> = [&msg.selected_card1, &msg.selected_card2]
            .iter()
            .filter_map(|opt| opt.as_ref())
            .filter_map(|s| Card::from_str(s.as_str()).ok())
            .collect();

        let result = {
            let game = match self.rooms.get_mut(&room_uuid) {
                Some(g) => g,
                None => return,
            };
            process_action(
                game,
                &msg.client_uuid,
                &msg.action,
                msg.target_name.as_deref(),
                selected_cards,
            )
        };

        match result {
            Ok(ActionOutcome::TurnComplete) => {
                self.finish_turn(&room_uuid);
            }
            Ok(ActionOutcome::WaitingForInfluenceChoice(target_uuid)) => {
                self.broadcast_game_state(&room_uuid);
                if let Some(game) = self.rooms.get(&room_uuid) {
                    if let Some(target) = game.players.iter().find(|p| p.uuid == target_uuid) {
                        let cards: Vec<CardView> = target
                            .cards
                            .iter()
                            .filter(|c| !c.visible)
                            .map(|c| CardView {
                                role: Some(c.role),
                                visible: false,
                            })
                            .collect();
                        self.send_to_player(
                            &target_uuid,
                            &ServerMessage::LoseInfluenceChoice { cards },
                        );
                    }
                }
            }
            Ok(ActionOutcome::WaitingForExchange) => {
                self.broadcast_game_state(&room_uuid);
            }
            Ok(ActionOutcome::PhaseStarted) => {
                self.start_current_phase(&room_uuid, ctx);
            }
            Err(e) => {
                self.send_error(&msg.client_uuid, &e);
            }
        }
    }
}

impl Handler<LoseInfluence> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: LoseInfluence, ctx: &mut Context<Self>) {
        let room_uuid = match player_to_game(&msg.client_uuid, self) {
            Some(uuid) => uuid,
            None => {
                self.send_error(&msg.client_uuid, "You are not in a game");
                return;
            }
        };

        // Validate and process the influence loss
        let result: Result<Option<AfterInfluenceLoss>, String> = (|| {
            let game = match self.rooms.get_mut(&room_uuid) {
                Some(g) => g,
                None => return Err("Game not found".to_string()),
            };

            if game.pending_influence_loss != Some(msg.client_uuid) {
                return Err("You are not the player who needs to lose influence".to_string());
            }

            let player = match game.players.iter_mut().find(|p| p.uuid == msg.client_uuid) {
                Some(p) => p,
                None => return Err("Player not found".to_string()),
            };

            player
                .lose_influence(Some(&msg.card_role))
                .map_err(|e| e.to_string())?;

            game.pending_influence_loss = None;

            // Extract after_influence_loss from turn_context
            let after = game
                .turn_context
                .as_mut()
                .and_then(|tc| tc.after_influence_loss.take());
            Ok(after)
        })();

        match result {
            Ok(Some(after)) => {
                self.after_influence_lost(&room_uuid, after, ctx);
            }
            Ok(None) => {
                // No turn_context (e.g., coup) or no after set
                self.finish_turn(&room_uuid);
            }
            Err(e) => {
                self.send_error(&msg.client_uuid, &e);
            }
        }
    }
}

impl Handler<ChallengeAction> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: ChallengeAction, ctx: &mut Context<Self>) {
        let room_uuid = match player_to_game(&msg.client_uuid, self) {
            Some(uuid) => uuid,
            None => {
                self.send_error(&msg.client_uuid, "You are not in a game");
                return;
            }
        };

        // Validate phase and eligibility
        let phase = {
            let game = match self.rooms.get(&room_uuid) {
                Some(g) => g,
                None => return,
            };
            if game.pending_influence_loss.is_some() {
                self.send_error(&msg.client_uuid, "Waiting for influence loss");
                return;
            }
            let turn_ctx = match &game.turn_context {
                Some(c) => c,
                None => {
                    self.send_error(&msg.client_uuid, "No active phase to challenge");
                    return;
                }
            };

            // Check eligibility based on phase
            let is_eligible = match &turn_ctx.phase {
                TurnPhase::AwaitingChallengeResponses => {
                    let player = game
                        .players
                        .iter()
                        .find(|p| p.uuid == msg.client_uuid);
                    player.map(|p| p.is_alive && p.uuid != turn_ctx.actor_uuid).unwrap_or(false)
                }
                TurnPhase::AwaitingBlockChallengeResponses => {
                    let blocker_uuid = turn_ctx.block_info.as_ref().map(|b| b.blocker_uuid);
                    let player = game
                        .players
                        .iter()
                        .find(|p| p.uuid == msg.client_uuid);
                    player
                        .map(|p| p.is_alive && Some(p.uuid) != blocker_uuid)
                        .unwrap_or(false)
                }
                _ => false,
            };

            if !is_eligible {
                self.send_error(&msg.client_uuid, "You cannot challenge in this phase");
                return;
            }

            turn_ctx.phase.clone()
        };

        match phase {
            TurnPhase::AwaitingChallengeResponses => {
                self.resolve_challenge(&room_uuid, msg.client_uuid, ctx);
            }
            TurnPhase::AwaitingBlockChallengeResponses => {
                self.resolve_block_challenge(&room_uuid, msg.client_uuid, ctx);
            }
            _ => {}
        }
    }
}

impl Handler<AllowAction> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: AllowAction, ctx: &mut Context<Self>) {
        let room_uuid = match player_to_game(&msg.client_uuid, self) {
            Some(uuid) => uuid,
            None => {
                self.send_error(&msg.client_uuid, "You are not in a game");
                return;
            }
        };

        // Check eligibility
        let eligible = self.get_eligible_responders(&room_uuid);
        if !eligible.contains(&msg.client_uuid) {
            self.send_error(&msg.client_uuid, "You cannot respond in this phase");
            return;
        }

        // Add to responded set and check if all have responded
        let all_responded = {
            let game = match self.rooms.get_mut(&room_uuid) {
                Some(g) => g,
                None => return,
            };
            if game.pending_influence_loss.is_some() {
                return;
            }
            let turn_ctx = match game.turn_context.as_mut() {
                Some(c) => c,
                None => return,
            };
            turn_ctx.responded.insert(msg.client_uuid);
            eligible
                .iter()
                .all(|uuid| turn_ctx.responded.contains(uuid))
        };

        if all_responded {
            self.on_all_allowed(&room_uuid, ctx);
        }
    }
}

impl Handler<BlockAction> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: BlockAction, ctx: &mut Context<Self>) {
        let room_uuid = match player_to_game(&msg.client_uuid, self) {
            Some(uuid) => uuid,
            None => {
                self.send_error(&msg.client_uuid, "You are not in a game");
                return;
            }
        };

        let claimed_role = match parse_role(&msg.claimed_role) {
            Some(r) => r,
            None => {
                self.send_error(&msg.client_uuid, "Invalid role");
                return;
            }
        };

        // Validate: must be in block phase, player must be eligible, role must be valid
        let valid = {
            let game = match self.rooms.get(&room_uuid) {
                Some(g) => g,
                None => return,
            };
            if game.pending_influence_loss.is_some() {
                self.send_error(&msg.client_uuid, "Waiting for influence loss");
                return;
            }
            let turn_ctx = match &game.turn_context {
                Some(c) => c,
                None => {
                    self.send_error(&msg.client_uuid, "No active phase");
                    return;
                }
            };

            if turn_ctx.phase != TurnPhase::AwaitingBlockResponses {
                self.send_error(&msg.client_uuid, "Cannot block in this phase");
                return;
            }

            // Check the role is a valid blocking role for this action
            let props = match get_action_properties(&turn_ctx.action) {
                Some(p) => p,
                None => return,
            };
            if !props.blockable_by.contains(&claimed_role) {
                self.send_error(&msg.client_uuid, "That role cannot block this action");
                return;
            }

            // Check player is eligible
            let eligible = if props.block_target_only {
                turn_ctx.target_uuid == Some(msg.client_uuid)
            } else {
                msg.client_uuid != turn_ctx.actor_uuid
            };
            let is_alive = game
                .players
                .iter()
                .find(|p| p.uuid == msg.client_uuid)
                .map(|p| p.is_alive)
                .unwrap_or(false);

            eligible && is_alive
        };

        if !valid {
            self.send_error(&msg.client_uuid, "You cannot block this action");
            return;
        }

        // Get blocker name and set up block info
        let blocker_name = {
            let game = self.rooms.get(&room_uuid).unwrap();
            game.players
                .iter()
                .find(|p| p.uuid == msg.client_uuid)
                .map(|p| p.name.clone())
                .unwrap_or_default()
        };

        if let Some(game) = self.rooms.get_mut(&room_uuid) {
            if let Some(ref mut turn_ctx) = game.turn_context {
                turn_ctx.block_info = Some(BlockInfo {
                    blocker_uuid: msg.client_uuid,
                    blocker_name,
                    claimed_role,
                });
                turn_ctx.phase = TurnPhase::AwaitingBlockChallengeResponses;
            }
        }

        self.start_current_phase(&room_uuid, ctx);
    }
}
