//! Issue #3249 — Faith's Fetters must prevent the enchanted permanent from attacking.
//!
//! Root cause: the compound static splitter for "can't attack or block, and …
//! activated abilities can't be activated" bound both prohibitions to
//! `TargetFilter::SelfRef` (the Aura) instead of the enchanted host filter.

use engine::game::combat::AttackTarget;
use engine::game::effects::attach::attach_to;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::phase::Phase;

const FAITHS_FETTERS: &str = "Enchant permanent\n\
When this Aura enters, you gain 4 life.\n\
Enchanted permanent can't attack or block, and its activated abilities can't be activated unless they're mana abilities.";

#[test]
fn faiths_fetters_prevents_enchanted_creature_from_attacking() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let fetters = scenario
        .add_creature(P0, "Faith's Fetters", 0, 0)
        .as_enchantment()
        .from_oracle_text(FAITHS_FETTERS)
        .id();
    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();

    let mut runner = scenario.build();
    {
        let fetters_obj = runner
            .state_mut()
            .objects
            .get_mut(&fetters)
            .expect("Faith's Fetters present");
        if !fetters_obj.card_types.subtypes.iter().any(|s| s == "Aura") {
            fetters_obj.card_types.subtypes.push("Aura".to_string());
            fetters_obj.base_card_types = fetters_obj.card_types.clone();
        }
    }
    assert!(
        attach_to(runner.state_mut(), fetters, bear).is_some(),
        "Faith's Fetters must attach to the bear"
    );

    runner.advance_to_combat();
    let err = runner
        .declare_attackers(&[(bear, AttackTarget::Player(P1))])
        .expect_err("enchanted creature must be unable to attack under Faith's Fetters");
    assert!(
        err.to_string().contains("can't attack"),
        "expected attack prohibition error, got {err}"
    );
}
