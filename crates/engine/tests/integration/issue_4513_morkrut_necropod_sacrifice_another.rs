//! Regression for issue #4513: Morkrut Necropod sacrifice-another filter.
//!
//! Oracle: "Whenever this creature attacks or blocks, sacrifice another
//! creature or land."

use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{Effect, FilterProp, TargetFilter, TypeFilter};
use engine::types::triggers::TriggerMode;

const MORKRUT_NECROPOD_ORACLE: &str =
    "Menace\nWhenever this creature attacks or blocks, sacrifice another creature or land.";

#[test]
fn morkrut_necropod_attacks_or_blocks_sacrifice_another_creature_or_land() {
    let parsed = parse_oracle_text(
        MORKRUT_NECROPOD_ORACLE,
        "Morkrut Necropod",
        &[],
        &["Creature".to_string()],
        &["Slug".to_string(), "Horror".to_string()],
    );
    assert_eq!(parsed.triggers.len(), 1);
    let trigger = &parsed.triggers[0];
    assert_eq!(trigger.mode, TriggerMode::AttacksOrBlocks);

    let execute = trigger.execute.as_ref().expect("trigger execute");
    let Effect::Sacrifice { target, .. } = &*execute.effect else {
        panic!("expected Sacrifice, got {:?}", execute.effect);
    };
    let TargetFilter::Or { filters } = target else {
        panic!("expected Or sacrifice filter, got {target:?}");
    };
    let creature_tf = filters.iter().find_map(|f| {
        let TargetFilter::Typed(tf) = f else {
            return None;
        };
        tf.type_filters
            .contains(&TypeFilter::Creature)
            .then_some(tf)
    });
    let land_tf = filters.iter().find_map(|f| {
        let TargetFilter::Typed(tf) = f else {
            return None;
        };
        tf.type_filters.contains(&TypeFilter::Land).then_some(tf)
    });
    let creature_tf = creature_tf.expect("missing creature leg");
    let land_tf = land_tf.expect("missing land leg");
    assert!(creature_tf.properties.contains(&FilterProp::Another));
    assert!(!land_tf.properties.contains(&FilterProp::Another));
}
