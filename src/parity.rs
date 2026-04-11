use crate::bfs::{BfsResult, PlacementRecord};
use crate::engine::{BagRemainderCounts, PostLockPrediction, QueueSnapshot, TetrisEngine};
use crate::garbage::{GarbageBatch, OutgoingAttackResolution, PendingGarbageSummary};
use crate::piece::{Piece, PieceKind};
use crate::scoring::{AttackStats, B2BMode, SpinMode, SpinResult};
use crate::{Board, BOARD_HEIGHT, BOARD_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EngineStateFixture {
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub spin_mode: Option<String>,
    #[serde(default)]
    pub b2b_mode: Option<String>,
    #[serde(default)]
    pub board: Vec<i8>,
    #[serde(default)]
    pub current_piece: Option<Piece>,
    #[serde(default)]
    pub bag: Vec<i8>,
    #[serde(default)]
    pub hold: Option<i8>,
    #[serde(default)]
    pub hold_locked: bool,
    #[serde(default)]
    pub bag_size: Option<usize>,
    #[serde(default)]
    pub b2b_chain: i32,
    #[serde(default)]
    pub surge_charge: i32,
    #[serde(default)]
    pub combo: i32,
    #[serde(default)]
    pub combo_active: bool,
    #[serde(default)]
    pub game_over: bool,
    #[serde(default)]
    pub game_over_reason: Option<String>,
    #[serde(default)]
    pub last_spawn_was_clutch: bool,
    #[serde(default)]
    pub pieces_placed: i32,
    #[serde(default)]
    pub total_lines_cleared: i32,
    #[serde(default)]
    pub total_attack_sent: i32,
    #[serde(default)]
    pub total_attack_canceled: i32,
    #[serde(default)]
    pub incoming_garbage: Vec<GarbageBatch>,
    #[serde(default)]
    pub garbage_col: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QueueSnapshotFixture {
    pub name: String,
    pub state: EngineStateFixture,
    pub next_slots: usize,
    pub expected: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BagRemainderCountsFixture {
    pub name: String,
    pub state: EngineStateFixture,
    pub expected: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SpinResultFixture {
    pub piece: PieceKind,
    pub spin_type: String,
    pub is_mini: bool,
    pub is_180: bool,
    pub kick_index: Option<u8>,
    pub rotation_dir: Option<i8>,
    pub corners: Option<u8>,
    pub front_corners: Option<u8>,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttackForClearFixture {
    pub name: String,
    pub state: EngineStateFixture,
    pub cleared_lines: i32,
    pub spin_result: Option<SpinResultFixture>,
    pub board_after_clear: Vec<i8>,
    pub combo: Option<i32>,
    pub combo_active: Option<bool>,
    pub b2b_chain: Option<i32>,
    pub surge_charge: Option<i32>,
    pub base_attack: Option<i32>,
    pub expected: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LockPredictionFixture {
    pub name: String,
    pub state: EngineStateFixture,
    pub piece: Piece,
    pub base_attack: Option<i32>,
    pub expected: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GarbageSummaryFixture {
    pub name: String,
    pub state: EngineStateFixture,
    pub expected: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GarbageResolutionFixture {
    pub name: String,
    pub state: EngineStateFixture,
    pub outgoing_attack: i32,
    pub opener_phase: Option<bool>,
    pub expected: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BfsFixture {
    pub name: String,
    pub state: EngineStateFixture,
    pub piece: Option<Piece>,
    pub include_180: bool,
    pub base_attack: Option<i32>,
    pub include_no_place: bool,
    pub dedupe_final: bool,
    pub expected: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParityFixtureSet {
    #[serde(default)]
    pub queue_snapshots: Vec<QueueSnapshotFixture>,
    #[serde(default)]
    pub bag_remainder_counts: Vec<BagRemainderCountsFixture>,
    #[serde(default)]
    pub scored_clears: Vec<AttackForClearFixture>,
    #[serde(default)]
    pub lock_predictions: Vec<LockPredictionFixture>,
    #[serde(default)]
    pub garbage_summaries: Vec<GarbageSummaryFixture>,
    #[serde(default)]
    pub garbage_resolutions: Vec<GarbageResolutionFixture>,
    #[serde(default)]
    pub bfs_results: Vec<BfsFixture>,
}

impl SpinResultFixture {
    pub fn to_runtime(&self) -> SpinResult {
        SpinResult {
            piece: self.piece,
            spin_type: spin_type_str(&self.spin_type),
            is_mini: self.is_mini,
            is_180: self.is_180,
            kick_index: self.kick_index,
            rotation_dir: self.rotation_dir,
            corners: self.corners,
            front_corners: self.front_corners,
            description: self.description.clone(),
        }
    }
}

pub fn engine_from_fixture(state: &EngineStateFixture) -> TetrisEngine {
    let spin_mode = match state.spin_mode.as_deref() {
        Some("t_only") => SpinMode::TOnly,
        _ => SpinMode::AllSpin,
    };
    let b2b_mode = match state.b2b_mode.as_deref() {
        Some("chaining") => B2BMode::Chaining,
        _ => B2BMode::Surge,
    };
    let mut engine =
        TetrisEngine::with_seed_and_modes(state.seed.unwrap_or(0), spin_mode, b2b_mode);
    if !state.board.is_empty() {
        engine.board = board_from_flat(&state.board);
    } else {
        engine.board = [0; BOARD_WIDTH * BOARD_HEIGHT];
    }
    engine.current_piece = state.current_piece;
    engine.bag = state.bag.clone();
    engine.hold = state.hold;
    engine.hold_locked = state.hold_locked;
    engine.bag_size = state.bag_size.unwrap_or(engine.bag.len());
    engine.b2b_chain = state.b2b_chain;
    engine.surge_charge = state.surge_charge;
    engine.combo = state.combo;
    engine.combo_active = state.combo_active;
    engine.game_over = state.game_over;
    engine.game_over_reason = state.game_over_reason.clone();
    engine.last_spawn_was_clutch = state.last_spawn_was_clutch;
    engine.pieces_placed = state.pieces_placed;
    engine.total_lines_cleared = state.total_lines_cleared;
    engine.total_attack_sent = state.total_attack_sent;
    engine.total_attack_canceled = state.total_attack_canceled;
    engine.incoming_garbage = state.incoming_garbage.clone();
    engine.garbage_col = state.garbage_col;
    engine.last_clear_stats = None;
    engine.last_end_phase = None;
    engine
}

pub fn board_from_flat(flat: &[i8]) -> Board {
    assert_eq!(
        flat.len(),
        BOARD_WIDTH * BOARD_HEIGHT,
        "board fixtures must contain exactly 400 cells"
    );
    let mut board = [0; BOARD_WIDTH * BOARD_HEIGHT];
    board.copy_from_slice(flat);
    board
}

pub fn normalize_board(board: &Board) -> Value {
    Value::Array(board.iter().copied().map(Value::from).collect())
}

pub fn normalize_queue_snapshot(snapshot: &QueueSnapshot) -> Value {
    json!({
        "current": snapshot.current,
        "hold": snapshot.hold,
        "next_ids": snapshot.next_ids,
        "next_kinds": snapshot.next_kinds,
        "piece_ids": snapshot.piece_ids,
    })
}

pub fn normalize_bag_remainder_counts(counts: &BagRemainderCounts) -> Value {
    json!({
        "counts": {
            "I": counts.counts[0],
            "O": counts.counts[1],
            "T": counts.counts[2],
            "S": counts.counts[3],
            "Z": counts.counts[4],
            "J": counts.counts[5],
            "L": counts.counts[6],
        },
        "remaining": counts.remaining,
        "bag_position": counts.bag_position,
    })
}

pub fn normalize_spin_result(spin: &SpinResult) -> Value {
    json!({
        "piece": spin.piece,
        "spin_type": spin.spin_type,
        "is_mini": spin.is_mini,
        "is_180": spin.is_180,
        "kick_index": spin.kick_index,
        "rotation_dir": spin.rotation_dir,
        "corners": spin.corners,
        "front_corners": spin.front_corners,
        "description": spin.description,
    })
}

pub fn normalize_attack_stats(stats: &AttackStats) -> Value {
    json!({
        "attack": stats.attack,
        "b2b_bonus": stats.b2b_bonus,
        "b2b_chain": stats.b2b_chain,
        "b2b_display": stats.b2b_display,
        "b2b_mode": stats.b2b_mode,
        "base_attack": stats.base_attack,
        "breaks_b2b": stats.breaks_b2b,
        "combo": stats.combo,
        "combo_active": stats.combo_active,
        "combo_attack": stats.combo_attack,
        "combo_bonus": stats.combo_bonus,
        "combo_multiplier": stats.combo_multiplier,
        "is_difficult": stats.is_difficult,
        "is_mini": stats.is_mini,
        "is_spin": stats.is_spin,
        "lines_cleared": stats.lines_cleared,
        "perfect_clear": stats.perfect_clear,
        "qualifies_b2b": stats.qualifies_b2b,
        "spin": stats.spin.as_ref().map(normalize_spin_result),
        "spin_type": stats.spin_type,
        "surge_charge": stats.surge_charge,
        "surge_segments": stats.surge_segments,
        "surge_send": stats.surge_send,
        "t_spin": stats.t_spin,
        "garbage_cleared": stats.garbage_cleared,
        "immediate_garbage": stats.immediate_garbage,
    })
}

pub fn normalize_post_lock_prediction(prediction: &PostLockPrediction) -> Value {
    json!({
        "board": normalize_board(&prediction.board),
        "stats": normalize_attack_stats(&prediction.stats),
        "blocks": prediction.blocks,
        "placement": prediction.placement,
    })
}

pub fn normalize_pending_garbage_summary(summary: &PendingGarbageSummary) -> Value {
    json!({
        "total_lines": summary.total_lines,
        "min_timer": summary.min_timer,
        "max_timer": summary.max_timer,
        "batch_count": summary.batch_count,
        "landing_within_one_ply": summary.landing_within_one_ply,
    })
}

pub fn normalize_outgoing_attack_resolution(resolution: &OutgoingAttackResolution) -> Value {
    json!({
        "incoming_before": normalize_pending_garbage_summary(&resolution.incoming_before),
        "incoming_after": normalize_pending_garbage_summary(&resolution.incoming_after),
        "outgoing_attack": resolution.outgoing_attack,
        "canceled": resolution.canceled,
        "sent": resolution.sent,
        "used_opener_multiplier": resolution.used_opener_multiplier,
        "opener_phase": resolution.opener_phase,
    })
}

pub fn normalize_placement_record(record: &PlacementRecord) -> Value {
    match record {
        PlacementRecord::Skip => json!({ "skip": true }),
        PlacementRecord::Placed {
            x,
            y,
            r,
            rotation,
            kind,
            last_was_rot,
            last_rot_dir,
            last_kick_idx,
        } => json!({
            "x": x,
            "y": y,
            "r": r,
            "rotation": rotation,
            "kind": kind,
            "last_was_rot": last_was_rot,
            "last_rot_dir": last_rot_dir,
            "last_kick_idx": last_kick_idx,
        }),
    }
}

pub fn normalize_bfs_result(result: &BfsResult) -> Value {
    let placements = sorted_placement_vec(&result.placements);
    let placement = if !placements.is_empty() {
        placements[0].clone()
    } else {
        normalize_placement_record(&result.placement)
    };
    json!({
        "board": result.board.as_ref().map(normalize_board),
        "stats": result.stats.as_ref().map(normalize_attack_stats),
        "placement": placement,
        "placements": placements,
    })
}

pub fn normalize_bfs_results(results: &[BfsResult]) -> Value {
    let mut normalized = results
        .iter()
        .map(|result| {
            let value = normalize_bfs_result(result);
            let placement = value
                .get("placement")
                .cloned()
                .expect("normalized bfs result always has placement");
            (placement_value_sort_key(&placement), value)
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.0.cmp(&right.0));
    Value::Array(normalized.into_iter().map(|(_, value)| value).collect())
}

fn sorted_placement_vec(placements: &[PlacementRecord]) -> Vec<Value> {
    let mut normalized = placements
        .iter()
        .map(|placement| {
            (
                placement_sort_key(placement),
                normalize_placement_record(placement),
            )
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.0.cmp(&right.0));
    normalized.into_iter().map(|(_, value)| value).collect()
}

fn placement_sort_key(placement: &PlacementRecord) -> (u8, i16, i16, i8, i16) {
    match placement {
        PlacementRecord::Skip => (u8::MAX, i16::MAX, i16::MAX, i8::MAX, i16::MAX),
        PlacementRecord::Placed {
            rotation,
            y,
            x,
            last_rot_dir,
            last_kick_idx,
            ..
        } => (
            *rotation,
            *y,
            *x,
            last_rot_dir.unwrap_or(i8::MIN),
            i16::from(last_kick_idx.unwrap_or(u8::MAX)),
        ),
    }
}

fn placement_value_sort_key(value: &Value) -> (u8, i16, i16, i8, i16) {
    if value.get("skip").and_then(Value::as_bool).unwrap_or(false) {
        return (u8::MAX, i16::MAX, i16::MAX, i8::MAX, i16::MAX);
    }

    (
        value["rotation"].as_u64().unwrap_or(u8::MAX as u64) as u8,
        value["y"].as_i64().unwrap_or(i16::MAX as i64) as i16,
        value["x"].as_i64().unwrap_or(i16::MAX as i64) as i16,
        value["last_rot_dir"].as_i64().unwrap_or(i8::MIN as i64) as i8,
        value["last_kick_idx"].as_u64().unwrap_or(u8::MAX as u64) as i16,
    )
}

fn spin_type_str(value: &str) -> &'static str {
    match value {
        "t-spin" => "t-spin",
        "spin" => "spin",
        _ => "spin",
    }
}

pub fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        Value::Object(map) => {
            let mut ordered = Map::new();
            let mut entries = map.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, value) in entries {
                ordered.insert(key, canonicalize_json(value));
            }
            Value::Object(ordered)
        }
        other => other,
    }
}
