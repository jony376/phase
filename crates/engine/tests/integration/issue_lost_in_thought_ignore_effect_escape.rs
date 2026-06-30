//! Lost in Thought — ignore-effect escape via exile-three-from-graveyard.
//!
//! CR 303.4e + CR 602.2a: the synthesized escape is an activated ability on the
//! Aura, but only the enchanted creature's controller may begin activation and
//! pay the graveyard exile cost ("its controller may exile three cards from
//! their graveyard").

use engine::game::combat::{declare_attackers, AttackTarget};
use engine::game::game_object::AttachTarget;
use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::{Effect, GameRestriction};
use engine::types::actions::GameAction;
use engine::types::game_state::{PayCostKind, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const LOST_IN_THOUGHT: &str = "Enchant creature\n\
Enchanted creature can't attack or block, and its activated abilities can't be activated.\n\
Until your next turn, its controller may exile three cards from their graveyard as though ~ were not on the battlefield.";

fn attach_aura_to_creature(
    runner: &mut GameRunner,
    aura: ObjectId,
    creature: ObjectId,
) {
    let state = runner.state_mut();
    let aura_obj = state.objects.get_mut(&aura).unwrap();
    if !aura_obj.card_types.subtypes.iter().any(|s| s == "Aura") {
        aura_obj.card_types.subtypes.push("Aura".to_string());
        aura_obj.base_card_types = aura_obj.card_types.clone();
    }
    aura_obj.attached_to = Some(AttachTarget::Object(creature));
    state.objects.get_mut(&creature).unwrap().attachments.push(aura);
    evaluate_layers(state);
}

fn ignore_ability_index(runner: &GameRunner, aura: ObjectId) -> usize {
    runner
        .state()
        .objects
        .get(&aura)
        .expect("Lost in Thought present")
        .abilities
        .iter()
        .position(|ability| {
            matches!(
                ability.effect.as_ref(),
                Effect::AddRestriction {
                    restriction: GameRestriction::StaticSourceIgnored { .. },
                }
            )
        })
        .expect("synthesized ignore-effect activated ability")
}

fn pay_exile_three_from_graveyard(
    runner: &mut GameRunner,
    payer: PlayerId,
) -> Result<(), String> {
    let graveyard_cards: Vec<ObjectId> = runner.state().players[payer.0 as usize]
        .graveyard
        .iter()
        .copied()
        .take(3)
        .collect();
    assert_eq!(graveyard_cards.len(), 3, "payer needs three graveyard cards");

    match &runner.state().waiting_for {
        WaitingFor::PayCost {
            kind: PayCostKind::ExileFromZone { .. },
            count,
            ..
        } => {
            assert_eq!(*count, 3);
            runner
                .act(GameAction::SelectCards {
                    cards: graveyard_cards,
                })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        other => Err(format!("expected graveyard exile PayCost, got {other:?}")),
    }
}

fn activate_ignore_escape(runner: &mut GameRunner, aura: ObjectId, payer: PlayerId) {
    runner.state_mut().priority_player = payer;
    let ability_index = ignore_ability_index(runner, aura);
    runner
        .act(GameAction::ActivateAbility {
            source_id: aura,
            ability_index,
        })
        .expect("begin ignore-effect activation");
    pay_exile_three_from_graveyard(runner, payer).expect("exile three graveyard cards");
    runner.advance_until_stack_empty();
}

#[test]
fn lost_in_thought_escape_lets_enchanted_creature_attack_until_next_turn() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_graveyard(P0, &["G1", "G2", "G3", "G4"]);

    let lit = scenario
        .add_creature(P0, "Lost in Thought", 0, 0)
        .as_enchantment()
        .with_subtypes(vec!["Aura"])
        .from_oracle_text(LOST_IN_THOUGHT)
        .id();
    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();

    let mut runner = scenario.build();
    attach_aura_to_creature(&mut runner, lit, bear);

    let mut events = Vec::new();
    assert!(
        declare_attackers(
            runner.state_mut(),
            &[(bear, AttackTarget::Player(P1))],
            &mut events,
        )
        .is_err(),
        "enchanted creature must be locked before paying escape"
    );

    runner.advance_to_phase(Phase::PreCombatMain);
    activate_ignore_escape(&mut runner, lit, P0);

    let mut events = Vec::new();
    declare_attackers(
        runner.state_mut(),
        &[(bear, AttackTarget::Player(P1))],
        &mut events,
    )
    .expect("enchanted creature must attack after ignore-effect escape");

    assert_eq!(
        runner.state().players[P0.0 as usize].graveyard.len(),
        1,
        "exactly three graveyard cards should have been exiled"
    );
}

#[test]
fn lost_in_thought_escape_requires_enchanted_controller_when_aura_controller_differs() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_graveyard(P1, &["G1", "G2", "G3", "G4"]);

    let lit = scenario
        .add_creature(P0, "Lost in Thought", 0, 0)
        .as_enchantment()
        .with_subtypes(vec!["Aura"])
        .from_oracle_text(LOST_IN_THOUGHT)
        .id();
    let bear = scenario.add_creature(P1, "Grizzly Bears", 2, 2).id();

    let mut runner = scenario.build();
    attach_aura_to_creature(&mut runner, lit, bear);

    runner.state_mut().priority_player = P0;
    let ability_index = ignore_ability_index(&runner, lit);
    assert!(
        runner
            .act(GameAction::ActivateAbility {
                source_id: lit,
                ability_index,
            })
            .is_err(),
        "the Aura's controller must not activate the enchanted creature's escape"
    );

    activate_ignore_escape(&mut runner, lit, P1);

    let mut events = Vec::new();
    declare_attackers(
        runner.state_mut(),
        &[(bear, AttackTarget::Player(P0))],
        &mut events,
    )
    .expect("the enchanted creature's controller must attack after paying escape");

    assert_eq!(
        runner.state().players[P1.0 as usize].graveyard.len(),
        1,
        "the enchanted controller's graveyard must supply the exile cost"
    );
}
