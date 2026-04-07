import sys
from pathlib import Path

import numpy as np
import pytest


REF_DIR = Path(__file__).resolve().parents[1] / "ref"
if str(REF_DIR) not in sys.path:
    sys.path.insert(0, str(REF_DIR))

import tetrisEngine as ref_engine


@pytest.fixture
def engine_module():
    return ref_engine


@pytest.fixture
def engine_factory(engine_module):
    def factory(*, spin_mode="t_only", seed=0):
        return engine_module.TetrisEngine(
            spin_mode=spin_mode,
            rng=np.random.default_rng(seed),
        )

    return factory
