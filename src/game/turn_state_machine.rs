use rust_fsm::*;

#[derive(Debug)]
enum TurnInput {
    BlockableAction,
    UnblockableAction,
    BluffCall,
    BluffCallCorrect,
    BluffCallIncorrect,
    BlockPossible,
    BlockImpossible,
    BlockAction,
    AllowAction,
    NoBluffCallAndBlockPossible,
    NoBluffCallAndBlockImpossible,
    None,
    NextTurn
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TurnState {
    SelectAction,
    StartTimer,
    Resolve,
    LoseInfluenceBluffCaller,
    LoseInfluenceBluffReceiver,
    GainGold,
    ExecuteAction,
    SelectCounteraction,
    EndTurn
}

#[derive(Debug, PartialEq)]
struct TurnOutput;

#[derive(Debug)]
struct TurnStateMachine;

impl StateMachineImpl for TurnStateMachine {
    type Input = TurnInput;
    type State = TurnState;
    type Output = TurnOutput;
    const INITIAL_STATE: Self::State = TurnState::SelectAction;

    fn transition(state: &Self::State, input: &Self::Input) -> Option<Self::State> {
        match (state, input) {
            // Select action
            (TurnState::SelectAction, TurnInput::BlockableAction) => {
                Some(TurnState::StartTimer)
            }
            (TurnState::SelectAction, TurnInput::UnblockableAction) => {
                Some(TurnState::ExecuteAction)
            }
            // Start timer
            (TurnState::StartTimer, TurnInput::BluffCall) => {
                Some(TurnState::Resolve)
            }
            (TurnState::StartTimer, TurnInput::NoBluffCallAndBlockPossible) => {
                Some(TurnState::SelectCounteraction)
            }
            (TurnState::StartTimer, TurnInput::NoBluffCallAndBlockImpossible) => {
                Some(TurnState::ExecuteAction)
            }
            // Resolve
            (TurnState::Resolve, TurnInput::BluffCallCorrect) => {
                Some(TurnState::LoseInfluenceBluffReceiver)
            }
            (TurnState::Resolve, TurnInput::BluffCallIncorrect) => {
                Some(TurnState::LoseInfluenceBluffCaller)
            }
            // Lose influence (bluff receiver)
            (TurnState::LoseInfluenceBluffReceiver, TurnInput::None) => {
                Some(TurnState::GainGold)
            }
            // Gain gold
            (TurnState::GainGold, TurnInput::None) => {
                Some(TurnState::EndTurn)
            }
            // Lose influence (bluff caller)
            (TurnState::LoseInfluenceBluffCaller, TurnInput::BlockPossible) => {
                Some(TurnState::SelectCounteraction)
            }
            (TurnState::LoseInfluenceBluffCaller, TurnInput::BlockImpossible) => {
                Some(TurnState::ExecuteAction)
            }
            // Select counteraction
            (TurnState::SelectCounteraction, TurnInput::BlockAction) => {
                Some(TurnState::StartTimer)
            }
            (TurnState::SelectCounteraction, TurnInput::AllowAction) => {
                Some(TurnState::ExecuteAction)
            }
            // Execute action
            (TurnState::ExecuteAction, TurnInput::None) => {
                Some(TurnState::EndTurn)
            }
            // End turn
            (TurnState::EndTurn, TurnInput::NextTurn) => {
                Some(TurnState::SelectAction)
            }
            _ => None,
        }
    }

    fn output(_state: &Self::State, _input: &Self::Input) -> Option<Self::Output> {
        None
    }
}

#[test]
fn coup_turn() {
    use std::sync::{Arc, Mutex};

    let machine: StateMachine<TurnStateMachine> = StateMachine::new();

    // State: Select action (init)
    let machine = Arc::new(Mutex::new(machine));
    {
        let mut lock = machine.lock().unwrap();
        // Input: Blockable action
        let _ = lock.consume(&TurnInput::BlockableAction).unwrap();
        // State: Start timer
        assert_eq!(lock.state(), &TurnState::StartTimer);

        // Input: Bluff call
        let _ = lock.consume(&TurnInput::BluffCall).unwrap();
        // State: Resolve
        assert_eq!(lock.state(), &TurnState::Resolve);

        // Input: Block action (invalid)
        let res = lock.consume(&TurnInput::BlockAction);
        assert!(matches!(res, Err(TransitionImpossibleError)));
        // State: Resolve
        assert_eq!(lock.state(), &TurnState::Resolve);

        // Input: Bluff call incorrect
        let _ = lock.consume(&TurnInput::BluffCallIncorrect).unwrap();
        // State: Lose influence (bluff caller)
        assert_eq!(lock.state(), &TurnState::LoseInfluenceBluffCaller);

        // Input: Block possible
        let _ = lock.consume(&TurnInput::BlockPossible).unwrap();
        // State: Select counteraction
        assert_eq!(lock.state(), &TurnState::SelectCounteraction);

        // Input: Allow action
        let _ = lock.consume(&TurnInput::AllowAction).unwrap();
        // State: Execute action
        assert_eq!(lock.state(), &TurnState::ExecuteAction);

        // Input: None
        let _ = lock.consume(&TurnInput::None).unwrap();
        // State: End turn
        assert_eq!(lock.state(), &TurnState::EndTurn);
    }

}