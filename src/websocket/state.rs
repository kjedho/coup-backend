use crate::game::action_properties::display_action_name;
use crate::game::card::Role;
use crate::game::game::{AvailableAction, Game};
use crate::game::player::Player;
use crate::game::turn_phase::TurnPhase;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
pub struct CardView {
    pub role: Option<Role>,
    pub visible: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct PlayerView {
    pub name: String,
    pub is_alive: bool,
    pub coins: u8,
    pub cards: Vec<CardView>,
    pub exchange_cards: Vec<CardView>,
    pub is_current_turn: bool,
    pub is_self: bool,
}

impl PlayerView {
    pub fn from_player(player: &Player, is_self: bool, is_current_turn: bool) -> Self {
        let cards = player
            .cards
            .iter()
            .map(|card| {
                if is_self || card.visible {
                    CardView {
                        role: Some(card.role),
                        visible: card.visible,
                    }
                } else {
                    CardView {
                        role: None,
                        visible: false,
                    }
                }
            })
            .collect();

        let exchange_cards = if is_self {
            player
                .exchange_cards
                .iter()
                .map(|card| CardView {
                    role: Some(card.role),
                    visible: card.visible,
                })
                .collect()
        } else {
            vec![]
        };

        PlayerView {
            name: player.name.clone(),
            is_alive: player.is_alive,
            coins: player.coins,
            cards,
            exchange_cards,
            is_current_turn,
            is_self,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct GameState {
    pub title: String,
    pub subtitle: String,
    pub players: Vec<PlayerView>,
    pub coins: u8,
    pub current_player: String,
    pub available_actions: Vec<AvailableAction>,
}

impl GameState {
    pub fn for_player(player_uuid: &Uuid, game: &Game) -> Self {
        let current = &game.players[game.current_player];

        let subtitle = if let Some(pending_uuid) = &game.pending_influence_loss {
            let pending_player = game
                .players
                .iter()
                .find(|p| p.uuid == *pending_uuid)
                .unwrap();
            format!("{} is choosing which influence to lose", pending_player.name)
        } else if let Some(ctx) = &game.turn_context {
            let action = display_action_name(&ctx.action);
            match &ctx.phase {
                TurnPhase::AwaitingChallengeResponses => {
                    if let Some(ref target_name) = ctx.target_name {
                        format!(
                            "{} claims {} to {} {}",
                            ctx.actor_name,
                            ctx.claimed_role
                                .map(|r| format!("{:?}", r))
                                .unwrap_or_default(),
                            action,
                            target_name
                        )
                    } else {
                        format!(
                            "{} claims {} to {}",
                            ctx.actor_name,
                            ctx.claimed_role
                                .map(|r| format!("{:?}", r))
                                .unwrap_or_default(),
                            action
                        )
                    }
                }
                TurnPhase::AwaitingBlockResponses => {
                    if let Some(ref target_name) = ctx.target_name {
                        format!(
                            "{} wants to {} {} - waiting for blocks",
                            ctx.actor_name, action, target_name
                        )
                    } else {
                        format!(
                            "{} wants to {} - waiting for blocks",
                            ctx.actor_name, action
                        )
                    }
                }
                TurnPhase::AwaitingBlockChallengeResponses => {
                    if let Some(ref block) = ctx.block_info {
                        format!(
                            "{} claims {:?} to block {}",
                            block.blocker_name, block.claimed_role, action
                        )
                    } else {
                        "Waiting for block challenge responses".to_string()
                    }
                }
            }
        } else if !game.players[game.current_player].exchange_cards.is_empty() {
            format!("{} is exchanging cards", current.name)
        } else {
            "Choosing an action".to_string()
        };

        Self {
            title: format!("{}'s turn", current.name),
            subtitle,
            coins: game.coins,
            current_player: current.name.clone(),
            available_actions: game.available_actions(player_uuid),
            players: game
                .players
                .iter()
                .map(|p| {
                    let is_self = p.uuid == *player_uuid;
                    let is_current_turn = p.uuid == current.uuid;
                    PlayerView::from_player(p, is_self, is_current_turn)
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LobbyState {
    pub room_code: String,
    pub max_players: usize,
    pub players: Vec<String>,
    pub creator_uuid: Uuid,
}

impl LobbyState {
    pub fn from_game(game: &Game) -> Self {
        Self {
            room_code: game.room_code.clone(),
            max_players: game.max_players,
            players: game
                .players
                .iter()
                .map(|p| p.name.clone())
                .collect(),
            creator_uuid: game.players[0].uuid,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "connected")]
    Connected { player_uuid: Uuid },

    #[serde(rename = "lobby_state")]
    LobbyState(LobbyState),

    #[serde(rename = "game_state")]
    GameState(GameState),

    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "game_over")]
    GameOver { winner: String },

    #[serde(rename = "lose_influence_choice")]
    LoseInfluenceChoice { cards: Vec<CardView> },

    #[serde(rename = "challenge_prompt")]
    ChallengePrompt {
        actor: String,
        action: String,
        claimed_role: String,
        target: Option<String>,
        deadline_secs: u64,
    },

    #[serde(rename = "block_prompt")]
    BlockPrompt {
        actor: String,
        action: String,
        blockable_by: Vec<String>,
        target: Option<String>,
        deadline_secs: u64,
    },

    #[serde(rename = "block_challenge_prompt")]
    BlockChallengePrompt {
        blocker: String,
        claimed_role: String,
        original_action: String,
        deadline_secs: u64,
    },

    #[serde(rename = "action_result")]
    ActionResult { message: String },
}
