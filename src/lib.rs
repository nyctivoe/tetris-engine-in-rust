pub mod board;
pub mod bfs;
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

pub use board::{Board, board_index, cell_blocked, compute_blocks, is_position_valid};
pub use bfs::{BfsInputs, BfsResult, PlacementRecord};
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
pub use piece::{Piece, PieceKind, piece_id, piece_kind_from_id};
pub use rng::EngineRng;
pub use rotation::{
    RotationDirection, rotation_candidates, rotation_delta_from_i8, rotation_delta_from_str,
    rotation_states,
};
pub use scoring::{AttackStats, B2BMode, B2BUpdate, SpinMode, SpinResult};
