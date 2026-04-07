import numpy as np
import pytest


def _result_signature(engine_module, result):
    board = result["board"]
    board_key = None if board is None else (board.shape, board.dtype.str, board.tobytes())
    return (
        board_key,
        engine_module._freeze_stats_value(result["stats"]),
        engine_module._freeze_stats_value(result["placement"]),
    )


def test_spawn_position_and_bag_generation(engine_factory, engine_module):
    engine = engine_factory(seed=1)

    assert engine._spawn_position_for() == (engine_module.SPAWN_X, engine_module.SPAWN_Y)
    assert sorted(engine.bag[:7].tolist()) == [1, 2, 3, 4, 5, 6, 7]


def test_bag_replenishes_and_queue_snapshot_shape(engine_factory, engine_module):
    engine = engine_factory(seed=2)
    engine.bag = np.array([1], dtype=int)
    engine.bag_size = 1

    first_piece = engine._pop_next_piece_id()
    assert first_piece == 1
    assert engine.bag_size > 14

    engine.current_piece = engine_module.Piece("T", position=(3, 0))
    engine.hold = 1
    snapshot = engine.get_queue_snapshot(next_slots=3)

    assert snapshot["current"] == "T"
    assert snapshot["hold"] == "I"
    assert len(snapshot["next_ids"]) == 3
    assert len(snapshot["piece_ids"]) == 5


def test_position_validity_checks_bounds_and_collisions(engine_factory, engine_module):
    engine = engine_factory()
    piece = engine_module.Piece("T", position=(3, 0))

    assert engine.is_position_valid(piece, position=(3, 0)) is True
    assert engine.is_position_valid(piece, position=(-1, 0)) is False

    engine.board[1, 4] = 9
    assert engine.is_position_valid(piece, position=(3, 0)) is False


@pytest.mark.parametrize(
    ("piece", "direction", "expected_position", "expected_rotation", "expected_last_was_rotation", "expected_last_dir", "expected_kick_idx"),
    [
        ("T", "CW", (0, 0), 1, True, 1, 0),
        ("I", "CW", (1, 0), 1, True, 1, 0),
        ("O", "CW", (4, 4), 1, False, None, None),
        ("T", "180", (0, 0), 2, True, 2, 0),
        ("T", 2, (0, 0), 3, True, 2, 0),
    ],
)
def test_rotate_piece_preserves_current_kick_behavior(
    engine_factory,
    engine_module,
    piece,
    direction,
    expected_position,
    expected_rotation,
    expected_last_was_rotation,
    expected_last_dir,
    expected_kick_idx,
):
    engine = engine_factory()
    rotation = 1 if piece == "T" and direction == 2 else 0
    position = (4, 4) if piece == "O" else (0, 0)
    current = engine_module.Piece(piece, rotation=rotation, position=position)

    assert engine.rotate_piece(current, direction) is True
    assert current.position == expected_position
    assert current.rotation == expected_rotation
    assert current.last_action_was_rotation is expected_last_was_rotation
    assert current.last_rotation_dir == expected_last_dir
    assert current.last_kick_index == expected_kick_idx


def test_bfs_skip_toggle_and_dedupe_behavior(engine_factory, engine_module):
    engine = engine_factory()
    piece = engine_module.Piece("I", rotation=0, position=(3, 0))

    with_skip = engine.bfs_all_placements(piece, include_no_place=True, dedupe_final=True)
    without_skip = engine.bfs_all_placements(piece, include_no_place=False, dedupe_final=True)
    raw = engine.bfs_all_placements(piece, include_no_place=False, dedupe_final=False)

    assert with_skip[0]["placement"] == {"skip": True}
    assert all(result["placement"] != {"skip": True} for result in without_skip)
    assert len(raw) == 34
    assert len(without_skip) == 17
    assert all(len(result.get("placements", [])) == 2 for result in without_skip)


def test_numba_and_python_bfs_paths_match(engine_factory, engine_module):
    if not engine_module.NUMBA_AVAILABLE:
        pytest.skip("numba is not available in this environment")

    engine = engine_factory()
    piece = engine_module.Piece("T", rotation=0, position=(3, 0))
    bfs_inputs = engine._bfs_inputs_for_piece(piece)

    numba_results = engine._numba_bfs_results(piece, bfs_inputs, include_180=True, base_attack=None)
    python_results = engine._python_bfs_results(piece, bfs_inputs, include_180=True, base_attack=None)

    numba_signatures = sorted(_result_signature(engine_module, result) for result in numba_results)
    python_signatures = sorted(_result_signature(engine_module, result) for result in python_results)
    assert numba_signatures == python_signatures


def test_simulate_lock_is_non_mutating_and_predict_matches(engine_factory, engine_module):
    engine = engine_factory()
    piece = engine_module.Piece("O", rotation=0, position=(3, 37))
    board_before = engine.board.copy()

    simulated_board, simulated_stats = engine._simulate_lock(piece)
    predicted = engine.predict_post_lock_stats(piece)

    assert np.array_equal(engine.board, board_before)
    assert engine.current_piece is None
    assert np.array_equal(simulated_board, predicted["board"])
    assert simulated_stats == predicted["stats"]
    assert np.array_equal(predicted["blocks"], engine.piece_blocks(piece))


def test_lock_piece_mutates_board_and_counters(engine_factory, engine_module):
    engine = engine_factory()
    piece = engine_module.Piece("O", rotation=0, position=(3, 37))

    cleared = engine.lock_piece(piece=piece)

    assert cleared == 0
    assert engine.current_piece is None
    assert engine.pieces_placed == 1
    assert engine.total_lines_cleared == 0
    assert engine.board[38, 4] == engine_module.KIND_TO_PIECE_ID["O"]
    assert engine.last_clear_stats["t_spin"] == "N"


def test_apply_and_execute_placement_preserve_payload_shape(engine_factory, engine_module):
    engine = engine_factory()
    engine.current_piece = engine_module.Piece("T", rotation=0, position=(3, 0))

    ok = engine.apply_placement({
        "x": 3,
        "y": 0,
        "rotation": 1,
        "last_was_rot": True,
        "last_rot_dir": 1,
        "last_kick_idx": 0,
    })
    assert ok is True
    assert engine.current_piece.rotation == 1
    assert engine.current_piece.last_rotation_dir == 1
    assert engine.current_piece.last_kick_index == 0

    failed = engine.execute_placement({"x": -10, "y": 0, "rotation": 0})
    assert failed == {
        "ok": False,
        "lines_cleared": 0,
        "stats": None,
        "end_phase": None,
        "attack": 0,
    }
    assert engine.game_over_reason == "invalid_placement"


def test_spin_detection_variants(engine_factory, engine_module):
    engine = engine_factory(spin_mode="all_spin")

    t_full = engine_module.Piece("T", rotation=0, position=(4, 4))
    t_full.last_action_was_rotation = True
    t_full.last_rotation_dir = 1
    t_full.last_kick_index = 0
    engine.board.fill(0)
    for x, y in [(4, 4), (6, 4), (4, 6)]:
        engine.board[y, x] = 9
    full_spin = engine.detect_spin(t_full)
    assert full_spin["is_mini"] is False
    assert full_spin["description"] == "T-Spin"

    t_mini = engine_module.Piece("T", rotation=0, position=(4, 4))
    t_mini.last_action_was_rotation = True
    t_mini.last_rotation_dir = 1
    t_mini.last_kick_index = 0
    engine.board.fill(0)
    for x, y in [(4, 4), (4, 6), (6, 6)]:
        engine.board[y, x] = 9
    mini_spin = engine.detect_spin(t_mini)
    assert mini_spin["is_mini"] is True
    assert mini_spin["description"] == "T-Spin Mini"

    t_180 = engine_module.Piece("T", rotation=0, position=(4, 4))
    t_180.last_action_was_rotation = True
    t_180.last_rotation_dir = 2
    t_180.last_kick_index = 0
    spin_180 = engine.detect_spin(t_180)
    assert spin_180["is_180"] is True
    assert spin_180["description"] == "180 T-Spin"

    non_t = engine_module.Piece("J", rotation=0, position=(4, 4))
    non_t.last_action_was_rotation = True
    non_t.last_rotation_dir = 1
    non_t.last_kick_index = 0
    engine.board.fill(0)
    for x, y in [(3, 4), (5, 4)]:
        engine.board[y, x] = 9
    all_spin = engine.detect_spin(non_t)
    assert all_spin["description"] == "J-Spin Mini"

    non_rotated = engine_module.Piece("T", rotation=0, position=(4, 4))
    assert engine.detect_spin(non_rotated) is None
