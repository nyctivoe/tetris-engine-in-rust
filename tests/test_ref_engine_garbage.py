import numpy as np


def test_add_incoming_garbage_with_explicit_and_random_columns(engine_factory):
    engine = engine_factory(seed=0)

    engine.add_incoming_garbage(3, timer=10, col=4)
    engine.add_incoming_garbage(2, timer=5)

    assert engine.incoming_garbage[0] == {"lines": 3, "timer": 10, "col": 4}
    assert engine.incoming_garbage[1]["col"] != 4


def test_cancel_garbage_consumes_oldest_batches_first(engine_factory):
    engine = engine_factory()
    engine.incoming_garbage = [
        {"lines": 2, "timer": 10, "col": 1},
        {"lines": 3, "timer": 11, "col": 2},
    ]

    remaining_attack = engine.cancel_garbage(4)

    assert remaining_attack == 0
    assert engine.incoming_garbage == [{"lines": 1, "timer": 11, "col": 2}]


def test_resolve_outgoing_attack_uses_opener_multiplier(engine_factory):
    engine = engine_factory()
    engine.pieces_placed = 0
    engine.incoming_garbage = [
        {"lines": 5, "timer": 10, "col": 2},
    ]

    result = engine.resolve_outgoing_attack(3, opener_phase=True)

    assert result["used_opener_multiplier"] is True
    assert result["canceled"] == 5
    assert result["sent"] == 0
    assert result["incoming_after"]["total_lines"] == 0
    assert engine.total_attack_canceled == 5


def test_apply_garbage_shifts_board_up(engine_factory, engine_module):
    engine = engine_factory()
    engine.board[-3, 0] = 7

    engine.apply_garbage(2, col=4)

    assert engine.board[-1, 4] == 0
    assert np.all(engine.board[-1, np.arange(engine_module.BOARD_WIDTH) != 4] == engine_module.GARBAGE_ID)
    assert engine.garbage_col == 4
    assert engine.board[-5, 0] == 7


def test_tick_garbage_lands_expired_batches_and_keeps_pending(engine_factory):
    engine = engine_factory()
    engine.incoming_garbage = [
        {"lines": 2, "timer": 1, "col": 3},
        {"lines": 1, "timer": 3, "col": 5},
    ]

    landed = engine.tick_garbage()

    assert landed == 2
    assert engine.incoming_garbage == [{"lines": 1, "timer": 2, "col": 5}]
    assert engine.board[-1, 3] == 0
    assert engine.get_pending_garbage_summary() == {
        "total_lines": 1,
        "min_timer": 2,
        "max_timer": 2,
        "batch_count": 1,
        "landing_within_one_ply": False,
    }
