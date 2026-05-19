//! Integration tests for Braids, Arisen Nightmare's end-step decline tail
//! (GitHub issue #491 — "directs life loss to the wrong player").
//!
//! Oracle text:
//!   At the beginning of your end step, you may sacrifice an artifact,
//!   creature, enchantment, land, or planeswalker. If you do, each opponent
//!   may sacrifice a permanent of their choice that shares a card type with
//!   it. For each opponent who doesn't, that player loses 2 life and you
//!   draw a card.
//!
//! Rules-correct behavior (CR 608.2c / CR 109.5):
//!   - "that player loses 2 life" → the *opponent who declined* loses 2 life.
//!   - "you draw a card"          → Braids' controller draws — once per
//!     declining opponent.
//!
//! These tests drive the real resolution pipeline: the trigger's `execute`
//! chain is parsed from the real Oracle text, built into a `ResolvedAbility`,
//! and resolved via `resolve_ability_chain`; the per-opponent optional
//! sacrifice decisions are submitted as `GameAction`s through `apply`.

use engine::game::ability_utils::build_resolved_from_def;
use engine::game::effects::resolve_ability_chain;
use engine::game::engine::apply;
use engine::game::zones::create_object;
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::ResolvedAbility;
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::format::FormatConfig;
use engine::types::game_state::{GameState, WaitingFor};
use engine::types::identifiers::{CardId, ObjectId};
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const BRAIDS_ORACLE: &str = "At the beginning of your end step, you may \
sacrifice an artifact, creature, enchantment, land, or planeswalker. If you \
do, each opponent may sacrifice a permanent of their choice that shares a \
card type with it. For each opponent who doesn't, that player loses 2 life \
and you draw a card.";

/// Build the Braids end-step trigger's `execute` chain as a `ResolvedAbility`
/// controlled by `controller`, with `source_id` as the source permanent.
fn braids_execute(controller: PlayerId, source_id: ObjectId) -> ResolvedAbility {
    let parsed = parse_oracle_text(
        BRAIDS_ORACLE,
        "Braids, Arisen Nightmare",
        &[],
        &["Legendary".to_string(), "Creature".to_string()],
        &["Nightmare".to_string()],
    );
    let trigger = parsed
        .triggers
        .first()
        .expect("Braids has an end-step trigger");
    let execute = trigger
        .execute
        .as_deref()
        .expect("Braids' trigger has an execute chain");
    build_resolved_from_def(execute, source_id, controller)
}

/// Create a battlefield permanent of the given core type owned by `player`.
fn add_permanent(
    state: &mut GameState,
    card_id: u64,
    player: PlayerId,
    name: &str,
    core_type: CoreType,
) -> ObjectId {
    let id = create_object(
        state,
        CardId(card_id),
        player,
        name.to_string(),
        Zone::Battlefield,
    );
    let obj = state.objects.get_mut(&id).unwrap();
    obj.card_types.core_types = vec![core_type];
    obj.base_card_types = obj.card_types.clone();
    id
}

/// Seed `player`'s library with one stand-in card so a `Draw` has something
/// to draw. Returns the seeded card's `ObjectId`.
fn seed_library(state: &mut GameState, card_id: u64, player: PlayerId) -> ObjectId {
    let card = create_object(
        state,
        CardId(card_id),
        player,
        "Forest".to_string(),
        Zone::Library,
    );
    state
        .players
        .iter_mut()
        .find(|p| p.id == player)
        .expect("player exists")
        .library
        .push_back(card);
    card
}

fn life(state: &GameState, player: PlayerId) -> i32 {
    state
        .players
        .iter()
        .find(|p| p.id == player)
        .expect("player exists")
        .life
}

fn hand_len(state: &GameState, player: PlayerId) -> usize {
    state
        .players
        .iter()
        .find(|p| p.id == player)
        .expect("player exists")
        .hand
        .len()
}

/// Decide the next pending `OptionalEffectChoice` for the player it lands on.
fn decide_optional(state: &mut GameState, accept: bool) {
    let player = match &state.waiting_for {
        WaitingFor::OptionalEffectChoice { player, .. } => *player,
        other => panic!("expected OptionalEffectChoice, got {other:?}"),
    };
    apply(state, player, GameAction::DecideOptionalEffect { accept })
        .expect("optional-effect decision should succeed");
}

/// Test A — single declining opponent. P0 controls Braids + a creature to
/// sacrifice; P1 controls only a Land (no shared card type → forced decline).
///
/// Discriminates Step 1a (Edge 1 stays scoped), Step 3 (`Not{IfYouDo}` gate),
/// and Step 4 (`None`→`ScopedPlayer` recipient). Does NOT discriminate Step 1b.
#[test]
fn braids_single_opponent_declines_loses_two_and_controller_draws() {
    let mut state = GameState::new(FormatConfig::standard(), 2, 42);
    let braids = add_permanent(&mut state, 10, PlayerId(0), "Braids", CoreType::Creature);
    let p0_creature = add_permanent(
        &mut state,
        11,
        PlayerId(0),
        "Grizzly Bears",
        CoreType::Creature,
    );
    // P1 controls only a Land — nothing shares a card type with a Creature.
    add_permanent(&mut state, 20, PlayerId(1), "Forest", CoreType::Land);
    seed_library(&mut state, 30, PlayerId(0));

    let p0_life_before = life(&state, PlayerId(0));
    let p1_life_before = life(&state, PlayerId(1));
    let p0_hand_before = hand_len(&state, PlayerId(0));

    let ability = braids_execute(PlayerId(0), braids);
    let mut events = Vec::new();
    resolve_ability_chain(&mut state, &ability, &mut events, 0).unwrap();

    // P0 accepts the optional sacrifice and sacrifices the creature.
    decide_optional(&mut state, true);
    apply(
        &mut state,
        PlayerId(0),
        GameAction::SelectCards {
            cards: vec![p0_creature],
        },
    )
    .unwrap();

    // P1 cannot sacrifice a matching permanent — decline.
    if matches!(state.waiting_for, WaitingFor::OptionalEffectChoice { .. }) {
        decide_optional(&mut state, false);
    }

    assert_eq!(
        life(&state, PlayerId(1)),
        p1_life_before - 2,
        "the declining opponent (P1) must lose 2 life"
    );
    assert_eq!(
        life(&state, PlayerId(0)),
        p0_life_before,
        "Braids' controller (P0) must NOT lose life"
    );
    assert_eq!(
        hand_len(&state, PlayerId(0)),
        p0_hand_before + 1,
        "Braids' controller draws exactly one card for the single declining opponent"
    );
}

/// Test B — multi-opponent fan-out (MANDATORY: the Edge-2 / Step-1b
/// discriminator). 3-player game; P1 and P2 are both forced to decline.
///
/// With Step 1b reverted (no `LoseLife` arm in
/// `effect_has_iteration_bound_recipient`), the `LoseLife→Draw` edge detaches:
/// the draw runs once after the `player_scope` loop instead of once per
/// declining opponent → P0's hand grows by 1, not 2. Test A cannot catch this.
#[test]
fn braids_two_opponents_decline_controller_draws_two() {
    let mut state = GameState::new(FormatConfig::standard(), 3, 42);
    let braids = add_permanent(&mut state, 10, PlayerId(0), "Braids", CoreType::Creature);
    let p0_creature = add_permanent(
        &mut state,
        11,
        PlayerId(0),
        "Grizzly Bears",
        CoreType::Creature,
    );
    // Both opponents control only a Land — forced decline.
    add_permanent(&mut state, 20, PlayerId(1), "Forest", CoreType::Land);
    add_permanent(&mut state, 21, PlayerId(2), "Island", CoreType::Land);
    // P0 draws once per declining opponent — seed two cards.
    seed_library(&mut state, 30, PlayerId(0));
    seed_library(&mut state, 31, PlayerId(0));

    let p0_life_before = life(&state, PlayerId(0));
    let p1_life_before = life(&state, PlayerId(1));
    let p2_life_before = life(&state, PlayerId(2));
    let p0_hand_before = hand_len(&state, PlayerId(0));

    let ability = braids_execute(PlayerId(0), braids);
    let mut events = Vec::new();
    resolve_ability_chain(&mut state, &ability, &mut events, 0).unwrap();

    decide_optional(&mut state, true);
    apply(
        &mut state,
        PlayerId(0),
        GameAction::SelectCards {
            cards: vec![p0_creature],
        },
    )
    .unwrap();

    // Both opponents decline in turn.
    for _ in 0..2 {
        if matches!(state.waiting_for, WaitingFor::OptionalEffectChoice { .. }) {
            decide_optional(&mut state, false);
        }
    }

    assert_eq!(
        life(&state, PlayerId(1)),
        p1_life_before - 2,
        "declining opponent P1 loses 2 life"
    );
    assert_eq!(
        life(&state, PlayerId(2)),
        p2_life_before - 2,
        "declining opponent P2 loses 2 life"
    );
    assert_eq!(
        life(&state, PlayerId(0)),
        p0_life_before,
        "Braids' controller P0 must NOT lose life"
    );
    assert_eq!(
        hand_len(&state, PlayerId(0)),
        p0_hand_before + 2,
        "Braids' controller draws ONCE PER declining opponent — exactly 2 cards \
         (Step 1b: the LoseLife→Draw edge stays inside the per-opponent scope)"
    );
}
