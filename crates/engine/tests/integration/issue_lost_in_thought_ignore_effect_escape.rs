//! Lost in Thought — ignore-effect escape via exile-three-from-graveyard.

use engine::game::combat::{declare_attackers, AttackTarget};
use engine::game::game_object::AttachTarget;
use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameScenario, P0, P1};

use engine::types::ability::{Effect, GameRestriction};
use engine::types::actions::GameAction;
use engine::types::game_state::{PayCostKind, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;

const LOST_IN_THOUGHT: &str = "Enchant creature\n\
Enchanted creature can't attack or block, and its activated abilities can't be activated.\n\
Until your next turn, its controller may exile three cards from their graveyard as though ~ were not on the battlefield.";

fn ignore_ability_index(runner: &engine::game::scenario::GameRunner, aura: ObjectId) -> usize {
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
    {
        let state = runner.state_mut();
        let lit_obj = state.objects.get_mut(&lit).unwrap();
        if !lit_obj.card_types.subtypes.iter().any(|s| s == "Aura") {
            lit_obj.card_types.subtypes.push("Aura".to_string());
            lit_obj.base_card_types = lit_obj.card_types.clone();
        }
        lit_obj.attached_to = Some(AttachTarget::Object(bear));
        state.objects.get_mut(&bear).unwrap().attachments.push(lit);
    }
    evaluate_layers(runner.state_mut());

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

    let ability_index = ignore_ability_index(&runner, lit);
    runner
        .act(GameAction::ActivateAbility {
            source_id: lit,
            ability_index,
        })
        .expect("begin ignore-effect activation");

    let graveyard_cards: Vec<ObjectId> = runner.state().players[P0.0 as usize]
        .graveyard
        .iter()
        .copied()
        .take(3)
        .collect();
    assert_eq!(graveyard_cards.len(), 3);

    match &runner.state().waiting_for {
        WaitingFor::PayCost {
            kind: PayCostKind::ExileFromZone { .. },
            count,
            ..
        } => {
            assert_eq!(*count, 3);
            runner
                .act(GameAction::SelectCards {
                    cards: graveyard_cards.clone(),
                })
                .expect("exile three graveyard cards");
        }
        other => panic!("expected graveyard exile PayCost, got {other:?}"),
    }
    runner.advance_until_stack_empty();

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
