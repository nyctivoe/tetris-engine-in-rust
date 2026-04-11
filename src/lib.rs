pub mod bfs;
pub mod board;
pub mod constants;
pub mod engine;
pub mod garbage;
pub mod parity;
pub mod piece;
#[cfg(feature = "python")]
mod python;
pub mod rng;
pub mod rotation;
pub mod scoring;

pub use bfs::{BfsInputs, BfsResult, PlacementRecord};
pub use board::{board_index, cell_blocked, compute_blocks, is_position_valid, Board};
pub use constants::{
    BOARD_HEIGHT, BOARD_WIDTH, GARBAGE_ID, HIDDEN_ROWS, ROTATION_NAMES, SPAWN_X, SPAWN_Y,
    VISIBLE_HEIGHT,
};
pub use engine::{
    BagRemainderCounts, EndPhaseResult, ExecutePlacementResult, PlacementPayload,
    PlacementSnapshot, PostLockPrediction, QueueSnapshot, TetrisEngine,
};
pub use garbage::{GarbageBatch, OutgoingAttackResolution, PendingGarbageSummary};
pub use parity::{
    AttackForClearFixture, BagRemainderCountsFixture, BfsFixture, EngineStateFixture,
    GarbageResolutionFixture, GarbageSummaryFixture, LockPredictionFixture, ParityFixtureSet,
    QueueSnapshotFixture,
};
pub use piece::{piece_id, piece_kind_from_id, Piece, PieceKind};
pub use rng::EngineRng;
pub use rotation::{
    rotation_candidates, rotation_delta_from_i8, rotation_delta_from_str, rotation_states,
    RotationDirection,
};
pub use scoring::{
    b2b_bonus_for_chain, base_attack_for_clear, classify_clear, combo_after_clear,
    combo_attack_down, is_difficult_clear, surge_segments, update_b2b_state, AttackStats, B2BMode,
    B2BUpdate, ClearClassification, ComboUpdate, SpinMode, SpinResult,
};
