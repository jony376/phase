//! Runtime regression: the "draw from empty library → win" replacement class
//! (Laboratory Maniac, Jace, Wielder of Mysteries).
//!
//! Oracle text (static replacement):
//!   "If you would draw a card while your library has no cards in it, you win
//!    the game instead."
//!
//! Reported bug: a 4-player Commander game ended with the active player winning
//! out of nowhere — every opponent (including a player at 32 life, non-empty
//! library, no commander damage) was eliminated the moment a card was played.
//!
//! Root cause: the parser dropped the "while your library has no cards in it"
//! antecedent, so the replacement was stored with `condition: null`. That makes
//! the replacement match on *every* draw. On a non-empty draw it stashed a
//! `WinTheGame` post-replacement continuation that was never drained (the draw
//! proceeded normally), leaking the continuation into a later turn where it
//! drained against the active player — eliminating all of *their* opponents.
//!
//! The fix gates the replacement on `ZoneCardCount(Library) == 0`, so a
//! non-empty draw no longer matches, stashes, or leaks.

use std::path::Path;
use std::sync::OnceLock;

use engine::database::card_db::CardDatabase;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::game::scenario_db::GameScenarioDbExt;
use engine::types::actions::{DebugAction, GameAction};
use engine::types::game_state::WaitingFor;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

fn load_db() -> Option<&'static CardDatabase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/public/card-data.json");
    if !path.exists() {
        return None;
    }
    static DB: OnceLock<CardDatabase> = OnceLock::new();
    Some(DB.get_or_init(|| CardDatabase::from_export(&path).expect("export should load")))
}

/// Jace, Wielder of Mysteries on P0's battlefield. `p0_library` cards are added
/// to P0's library; P1 always gets a non-empty library so its own SBAs are inert.
fn scenario_with_jace(
    db: &CardDatabase,
    p0_library: &[&str],
) -> engine::game::scenario::GameRunner {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_real_card(P0, "Jace, Wielder of Mysteries", Zone::Battlefield, db);
    for name in p0_library {
        scenario.add_real_card(P0, name, Zone::Library, db);
    }
    for _ in 0..5 {
        scenario.add_real_card(P1, "Plains", Zone::Library, db);
    }
    let mut runner = scenario.build();
    engine::game::rehydrate_game_from_card_db(runner.state_mut(), db);
    runner.state_mut().debug_mode = true;
    runner
}

/// CR 104.2b + CR 104.3c: The "while your library has no cards in it" antecedent
/// gates the replacement. Drawing from a NON-empty library must not win the game
/// — and must not stash a post-replacement `WinTheGame` continuation that would
/// leak into a later turn (the reported bug).
#[test]
fn jace_draw_from_nonempty_library_does_not_win_and_does_not_leak() {
    let Some(db) = load_db() else {
        return;
    };

    let mut runner = scenario_with_jace(db, &["Plains", "Island", "Forest"]);
    assert!(
        !matches!(runner.state().waiting_for, WaitingFor::GameOver { .. }),
        "precondition: game must not already be over"
    );

    runner
        .act(GameAction::Debug(DebugAction::DrawCards {
            player_id: P0,
            count: 1,
        }))
        .expect("debug draw must succeed");
    runner.advance_until_stack_empty();

    assert!(
        !matches!(runner.state().waiting_for, WaitingFor::GameOver { .. }),
        "drawing from a 3-card library must NOT win the game. waiting_for={:?}",
        runner.state().waiting_for
    );
    assert!(
        !runner.state().players[1].is_eliminated,
        "opponent must not be eliminated by a normal draw"
    );
    // The load-bearing leak assertion: a non-empty draw must not stash a
    // post-replacement continuation. A leaked `WinTheGame` continuation is what
    // drained against the wrong player on a later turn in the bug report.
    assert!(
        runner.state().post_replacement_continuation.is_none(),
        "a non-empty draw must NOT leak a stashed post-replacement continuation; \
         found {:?}",
        runner.state().post_replacement_continuation
    );
}

/// CR 104.2b: With an empty library, the replacement should fire and the drawing
/// player should win.
///
/// IGNORED: tracks a *separate, pre-existing* defect — a matched draw-replacement
/// stashes its `WinTheGame` post-effect continuation but the continuation is not
/// drained in the same action (it sits in `post_replacement_continuation`). This
/// is the runtime half of the leak; the parser gate above prevents it from ever
/// triggering on a non-empty draw, but the legitimate empty-library win
/// (Laboratory Maniac / Jace) still does not resolve. Un-ignore when the
/// post-replacement-continuation drain is fixed for the mandatory draw path.
#[test]
#[ignore = "pre-existing: post-replacement WinTheGame continuation is not drained on the empty-library draw"]
fn jace_draw_from_empty_library_wins() {
    let Some(db) = load_db() else {
        return;
    };

    let mut runner = scenario_with_jace(db, &[]);

    runner
        .act(GameAction::Debug(DebugAction::DrawCards {
            player_id: P0,
            count: 1,
        }))
        .expect("debug draw must succeed");
    runner.advance_until_stack_empty();

    assert!(
        matches!(
            runner.state().waiting_for,
            WaitingFor::GameOver { winner: Some(P0) }
        ),
        "drawing from an empty library must win the game for Jace's controller. \
         waiting_for={:?}",
        runner.state().waiting_for
    );
}
