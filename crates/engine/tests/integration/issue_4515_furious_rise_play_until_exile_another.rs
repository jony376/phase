//! Regression for issue #4515: Furious Rise play-from-exile duration (cluster 33).
//!
//! Oracle: "…exile the top card of your library. You may play that card until
//! you exile another card with this enchantment."

use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    CastingPermission, Duration, Effect, PlayerScope, TargetFilter, TrackedSetId,
};

const FURIOUS_RISE_ORACLE: &str = "At the beginning of your end step, if you control a creature with power 4 or greater, exile the top card of your library. You may play that card until you exile another card with this enchantment.";

#[test]
fn furious_rise_end_step_exile_play_until_exile_another_with_source() {
    let parsed = parse_oracle_text(
        FURIOUS_RISE_ORACLE,
        "Furious Rise",
        &[],
        &["Enchantment".to_string()],
        &[],
    );
    assert_eq!(parsed.triggers.len(), 1);
    let trigger = &parsed.triggers[0];
    let execute = trigger.execute.as_ref().expect("trigger execute");
    let Effect::ExileTop { count: 1, .. } = &*execute.effect else {
        panic!("expected ExileTop, got {:?}", execute.effect);
    };
    let play_grant = execute
        .sub_ability
        .as_ref()
        .expect("play grant chained after exile");
    let Effect::GrantCastingPermission {
        permission,
        target,
        ..
    } = &*play_grant.effect
    else {
        panic!("expected GrantCastingPermission, got {:?}", play_grant.effect);
    };
    let CastingPermission::PlayFromExile { duration, .. } = permission else {
        panic!("expected PlayFromExile, got {permission:?}");
    };
    assert_eq!(
        *duration,
        Duration::UntilPlayerExilesAnotherCardWithSource {
            player: PlayerScope::Controller,
        }
    );
    assert_eq!(
        *target,
        TargetFilter::TrackedSet {
            id: TrackedSetId(0)
        }
    );
}
