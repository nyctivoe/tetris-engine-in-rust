import math

import numpy as np
import pytest


@pytest.mark.parametrize(
    ("cleared_lines", "expected"),
    [
        (1, 0),
        (2, 1),
        (3, 2),
        (4, 4),
    ],
)
def test_base_attack_for_standard_clears(engine_factory, cleared_lines, expected):
    engine = engine_factory()
    board_after_clear = np.zeros_like(engine.board)
    board_after_clear[0, 0] = 1

    stats = engine.compute_attack_for_clear(
        cleared_lines,
        None,
        board_after_clear=board_after_clear,
    )

    assert stats["base_attack"] == expected


def test_perfect_clear_base_attack(engine_factory):
    engine = engine_factory()
    stats = engine.compute_attack_for_clear(
        4,
        None,
        board_after_clear=np.zeros_like(engine.board),
    )

    assert stats["perfect_clear"] is True
    assert stats["base_attack"] == 10
    assert stats["attack"] == 10


@pytest.mark.parametrize(
    ("base_attack", "combo", "expected"),
    [
        (4, 2, 6),
        (0, 2, 1),
        (0, 1, 0),
    ],
)
def test_combo_attack_down_matches_current_rounding(engine_factory, base_attack, combo, expected):
    engine = engine_factory()
    assert engine.combo_attack_down(base_attack, combo=combo) == expected


@pytest.mark.parametrize(
    ("chain_len", "expected_bonus"),
    [
        (2, 0),
        (3, 1),
        (8, 2),
        (24, 3),
        (67, 4),
    ],
)
def test_b2b_bonus_ladder_thresholds(engine_factory, chain_len, expected_bonus):
    engine = engine_factory()
    assert engine._b2b_bonus_for_chain(chain_len) == expected_bonus


def test_b2b_and_surge_charge_and_release(engine_factory):
    engine = engine_factory()

    next_chain, next_surge, b2b_bonus, surge_send = engine._update_b2b_and_surge(
        cleared_lines=4,
        difficult=True,
        b2b_chain=4,
        surge_charge=0,
    )
    assert (next_chain, next_surge, b2b_bonus, surge_send) == (5, 4, 2, 0)

    next_chain, next_surge, b2b_bonus, surge_send = engine._update_b2b_and_surge(
        cleared_lines=1,
        difficult=False,
        b2b_chain=5,
        surge_charge=4,
    )
    assert (next_chain, next_surge, b2b_bonus, surge_send) == (0, 0, 0, 4)
    assert engine._surge_segments(4) == [2, 1, 1]


def test_compute_attack_for_clear_preserves_payload_shape(engine_factory):
    engine = engine_factory()
    board_after_clear = np.zeros_like(engine.board)
    board_after_clear[0, 0] = 1
    spin_result = {
        "spin_type": "t-spin",
        "is_mini": False,
    }

    stats = engine.compute_attack_for_clear(
        2,
        spin_result,
        board_after_clear=board_after_clear,
        combo=1,
        combo_active=True,
        b2b_chain=2,
        surge_charge=0,
    )

    expected_keys = {
        "attack",
        "b2b_bonus",
        "b2b_chain",
        "b2b_display",
        "base_attack",
        "breaks_b2b",
        "combo",
        "combo_active",
        "combo_attack",
        "combo_bonus",
        "combo_multiplier",
        "is_difficult",
        "is_mini",
        "is_spin",
        "lines_cleared",
        "perfect_clear",
        "qualifies_b2b",
        "spin",
        "spin_type",
        "surge_charge",
        "surge_segments",
        "surge_send",
    }

    assert expected_keys == set(stats)
    assert stats["base_attack"] == 4
    assert stats["combo_attack"] == 6
    assert stats["combo_bonus"] == 2
    assert math.isclose(stats["combo_multiplier"], 1.5)
    assert stats["attack"] == 7
