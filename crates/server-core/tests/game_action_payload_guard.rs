//! Wire-payload bounds for in-game `GameAction` bodies (see
//! `server_core::game_action_payload_guard`).

use engine::types::{GameAction, ObjectId};
use server_core::game_action_payload_guard::{guard_game_action_payload, MAX_ACTION_LIST_LEN};

#[test]
fn rejects_oversized_action_list() {
    let action = GameAction::ReorderHand {
        order: vec![ObjectId(1); MAX_ACTION_LIST_LEN + 1],
    };
    assert!(
        guard_game_action_payload(&action).is_err(),
        "a list exceeding MAX_ACTION_LIST_LEN must be rejected"
    );
}

#[test]
fn accepts_reasonably_sized_action_list() {
    let action = GameAction::ReorderHand {
        order: vec![ObjectId(1); 20],
    };
    assert!(
        guard_game_action_payload(&action).is_ok(),
        "a realistic action list must be accepted"
    );
}

#[test]
fn passes_scalar_only_action() {
    // Variants with no client-supplied list/string fall through unguarded.
    assert!(guard_game_action_payload(&GameAction::PassPriority).is_ok());
}
