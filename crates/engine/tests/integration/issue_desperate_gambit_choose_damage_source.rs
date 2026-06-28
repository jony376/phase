//! Desperate Gambit — "Choose a source you control" must parse as
//! `ChooseDamageSource` with a You-controller candidate filter and thread that
//! filter into the coin-flip one-shot damage-replacement shields.

use engine::game::effects::deal_damage;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    AbilityKind, ControllerRef, DamageModification, Effect, PreventionAmount, ShieldKind,
    TargetFilter, TypedFilter,
};
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;

const DESPERATE_GAMBIT: &str = "Choose a source you control. Flip a coin. If you win the flip, \
    the next time that source would deal damage this turn, it deals double that damage instead. \
    If you lose the flip, the next time that source would deal damage this turn, prevent that damage.";

fn typed_you_control(filter: &TargetFilter) -> Option<&TypedFilter> {
    match filter {
        TargetFilter::Typed(tf) if tf.controller == Some(ControllerRef::You) => Some(tf),
        _ => None,
    }
}

fn add_mana(runner: &mut engine::game::scenario::GameRunner, mana: &[ManaType]) {
    let dummy = ObjectId(0);
    let pool = &mut runner
        .state_mut()
        .players
        .iter_mut()
        .find(|p| p.id == P0)
        .unwrap()
        .mana_pool;
    for m in mana {
        pool.add(ManaUnit::new(*m, dummy, false, vec![]));
    }
}

fn resolve_desperate_gambit_through_source_choice(
    runner: &mut engine::game::scenario::GameRunner,
    gambit: ObjectId,
    chosen_source: ObjectId,
) {
    runner
        .act(GameAction::CastSpell {
            object_id: gambit,
            card_id: runner.state().objects[&gambit].card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("cast Desperate Gambit");

    for _ in 0..32 {
        match &runner.state().waiting_for {
            WaitingFor::DamageSourceChoice { options, .. } => {
                assert!(
                    options.contains(&chosen_source),
                    "chosen source must be offered, got options {options:?}"
                );
                runner
                    .act(GameAction::ChooseDamageSource {
                        source: chosen_source,
                    })
                    .expect("choose damage source");
            }
            WaitingFor::ManaPayment { .. } => {
                runner.act(GameAction::PassPriority).expect("pay mana");
            }
            WaitingFor::Priority { .. } => break,
            other => panic!("unexpected pre-source-choice prompt: {other:?}"),
        }
    }
}

#[test]
fn desperate_gambit_oracle_parses_choose_damage_source_you_control() {
    let parsed = parse_oracle_text(
        DESPERATE_GAMBIT,
        "Desperate Gambit",
        &[],
        &[],
        &["Instant".to_string()],
    );
    let spell = parsed
        .abilities
        .iter()
        .find(|a| matches!(a.kind, AbilityKind::Spell))
        .expect("Desperate Gambit must have a spell ability");
    match spell.effect.as_ref() {
        Effect::ChooseDamageSource { source_filter } => {
            assert!(
                typed_you_control(source_filter).is_some(),
                "expected You-controller source filter, got {source_filter:?}"
            );
        }
        other => panic!("expected ChooseDamageSource head, got {other:?}"),
    }
}

#[test]
fn desperate_gambit_damage_source_choice_excludes_opponent_sources() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let gambit = scenario
        .add_spell_to_hand_from_oracle(P0, "Desperate Gambit", true, DESPERATE_GAMBIT)
        .with_mana_cost(ManaCost::Red)
        .id();
    let p0_source = scenario.add_creature(P0, "Your Shaman", 2, 2).id();
    let p1_source = scenario.add_creature(P1, "Their Bear", 2, 2).id();

    let mut runner = scenario.build();
    add_mana(&mut runner, &[ManaType::Red]);

    runner
        .act(GameAction::CastSpell {
            object_id: gambit,
            card_id: runner.state().objects[&gambit].card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("cast Desperate Gambit");

    for _ in 0..32 {
        match &runner.state().waiting_for {
            WaitingFor::DamageSourceChoice { options, .. } => {
                assert!(options.contains(&p0_source), "P0's source must be offered");
                assert!(
                    !options.contains(&p1_source),
                    "P1's source must not be offered under 'you control'"
                );
                runner
                    .act(GameAction::ChooseDamageSource { source: p0_source })
                    .expect("choose P0 source");
            }
            WaitingFor::ManaPayment { .. } => {
                runner.act(GameAction::PassPriority).expect("pay mana");
            }
            WaitingFor::Priority { .. } => break,
            other => panic!("unexpected prompt: {other:?}"),
        }
    }
}

#[test]
fn desperate_gambit_one_shot_shield_applies_to_chosen_source_only() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let gambit = scenario
        .add_spell_to_hand_from_oracle(P0, "Desperate Gambit", true, DESPERATE_GAMBIT)
        .with_mana_cost(ManaCost::Red)
        .id();
    let source = scenario.add_creature(P0, "Damage Source", 2, 2).id();

    let mut runner = scenario.build();
    add_mana(&mut runner, &[ManaType::Red]);
    resolve_desperate_gambit_through_source_choice(&mut runner, gambit, source);
    runner.advance_until_stack_empty();

    let host = runner.state().objects.get(&source).expect("source present");
    let double_shield = host.replacement_definitions.iter().find(|s| {
        matches!(s.shield_kind, ShieldKind::DamageReplacementOneShot)
            && s.damage_modification == Some(DamageModification::Double)
    });
    let prevent_shield = host.replacement_definitions.iter().find(|s| {
        matches!(
            s.shield_kind,
            ShieldKind::Prevention {
                amount: PreventionAmount::All
            }
        )
    });
    assert!(
        double_shield.is_some() || prevent_shield.is_some(),
        "coin flip must install a one-shot shield on the chosen source, got {:?}",
        host.replacement_definitions
    );

    let ctx = deal_damage::DamageContext::from_source(runner.state(), source).unwrap();
    let mut events = Vec::new();
    let result = deal_damage::apply_damage_to_target(
        runner.state_mut(),
        &ctx,
        engine::types::ability::TargetRef::Player(P1),
        3,
        false,
        &mut events,
    )
    .unwrap();

    if double_shield.is_some() {
        assert!(
            matches!(result, deal_damage::DamageResult::Applied(6)),
            "win branch must double 3 → 6, got {result:?}"
        );
        assert_eq!(runner.state().players[P1.0 as usize].life, 14);
    } else {
        assert!(
            matches!(result, deal_damage::DamageResult::Applied(0)),
            "lose branch must prevent damage, got {result:?}"
        );
        assert_eq!(runner.state().players[P1.0 as usize].life, 20);
    }
}
