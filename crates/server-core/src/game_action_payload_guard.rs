//! Wire-payload bounds for in-game `GameAction` bodies on the native WebSocket
//! path.
//!
//! The engine validates action *legality*, but a client controls the *size* of
//! the lists and strings inside a `GameAction`, and those reach clone-heavy
//! reducers before legality is fully resolved. This mirrors
//! `draft_action_payload_guard` (which bounds `DraftAction` lists) for the main
//! game action: reject adversarial multi-thousand-entry payloads up front.
//!
//! The cap is deliberately generous — far above any realistic game state,
//! including degenerate token-army boards — so it never rejects legitimate play;
//! it only blocks payloads engineered to force large allocations/clones.
use engine::types::actions::GameAction;

/// Max number of entries accepted in any single client-supplied action list
/// (targets, attackers, blockers, selections, reorder permutations, pile
/// partitions, distributions, ...). Chosen far above any realistic action list
/// while still rejecting adversarial payloads.
pub const MAX_ACTION_LIST_LEN: usize = 10_000;

/// Max length, in bytes, of a free-form choice string on the wire (a chosen
/// option / named card / mode label). Comfortably above the longest real card
/// name.
pub const MAX_CHOICE_LEN: usize = 256;

fn bound_list(field: &str, len: usize) -> Result<(), String> {
    if len > MAX_ACTION_LIST_LEN {
        return Err(format!(
            "{field} has {len} entries; at most {MAX_ACTION_LIST_LEN} allowed"
        ));
    }
    Ok(())
}

/// Validate client-supplied `GameAction` payload sizes before engine dispatch.
/// Variants carrying only bounded scalars (object ids, indices, booleans) need
/// no bound and fall through the wildcard arm.
pub fn guard_game_action_payload(action: &GameAction) -> Result<(), String> {
    match action {
        GameAction::CastSpell { targets, .. }
        | GameAction::CastSpellWithPaymentMode { targets, .. } => {
            bound_list("CastSpell.targets", targets.len())?;
        }
        GameAction::SelectTargets { targets } => {
            bound_list("SelectTargets.targets", targets.len())?;
        }
        GameAction::DeclareAttackers { attacks } => {
            bound_list("DeclareAttackers.attacks", attacks.len())?;
        }
        GameAction::DeclareBlockers { assignments } => {
            bound_list("DeclareBlockers.assignments", assignments.len())?;
        }
        GameAction::AssignCombatDamage { assignments, .. } => {
            bound_list("AssignCombatDamage.assignments", assignments.len())?;
        }
        GameAction::ReorderHand { order } => {
            bound_list("ReorderHand.order", order.len())?;
        }
        GameAction::OrderTriggers { order } => {
            bound_list("OrderTriggers.order", order.len())?;
        }
        GameAction::SelectCards { cards } => {
            bound_list("SelectCards.cards", cards.len())?;
        }
        GameAction::SelectCoinFlips { keep_indices } => {
            bound_list("SelectCoinFlips.keep_indices", keep_indices.len())?;
        }
        GameAction::SelectModes { indices } => {
            bound_list("SelectModes.indices", indices.len())?;
        }
        GameAction::ChooseOutsideGameCards { selections } => {
            bound_list("ChooseOutsideGameCards.selections", selections.len())?;
        }
        GameAction::ChooseCounterMoveDistribution { selections } => {
            bound_list("ChooseCounterMoveDistribution.selections", selections.len())?;
        }
        GameAction::CrewVehicle { creature_ids, .. } => {
            bound_list("CrewVehicle.creature_ids", creature_ids.len())?;
        }
        GameAction::SaddleMount { creature_ids, .. } => {
            bound_list("SaddleMount.creature_ids", creature_ids.len())?;
        }
        GameAction::SubmitSideboard { main, sideboard } => {
            bound_list("SubmitSideboard.main", main.len())?;
            bound_list("SubmitSideboard.sideboard", sideboard.len())?;
        }
        GameAction::SubmitPilePartition { pile_a, .. } => {
            bound_list("SubmitPilePartition.pile_a", pile_a.len())?;
        }
        GameAction::SetPhaseStops { stops } => {
            bound_list("SetPhaseStops.stops", stops.len())?;
        }
        GameAction::DistributeAmong { distribution, .. } => {
            bound_list("DistributeAmong.distribution", distribution.len())?;
        }
        GameAction::RetargetSpell { new_targets, .. } => {
            bound_list("RetargetSpell.new_targets", new_targets.len())?;
        }
        GameAction::ChooseOption { choice, .. } if choice.len() > MAX_CHOICE_LEN => {
            return Err(format!(
                "ChooseOption.choice is {} bytes; at most {MAX_CHOICE_LEN} allowed",
                choice.len()
            ));
        }
        // All other variants carry only bounded scalars (ids/indices/bools).
        _ => {}
    }
    Ok(())
}
