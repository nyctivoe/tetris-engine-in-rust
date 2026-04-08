#![allow(unsafe_op_in_unsafe_fn)]

use std::cell::{Cell, RefCell};

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PyModule};
use serde::Serialize;
use serde_json::Value;

use crate::board::Board;
use crate::constants::{BOARD_HEIGHT, BOARD_WIDTH, GARBAGE_ID, HIDDEN_ROWS, SPAWN_X, SPAWN_Y, VISIBLE_HEIGHT};
use crate::engine::{BagRemainderCounts, QueueSnapshot, TetrisEngine as CoreEngine};
use crate::garbage::PendingGarbageSummary;
use crate::piece::{Piece, PieceKind, piece_id};
use crate::rotation::{rotation_delta_from_i8, rotation_delta_from_str, rotation_states};
use crate::scoring::{B2BMode, SpinMode};

fn piece_kind_as_str(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::I => "I",
        PieceKind::O => "O",
        PieceKind::T => "T",
        PieceKind::S => "S",
        PieceKind::Z => "Z",
        PieceKind::J => "J",
        PieceKind::L => "L",
    }
}

fn piece_kind_from_str(kind: &str) -> Option<PieceKind> {
    match kind {
        "I" => Some(PieceKind::I),
        "O" => Some(PieceKind::O),
        "T" => Some(PieceKind::T),
        "S" => Some(PieceKind::S),
        "Z" => Some(PieceKind::Z),
        "J" => Some(PieceKind::J),
        "L" => Some(PieceKind::L),
        _ => None,
    }
}

fn spin_mode_from_str(mode: &str) -> PyResult<SpinMode> {
    match mode {
        "t_only" => Ok(SpinMode::TOnly),
        "all_spin" => Ok(SpinMode::AllSpin),
        _ => Err(PyValueError::new_err(format!("Unsupported spin mode: {mode}"))),
    }
}

fn b2b_mode_from_str(mode: &str) -> PyResult<B2BMode> {
    match mode {
        "surge" => Ok(B2BMode::Surge),
        "chaining" => Ok(B2BMode::Chaining),
        _ => Err(PyValueError::new_err(format!("Unsupported b2b_mode: {mode}"))),
    }
}

fn rotation_delta_from_py(direction: &Bound<'_, PyAny>) -> PyResult<i8> {
    if let Ok(raw) = direction.extract::<i8>() {
        return rotation_delta_from_i8(raw)
            .ok_or_else(|| PyValueError::new_err(format!("Unsupported rotation direction: {raw}")));
    }

    if let Ok(raw) = direction.extract::<&str>() {
        return rotation_delta_from_str(raw)
            .ok_or_else(|| PyValueError::new_err(format!("Unsupported rotation direction: {raw}")));
    }

    Err(PyTypeError::new_err(
        "Rotation direction must be an int or one of 'CW', 'CCW', '180'",
    ))
}

fn board_rows(board: &Board) -> Vec<Vec<i8>> {
    board
        .chunks_exact(BOARD_WIDTH)
        .map(|row| row.to_vec())
        .collect()
}

fn serde_value_to_py(py: Python<'_>, value: Value) -> PyResult<PyObject> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(value) => value.into_py(py),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                value.into_py(py)
            } else if let Some(value) = value.as_u64() {
                value.into_py(py)
            } else if let Some(value) = value.as_f64() {
                value.into_py(py)
            } else {
                py.None()
            }
        }
        Value::String(value) => value.into_py(py),
        Value::Array(values) => {
            let list = PyList::empty_bound(py);
            for value in values {
                list.append(serde_value_to_py(py, value)?)?;
            }
            list.into_py(py)
        }
        Value::Object(values) => {
            let dict = PyDict::new_bound(py);
            for (key, value) in values {
                dict.set_item(key, serde_value_to_py(py, value)?)?;
            }
            dict.into_py(py)
        }
    })
}

fn serialize_to_py<T: Serialize>(py: Python<'_>, value: &T) -> PyResult<PyObject> {
    let value = serde_json::to_value(value)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize engine state: {err}")))?;
    serde_value_to_py(py, value)
}

fn optional_serialize_to_py<T: Serialize>(py: Python<'_>, value: Option<&T>) -> PyResult<PyObject> {
    match value {
        Some(value) => serialize_to_py(py, value),
        None => Ok(py.None()),
    }
}

fn queue_snapshot_to_py(py: Python<'_>, snapshot: &QueueSnapshot) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    dict.set_item("current", snapshot.current.map(piece_kind_as_str))?;
    dict.set_item("hold", snapshot.hold.map(piece_kind_as_str))?;
    dict.set_item("next_ids", snapshot.next_ids.clone())?;
    dict.set_item(
        "next_kinds",
        snapshot
            .next_kinds
            .iter()
            .map(|kind| piece_kind_as_str(*kind))
            .collect::<Vec<_>>(),
    )?;
    dict.set_item("piece_ids", snapshot.piece_ids.clone())?;
    Ok(dict.into_py(py))
}

fn bag_counts_to_py(py: Python<'_>, counts: &BagRemainderCounts) -> PyResult<PyObject> {
    let count_dict = PyDict::new_bound(py);
    for kind in [
        PieceKind::I,
        PieceKind::O,
        PieceKind::T,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::J,
        PieceKind::L,
    ] {
        count_dict.set_item(piece_kind_as_str(kind), counts.counts[kind as usize])?;
    }

    let dict = PyDict::new_bound(py);
    dict.set_item("counts", count_dict)?;
    dict.set_item("remaining", counts.remaining)?;
    dict.set_item("bag_position", counts.bag_position)?;
    Ok(dict.into_py(py))
}

fn pending_garbage_to_py(py: Python<'_>, pending: &PendingGarbageSummary) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    dict.set_item("total_lines", pending.total_lines)?;
    dict.set_item("min_timer", pending.min_timer)?;
    dict.set_item("max_timer", pending.max_timer)?;
    dict.set_item("batch_count", pending.batch_count)?;
    dict.set_item("landing_within_one_ply", pending.landing_within_one_ply)?;
    Ok(dict.into_py(py))
}

#[pyclass(name = "Piece", module = "tetrisEngine_rs")]
#[derive(Clone)]
pub struct PyPiece {
    #[pyo3(get, set)]
    pub kind: String,
    #[pyo3(get, set)]
    pub rotation: u8,
    #[pyo3(get, set)]
    pub position: (i16, i16),
    #[pyo3(get, set)]
    pub last_action_was_rotation: bool,
    #[pyo3(get, set)]
    pub last_rotation_dir: Option<i8>,
    #[pyo3(get, set)]
    pub last_kick_index: Option<u8>,
}

impl PyPiece {
    fn from_piece(piece: Piece) -> Self {
        Self {
            kind: piece_kind_as_str(piece.kind).to_owned(),
            rotation: piece.rotation,
            position: piece.position,
            last_action_was_rotation: piece.last_action_was_rotation,
            last_rotation_dir: piece.last_rotation_dir,
            last_kick_index: piece.last_kick_index,
        }
    }

    fn to_piece(&self) -> PyResult<Piece> {
        let kind = piece_kind_from_str(&self.kind)
            .ok_or_else(|| PyValueError::new_err(format!("Unsupported piece kind: {}", self.kind)))?;
        Ok(Piece {
            kind,
            rotation: self.rotation % 4,
            position: self.position,
            last_action_was_rotation: self.last_action_was_rotation,
            last_rotation_dir: self.last_rotation_dir,
            last_kick_index: self.last_kick_index,
        })
    }
}

#[pymethods]
impl PyPiece {
    fn copy(&self) -> Self {
        self.clone()
    }

    fn __str__(&self) -> String {
        format!("{} at {:?} (Rot {})", self.kind, self.position, self.rotation)
    }
}

#[pyclass(name = "TetrisEngine", module = "tetrisEngine_rs")]
pub struct PyTetrisEngine {
    inner: RefCell<CoreEngine>,
    gravity_timer_ms: Cell<i32>,
    lock_timer_ms: Cell<i32>,
}

impl PyTetrisEngine {
    fn reset_runtime_timers(&self) {
        self.gravity_timer_ms.set(0);
        self.lock_timer_ms.set(0);
    }

    fn after_piece_motion(&self) {
        self.lock_timer_ms.set(0);
    }

    fn translate_current_internal(&self, dx: i16, dy: i16) -> bool {
        let moved = self.inner.borrow_mut().translate_current(dx, dy);
        if moved {
            self.after_piece_motion();
        }
        moved
    }

    fn fall_one_row_internal(&self) -> bool {
        let moved = self.inner.borrow_mut().translate_current(0, 1);
        if moved {
            self.after_piece_motion();
        }
        moved
    }

    fn tick_garbage_internal(&self) -> i32 {
        let mut engine = self.inner.borrow_mut();
        let landed = engine.tick_garbage();
        if landed > 0 {
            if let Some(piece) = engine.current_piece {
                if !engine.is_position_valid(&piece, Some(piece.position), Some(piece.rotation)) {
                    engine.game_over = true;
                    engine.game_over_reason = Some("garbage_top_out".to_string());
                }
            }
        }
        landed
    }
}

#[pymethods]
impl PyTetrisEngine {
    #[new]
    #[pyo3(signature = (spin_mode="all_spin", b2b_mode="surge", seed=0, rng=None))]
    fn new(spin_mode: &str, b2b_mode: &str, seed: u64, rng: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let _ = rng;
        let spin_mode = spin_mode_from_str(spin_mode)?;
        let b2b_mode = b2b_mode_from_str(b2b_mode)?;
        Ok(Self {
            inner: RefCell::new(CoreEngine::with_seed_and_modes(seed, spin_mode, b2b_mode)),
            gravity_timer_ms: Cell::new(0),
            lock_timer_ms: Cell::new(0),
        })
    }

    fn reset(&self) {
        self.inner.borrow_mut().reset();
        self.reset_runtime_timers();
    }

    #[pyo3(signature = (allow_clutch=false))]
    fn spawn_next(&self, allow_clutch: bool) -> bool {
        let spawned = self.inner.borrow_mut().spawn_next(allow_clutch);
        if spawned {
            self.reset_runtime_timers();
        }
        spawned
    }

    fn hold_current(&self) -> bool {
        let held = self.inner.borrow_mut().hold_current();
        if held {
            self.reset_runtime_timers();
        }
        held
    }

    fn rotate_current(&self, direction: &Bound<'_, PyAny>) -> PyResult<bool> {
        let delta = rotation_delta_from_py(direction)?;
        let rotated = self.inner.borrow_mut().rotate_current(delta);
        if rotated {
            self.after_piece_motion();
        }
        Ok(rotated)
    }

    fn translate_current(&self, dx: i16, dy: i16) -> bool {
        self.translate_current_internal(dx, dy)
    }

    fn move_left(&self) -> bool {
        self.translate_current_internal(-1, 0)
    }

    fn move_right(&self) -> bool {
        self.translate_current_internal(1, 0)
    }

    fn soft_drop(&self) -> bool {
        let moved = self.fall_one_row_internal();
        if moved {
            self.gravity_timer_ms.set(0);
        }
        moved
    }

    fn hard_drop(&self) -> bool {
        {
            let engine = self.inner.borrow();
            if engine.game_over || engine.current_piece.is_none() {
                return false;
            }
        }

        while self.inner.borrow_mut().translate_current(0, 1) {}

        let cleared = self.inner.borrow_mut().lock_and_spawn(None).0;
        self.reset_runtime_timers();
        cleared >= 0
    }

    fn tick_runtime(&self, dt_ms: i32, gravity_ms: i32, lock_delay_ms: i32) {
        let dt_ms = dt_ms.max(0);
        let gravity_ms = gravity_ms.max(1);
        let lock_delay_ms = lock_delay_ms.max(1);

        let can_fall = {
            let engine = self.inner.borrow();
            if engine.game_over {
                return;
            }
            let Some(piece) = engine.current_piece else {
                return;
            };
            engine.is_position_valid(
                &piece,
                Some((piece.position.0, piece.position.1 + 1)),
                Some(piece.rotation),
            )
        };

        if can_fall {
            self.lock_timer_ms.set(0);
            let mut gravity_timer_ms = self.gravity_timer_ms.get() + dt_ms;

            while gravity_timer_ms >= gravity_ms {
                if !self.fall_one_row_internal() {
                    break;
                }
                gravity_timer_ms -= gravity_ms;
            }

            self.gravity_timer_ms.set(gravity_timer_ms);
            return;
        }

        self.gravity_timer_ms.set(0);
        let lock_timer_ms = self.lock_timer_ms.get() + dt_ms;
        if lock_timer_ms >= lock_delay_ms {
            self.inner.borrow_mut().lock_and_spawn(None);
            self.lock_timer_ms.set(0);
        } else {
            self.lock_timer_ms.set(lock_timer_ms);
        }
    }

    #[pyo3(signature = (piece, position=None, rotation=None))]
    fn piece_blocks(
        &self,
        piece: PyRef<'_, PyPiece>,
        position: Option<(i16, i16)>,
        rotation: Option<u8>,
    ) -> PyResult<Vec<(i16, i16)>> {
        let piece = piece.to_piece()?;
        Ok(self
            .inner
            .borrow()
            .piece_blocks(&piece, position, rotation)
            .into_iter()
            .collect())
    }

    #[pyo3(signature = (piece, position=None, rotation=None))]
    fn is_position_valid(
        &self,
        piece: PyRef<'_, PyPiece>,
        position: Option<(i16, i16)>,
        rotation: Option<u8>,
    ) -> PyResult<bool> {
        let piece = piece.to_piece()?;
        Ok(self
            .inner
            .borrow()
            .is_position_valid(&piece, position, rotation))
    }

    fn board_with_active_piece(&self) -> Vec<Vec<i8>> {
        let board = self.inner.borrow().board_with_active_piece();
        board_rows(&board)
    }

    fn ghost_position(&self) -> Option<(i16, i16)> {
        self.inner.borrow().ghost_position()
    }

    fn lock_and_spawn(&self, py: Python<'_>) -> PyResult<(i32, PyObject)> {
        let (cleared, end_phase) = self.inner.borrow_mut().lock_and_spawn(None);
        self.reset_runtime_timers();
        Ok((cleared, serialize_to_py(py, &end_phase)?))
    }

    #[pyo3(signature = (lines, timer=60, col=None))]
    fn add_incoming_garbage(&self, lines: i32, timer: i32, col: Option<u8>) {
        self.inner.borrow_mut().add_incoming_garbage(lines, timer, col);
    }

    fn tick_garbage(&self) -> i32 {
        self.tick_garbage_internal()
    }

    #[pyo3(signature = (next_slots=5))]
    fn get_queue_snapshot(&self, py: Python<'_>, next_slots: usize) -> PyResult<PyObject> {
        let snapshot = self.inner.borrow().get_queue_snapshot(next_slots);
        queue_snapshot_to_py(py, &snapshot)
    }

    fn get_bag_remainder_counts(&self, py: Python<'_>) -> PyResult<PyObject> {
        let counts = self.inner.borrow().get_bag_remainder_counts();
        bag_counts_to_py(py, &counts)
    }

    fn get_pending_garbage_summary(&self, py: Python<'_>) -> PyResult<PyObject> {
        let pending = self.inner.borrow().get_pending_garbage_summary();
        pending_garbage_to_py(py, &pending)
    }

    #[getter]
    fn current_piece(&self) -> Option<PyPiece> {
        self.inner.borrow().current_piece.map(PyPiece::from_piece)
    }

    #[getter]
    fn bag(&self) -> Vec<i8> {
        self.inner.borrow().bag.clone()
    }

    #[getter]
    fn hold(&self) -> Option<i8> {
        self.inner.borrow().hold
    }

    #[getter]
    fn hold_locked(&self) -> bool {
        self.inner.borrow().hold_locked
    }

    #[getter]
    fn bag_size(&self) -> usize {
        self.inner.borrow().bag_size
    }

    #[getter]
    fn spin_mode(&self) -> String {
        self.inner.borrow().spin_mode.as_str().to_owned()
    }

    #[getter]
    fn b2b_mode(&self) -> String {
        self.inner.borrow().b2b_mode.as_str().to_owned()
    }

    #[getter]
    fn b2b_chain(&self) -> i32 {
        self.inner.borrow().b2b_chain
    }

    #[getter]
    fn surge_charge(&self) -> i32 {
        self.inner.borrow().surge_charge
    }

    #[getter]
    fn combo(&self) -> i32 {
        self.inner.borrow().combo
    }

    #[getter]
    fn combo_active(&self) -> bool {
        self.inner.borrow().combo_active
    }

    #[getter]
    fn game_over(&self) -> bool {
        self.inner.borrow().game_over
    }

    #[setter]
    fn set_game_over(&self, value: bool) {
        self.inner.borrow_mut().game_over = value;
    }

    #[getter]
    fn game_over_reason(&self) -> Option<String> {
        self.inner.borrow().game_over_reason.clone()
    }

    #[setter]
    fn set_game_over_reason(&self, value: Option<String>) {
        self.inner.borrow_mut().game_over_reason = value;
    }

    #[getter]
    fn last_spawn_was_clutch(&self) -> bool {
        self.inner.borrow().last_spawn_was_clutch
    }

    #[getter]
    fn last_clear_stats(&self, py: Python<'_>) -> PyResult<PyObject> {
        optional_serialize_to_py(py, self.inner.borrow().last_clear_stats.as_ref())
    }

    #[getter]
    fn last_end_phase(&self, py: Python<'_>) -> PyResult<PyObject> {
        optional_serialize_to_py(py, self.inner.borrow().last_end_phase.as_ref())
    }

    #[getter]
    fn pieces_placed(&self) -> i32 {
        self.inner.borrow().pieces_placed
    }

    #[getter]
    fn total_lines_cleared(&self) -> i32 {
        self.inner.borrow().total_lines_cleared
    }

    #[getter]
    fn total_attack_sent(&self) -> i32 {
        self.inner.borrow().total_attack_sent
    }

    #[getter]
    fn total_attack_canceled(&self) -> i32 {
        self.inner.borrow().total_attack_canceled
    }

    #[getter]
    fn gravity_timer_ms(&self) -> i32 {
        self.gravity_timer_ms.get()
    }

    #[getter]
    fn lock_timer_ms(&self) -> i32 {
        self.lock_timer_ms.get()
    }
}

fn add_piece_mappings(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let piece_id_to_kind = PyDict::new_bound(py);
    let kind_to_piece_id = PyDict::new_bound(py);
    for kind in [
        PieceKind::I,
        PieceKind::O,
        PieceKind::T,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::J,
        PieceKind::L,
    ] {
        let kind_name = piece_kind_as_str(kind);
        piece_id_to_kind.set_item(piece_id(kind), kind_name)?;
        kind_to_piece_id.set_item(kind_name, piece_id(kind))?;
    }
    m.add("PIECE_ID_TO_KIND", piece_id_to_kind)?;
    m.add("KIND_TO_PIECE_ID", kind_to_piece_id)?;
    Ok(())
}

fn add_piece_rotations(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let rotations = PyDict::new_bound(py);
    for kind in [
        PieceKind::I,
        PieceKind::O,
        PieceKind::T,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::J,
        PieceKind::L,
    ] {
        rotations.set_item(
            piece_kind_as_str(kind),
            rotation_states(kind)
                .iter()
                .map(|rotation| rotation.to_vec())
                .collect::<Vec<_>>(),
        )?;
    }
    m.add("PIECE_ROTATIONS", rotations)?;
    Ok(())
}

#[pymodule(name = "tetrisEngine_rs")]
pub fn tetris_engine_rs(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPiece>()?;
    m.add_class::<PyTetrisEngine>()?;

    m.add("BOARD_WIDTH", BOARD_WIDTH)?;
    m.add("BOARD_HEIGHT", BOARD_HEIGHT)?;
    m.add("VISIBLE_HEIGHT", VISIBLE_HEIGHT)?;
    m.add("HIDDEN_ROWS", HIDDEN_ROWS)?;
    m.add("GARBAGE_ID", GARBAGE_ID)?;
    m.add("SPAWN_X", SPAWN_X)?;
    m.add("SPAWN_Y", SPAWN_Y)?;
    m.add("ROTATION_NAMES", vec!["N", "E", "S", "W"])?;

    add_piece_mappings(py, m)?;
    add_piece_rotations(py, m)?;

    Ok(())
}
