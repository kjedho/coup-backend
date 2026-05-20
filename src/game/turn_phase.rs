use std::collections::HashSet;

use super::card::Role;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum TurnPhase {
    AwaitingChallengeResponses,
    AwaitingBlockResponses,
    AwaitingBlockChallengeResponses,
}

/// What should happen after a player loses influence during a turn phase.
#[derive(Debug, Clone, PartialEq)]
pub enum AfterInfluenceLoss {
    /// The turn ends (e.g. actor lost a challenge while bluffing).
    TurnEnds,
    /// The action was truthful, challenger lost influence; now proceed
    /// to block phase if blockable, or execute the action.
    ProceedAfterFailedChallenge,
    /// Action effect already applied, turn ends after influence loss
    /// (e.g. target loses influence from coup/assassinate after resolution).
    ActionComplete,
    /// Execute the action (e.g. block was challenged and blocker was bluffing).
    ExecuteAction,
    /// Block succeeds (e.g. block-challenge failed, blocker had the card).
    BlockSucceeds,
}

#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub blocker_uuid: Uuid,
    pub blocker_name: String,
    pub claimed_role: Role,
}

#[derive(Debug, Clone)]
pub struct TurnContext {
    pub action: String,
    pub actor_uuid: Uuid,
    pub actor_name: String,
    pub target_uuid: Option<Uuid>,
    pub target_name: Option<String>,
    pub claimed_role: Option<Role>,
    pub phase: TurnPhase,
    pub timer_generation: u64,
    pub responded: HashSet<Uuid>,
    pub block_info: Option<BlockInfo>,
    pub after_influence_loss: Option<AfterInfluenceLoss>,
}

impl TurnContext {
    pub fn new(
        action: String,
        actor_uuid: Uuid,
        actor_name: String,
        target_uuid: Option<Uuid>,
        target_name: Option<String>,
        claimed_role: Option<Role>,
        phase: TurnPhase,
    ) -> Self {
        Self {
            action,
            actor_uuid,
            actor_name,
            target_uuid,
            target_name,
            claimed_role,
            phase,
            timer_generation: 0,
            responded: HashSet::new(),
            block_info: None,
            after_influence_loss: None,
        }
    }

    pub fn next_generation(&mut self) -> u64 {
        self.timer_generation += 1;
        self.timer_generation
    }
}
