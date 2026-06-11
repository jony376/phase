//! Issue #3004 / #2936 — Trystan's Command mode 2 ("return one or two target
//! permanent cards from your graveyard to your hand") must carry
//! `MultiTargetSpec { min: 1, max: 2 }` through the card export and surface
//! a two-target selection at cast resolution.
//!
//! CR 115.1a + CR 601.2c: bounded multi-target spells announce 1–2 targets.
//! CR 700.2f: modal modes carry independent targeting requirements.

use std::path::PathBuf;

use engine::database::card_db::CardDatabase;
use engine::game::rehydrate_game_from_card_db;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::game::scenario_db::GameScenarioDbExt;
use engine::types::ability::{Effect, MultiTargetSpec};
use engine::types::actions::GameAction;
use engine::types::game_state::CastPaymentMode;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

fn export_db() -> Option<CardDatabase> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../client/public/card-data.json");
    if !path.exists() {
        eprintln!("skipping: client/public/card-data.json not generated");
        return None;
    }
    Some(CardDatabase::from_export(&path).expect("export should load"))
}

fn add_mana_for_trystan(runner: &mut engine::game::scenario::GameRunner) {
    let dummy = ObjectId(0);
    let pool = &mut runner
        .state_mut()
        .players
        .iter_mut()
        .find(|p| p.id == P0)
        .unwrap()
        .mana_pool;
    for _ in 0..4 {
        pool.add(ManaUnit::new(ManaType::Colorless, dummy, false, vec![]));
    }
    pool.add(ManaUnit::new(ManaType::Black, dummy, false, vec![]));
    pool.add(ManaUnit::new(ManaType::Green, dummy, false, vec![]));
}

/// The export must carry bounded multi-target metadata on the return mode so
/// production loads (CardDatabase::from_export) match the live parser.
#[test]
fn trystan_export_return_mode_has_bounded_multi_target() {
    let Some(db) = export_db() else {
        return;
    };
    let face = db
        .get_face_by_name("Trystan's Command")
        .expect("Trystan's Command must be in card-data export");
    let return_mode = face
        .abilities
        .get(1)
        .expect("return mode is ability index 1");
    assert!(
        matches!(*return_mode.effect, Effect::Bounce { .. }),
        "mode 2 must lower to Bounce, got {:?}",
        return_mode.effect
    );
    assert_eq!(
        return_mode.multi_target,
        Some(MultiTargetSpec::fixed(1, 2)),
        "export must carry MultiTargetSpec::fixed(1, 2) on the return mode"
    );
}

/// Drive a real cast of mode 2 and assert the engine offers up to two
/// graveyard permanent targets (not a single-target prompt).
#[test]
fn trystan_return_mode_surfaces_two_target_selection() {
    let Some(db) = export_db() else {
        return;
    };

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_to_graveyard(P0, "Gy Bear 1", 2, 2);
    scenario.add_creature_to_graveyard(P0, "Gy Bear 2", 2, 2);
    // Destroy target for mode 3 when paired with return in a choose-two cast.
    scenario.add_creature(P1, "Opp Bear", 2, 2);

    let trystan = scenario.add_real_card(P0, "Trystan's Command", Zone::Hand, &db);

    let mut runner = scenario.build();
    rehydrate_game_from_card_db(runner.state_mut(), &db);
    add_mana_for_trystan(&mut runner);

    let card_id = runner.state().objects[&trystan].card_id;
    runner
        .act(GameAction::CastSpell {
            object_id: trystan,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("casting Trystan's Command must succeed");

    let mut return_slot_optionals = None;
    for _ in 0..60 {
        match runner.state().waiting_for.clone() {
            WaitingFor::Priority { .. } => {
                if runner.act(GameAction::PassPriority).is_err() {
                    break;
                }
            }
            WaitingFor::ModeChoice { .. } => {
                // CR 700.2: choose two modes — return (1) + destroy (2).
                runner
                    .act(GameAction::SelectModes {
                        indices: vec![1, 2],
                    })
                    .expect("modal mode choice must succeed");
            }
            WaitingFor::TargetSelection {
                pending_cast,
                target_slots,
                ..
            } => {
                assert_eq!(
                    pending_cast.ability.multi_target,
                    Some(MultiTargetSpec::fixed(1, 2)),
                    "return mode must carry bounded multi-target metadata at cast"
                );
                // Modal casts fan out one slot per announced target; return mode
                // needs two slots (one required, one optional) before destroy.
                return_slot_optionals = Some(
                    target_slots
                        .iter()
                        .filter(|slot| slot.legal_targets.len() == 2)
                        .map(|slot| slot.optional)
                        .collect::<Vec<_>>(),
                );
                break;
            }
            _ => {
                if runner.act(GameAction::PassPriority).is_ok() {
                    continue;
                }
                break;
            }
        }
    }

    let return_optionals = return_slot_optionals.unwrap_or_else(|| {
        panic!(
            "return mode must surface TargetSelection with two graveyard slots; \
             last waiting_for: {:?}",
            runner.state().waiting_for
        )
    });
    assert_eq!(
        return_optionals,
        vec![false, true],
        "return mode must offer two target slots (min 1, max 2)"
    );
}
