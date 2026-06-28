//! Regression for issue #4511: Amplifire upkeep RevealUntil chain with
//! UntilYourNextTurn dynamic base P/T.
//!
//! Oracle: "At the beginning of your upkeep, reveal cards from the top of your
//! library until you reveal a creature card. Until your next turn, this
//! creature's base power becomes twice that card's power and its base toughness
//! becomes twice that card's toughness. Put the revealed cards on the bottom of
//! your library in a random order."

use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    ContinuousModification, Duration, Effect, PlayerScope, QuantityExpr, QuantityRef, TargetFilter,
    TypeFilter,
};
use engine::types::triggers::TriggerMode;
use engine::types::zones::Zone;

const AMPLIFIRE_ORACLE: &str = "At the beginning of your upkeep, reveal cards from the top of \
    your library until you reveal a creature card. Until your next turn, this creature's base \
    power becomes twice that card's power and its base toughness becomes twice that card's \
    toughness. Put the revealed cards on the bottom of your library in a random order.";

#[test]
fn amplifire_upkeep_trigger_reveal_until_dynamic_base_pt() {
    let parsed = parse_oracle_text(
        AMPLIFIRE_ORACLE,
        "Amplifire",
        &[],
        &["Creature".to_string()],
        &["Elemental".to_string()],
    );
    assert_eq!(parsed.triggers.len(), 1, "expected one upkeep trigger");
    let trigger = &parsed.triggers[0];
    assert_eq!(trigger.mode, TriggerMode::Upkeep);

    let execute = trigger
        .execute
        .as_ref()
        .expect("upkeep trigger should have execute");
    let Effect::RevealUntil {
        filter,
        rest_destination,
        ..
    } = &*execute.effect
    else {
        panic!("expected RevealUntil execute, got {:?}", execute.effect);
    };
    let TargetFilter::Typed(tf) = filter else {
        panic!("expected typed creature filter, got {filter:?}");
    };
    assert!(tf.type_filters.contains(&TypeFilter::Creature));
    assert_eq!(*rest_destination, Zone::Library);

    let layer = execute
        .sub_ability
        .as_ref()
        .expect("RevealUntil should chain to layer effect");
    assert_eq!(
        layer.duration,
        Some(Duration::UntilNextTurnOf {
            player: PlayerScope::Controller,
        })
    );
    let Effect::GenericEffect {
        static_abilities, ..
    } = &*layer.effect
    else {
        panic!("expected GenericEffect layer, got {:?}", layer.effect);
    };
    let mods = &static_abilities[0].modifications;
    let twice_power = QuantityExpr::Multiply {
        factor: 2,
        inner: Box::new(QuantityExpr::Ref {
            qty: QuantityRef::Power {
                scope: engine::types::ability::ObjectScope::Demonstrative,
            },
        }),
    };
    assert!(
        mods.iter().any(|m| matches!(
            m,
            ContinuousModification::SetPowerDynamic { value } if *value == twice_power
        )),
        "missing SetPowerDynamic(twice revealed power) in {mods:?}"
    );
    assert!(
        mods.iter()
            .any(|m| matches!(m, ContinuousModification::SetToughnessDynamic { .. })),
        "missing SetToughnessDynamic in {mods:?}"
    );
    assert!(
        !mods
            .iter()
            .any(|m| matches!(m, ContinuousModification::SetPower { .. })),
        "expected dynamic base power, got {mods:?}"
    );
}
