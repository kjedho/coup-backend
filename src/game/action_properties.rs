use super::card::Role;

pub struct ActionProperties {
    pub claimed_role: Option<Role>,
    pub blockable_by: Vec<Role>,
    pub block_target_only: bool,
    pub cost: u8,
    pub requires_target: bool,
}

pub fn display_action_name(action: &str) -> String {
    match action {
        "foreign_aid" => "foreign aid".to_string(),
        "exchange_draw" => "exchange".to_string(),
        _ => action.to_string(),
    }
}

pub fn get_action_properties(action: &str) -> Option<ActionProperties> {
    match action {
        "income" => Some(ActionProperties {
            claimed_role: None,
            blockable_by: vec![],
            block_target_only: false,
            cost: 0,
            requires_target: false,
        }),
        "foreign_aid" => Some(ActionProperties {
            claimed_role: None,
            blockable_by: vec![Role::Duke],
            block_target_only: false,
            cost: 0,
            requires_target: false,
        }),
        "coup" => Some(ActionProperties {
            claimed_role: None,
            blockable_by: vec![],
            block_target_only: false,
            cost: 7,
            requires_target: true,
        }),
        "tax" => Some(ActionProperties {
            claimed_role: Some(Role::Duke),
            blockable_by: vec![],
            block_target_only: false,
            cost: 0,
            requires_target: false,
        }),
        "assassinate" => Some(ActionProperties {
            claimed_role: Some(Role::Assassin),
            blockable_by: vec![Role::Contessa],
            block_target_only: true,
            cost: 3,
            requires_target: true,
        }),
        "steal" => Some(ActionProperties {
            claimed_role: Some(Role::Captain),
            blockable_by: vec![Role::Captain, Role::Ambassador],
            block_target_only: true,
            cost: 0,
            requires_target: true,
        }),
        "exchange_draw" => Some(ActionProperties {
            claimed_role: Some(Role::Ambassador),
            blockable_by: vec![],
            block_target_only: false,
            cost: 0,
            requires_target: false,
        }),
        _ => None,
    }
}
