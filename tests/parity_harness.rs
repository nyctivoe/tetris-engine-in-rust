use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use tetrisEngine::parity::{
    board_from_flat, canonicalize_json, engine_from_fixture, normalize_attack_stats,
    normalize_bag_remainder_counts, normalize_bfs_results, normalize_outgoing_attack_resolution,
    normalize_pending_garbage_summary, normalize_post_lock_prediction,
    normalize_queue_snapshot,
};
use tetrisEngine::{GarbageBatch, ParityFixtureSet, Piece, PieceKind, PlacementRecord, TetrisEngine};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("parity.json")
}

fn load_fixtures() -> ParityFixtureSet {
    let payload = fs::read_to_string(fixture_path()).expect("fixture file must exist");
    serde_json::from_str(&payload).expect("fixture file must deserialize")
}

fn canonical(value: Value) -> Value {
    canonicalize_json(value)
}

#[test]
fn fixture_round_trip_parsing() {
    let fixtures = load_fixtures();

    assert!(!fixtures.queue_snapshots.is_empty());
    assert!(!fixtures.bag_remainder_counts.is_empty());
    assert!(!fixtures.scored_clears.is_empty());
    assert!(!fixtures.lock_predictions.is_empty());
    assert!(!fixtures.garbage_summaries.is_empty());
    assert!(!fixtures.garbage_resolutions.is_empty());
    assert!(!fixtures.bfs_results.is_empty());
}

#[test]
fn parity_fixture_set_matches_python_reference() {
    let fixtures = load_fixtures();

    for case in &fixtures.queue_snapshots {
        let engine = engine_from_fixture(&case.state);
        let actual = canonical(normalize_queue_snapshot(&engine.get_queue_snapshot(case.next_slots)));
        assert_eq!(actual, canonical(case.expected.clone()), "queue fixture {}", case.name);
    }

    for case in &fixtures.bag_remainder_counts {
        let engine = engine_from_fixture(&case.state);
        let actual = canonical(normalize_bag_remainder_counts(&engine.get_bag_remainder_counts()));
        assert_eq!(
            actual,
            canonical(case.expected.clone()),
            "bag remainder fixture {}",
            case.name
        );
    }

    for case in &fixtures.scored_clears {
        let engine = engine_from_fixture(&case.state);
        let board_after_clear = board_from_flat(&case.board_after_clear);
        let spin_result = case.spin_result.as_ref().map(|spin| spin.to_runtime());
        let actual = canonical(normalize_attack_stats(&engine.compute_attack_for_clear(
            case.cleared_lines,
            spin_result,
            &board_after_clear,
            case.combo,
            case.combo_active,
            case.b2b_chain,
            case.surge_charge,
            case.base_attack,
        )));
        assert_eq!(
            actual,
            canonical(case.expected.clone()),
            "scored clear fixture {}",
            case.name
        );
    }

    for case in &fixtures.lock_predictions {
        let engine = engine_from_fixture(&case.state);
        let actual = canonical(normalize_post_lock_prediction(
            &engine.predict_post_lock_stats(&case.piece, case.base_attack),
        ));
        assert_eq!(
            actual,
            canonical(case.expected.clone()),
            "lock prediction fixture {}",
            case.name
        );
    }

    for case in &fixtures.garbage_summaries {
        let engine = engine_from_fixture(&case.state);
        let actual = canonical(normalize_pending_garbage_summary(
            &engine.get_pending_garbage_summary(),
        ));
        assert_eq!(
            actual,
            canonical(case.expected.clone()),
            "garbage summary fixture {}",
            case.name
        );
    }

    for case in &fixtures.garbage_resolutions {
        let mut engine = engine_from_fixture(&case.state);
        let actual = canonical(normalize_outgoing_attack_resolution(
            &engine.resolve_outgoing_attack(case.outgoing_attack, case.opener_phase),
        ));
        assert_eq!(
            actual,
            canonical(case.expected.clone()),
            "garbage resolution fixture {}",
            case.name
        );
    }

    for case in &fixtures.bfs_results {
        let engine = engine_from_fixture(&case.state);
        let results = engine.bfs_all_placements(
            case.piece.as_ref(),
            case.include_180,
            case.base_attack,
            case.include_no_place,
            case.dedupe_final,
        );
        let normalized = normalize_bfs_results(&results);
        let representative_len = case.expected["placements"]
            .as_array()
            .map(|placements| placements.len())
            .unwrap_or(0);
        let actual = canonical(json!({
            "count": results.len(),
            "placements": normalized
                .as_array()
                .expect("normalized bfs results are an array")
                .iter()
                .take(representative_len)
                .map(|result| result["placement"].clone())
                .collect::<Vec<_>>(),
        }));
        assert_eq!(actual, canonical(case.expected.clone()), "bfs fixture {}", case.name);
    }
}

#[test]
fn placement_record_serialization_shape_matches_python_contract() {
    let skip = serde_json::to_value(PlacementRecord::Skip).expect("skip placement should serialize");
    assert_eq!(skip, json!({ "skip": true }));

    let placed = serde_json::to_value(PlacementRecord::Placed {
        x: 3,
        y: 17,
        r: "N",
        rotation: 0,
        kind: PieceKind::T,
        last_was_rot: true,
        last_rot_dir: Some(1),
        last_kick_idx: Some(0),
    })
    .expect("placed record should serialize");
    assert_eq!(
        placed,
        json!({
            "x": 3,
            "y": 17,
            "r": "N",
            "rotation": 0,
            "kind": "T",
            "last_was_rot": true,
            "last_rot_dir": 1,
            "last_kick_idx": 0,
        })
    );
}

#[test]
fn clone_behavior_is_deep_for_runtime_state() {
    let mut engine = TetrisEngine::with_seed(7);
    engine.current_piece = Some(Piece::new(PieceKind::T, 1, (3, 0)));
    engine.hold = Some(1);
    engine.b2b_chain = 3;
    engine.surge_charge = 4;
    engine.combo = 2;
    engine.combo_active = true;
    engine.total_attack_sent = 9;
    engine.total_attack_canceled = 5;
    engine.incoming_garbage = vec![GarbageBatch {
        lines: 3,
        timer: 10,
        col: 4,
    }];
    engine.garbage_col = Some(4);

    let mut cloned = engine.clone();
    cloned.board[0] = 9;
    cloned.current_piece.as_mut().expect("clone has active piece").position = (5, 5);
    cloned.bag.remove(0);
    cloned.incoming_garbage[0].lines = 1;
    cloned.garbage_col = Some(2);
    cloned.total_attack_canceled = 8;

    assert_eq!(engine.board[0], 0);
    assert_eq!(
        engine.current_piece.expect("original keeps active piece").position,
        (3, 0)
    );
    assert_ne!(engine.bag.len(), cloned.bag.len());
    assert_eq!(engine.incoming_garbage[0].lines, 3);
    assert_eq!(engine.garbage_col, Some(4));
    assert_eq!(engine.total_attack_canceled, 5);
}

#[test]
fn reset_and_snapshot_clear_garbage_runtime_state() {
    let mut engine = TetrisEngine::with_seed(11);
    engine.current_piece = Some(Piece::new(PieceKind::I, 0, (3, 0)));
    engine.hold = Some(3);
    engine.total_attack_canceled = 6;
    engine.incoming_garbage = vec![GarbageBatch {
        lines: 2,
        timer: 5,
        col: 2,
    }];
    engine.garbage_col = Some(2);

    engine.reset();

    let snapshot = engine.get_queue_snapshot(2);
    assert_eq!(snapshot.current, None);
    assert_eq!(snapshot.hold, None);
    assert!(engine.incoming_garbage.is_empty());
    assert_eq!(engine.garbage_col, None);
    assert_eq!(engine.total_attack_canceled, 0);
    let skip_only = engine.empty_bfs_results(true);
    assert_eq!(skip_only.len(), 1);
    assert_eq!(skip_only[0].placement, PlacementRecord::Skip);
}
