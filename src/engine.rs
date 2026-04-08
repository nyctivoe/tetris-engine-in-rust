use crate::Board;
use crate::board::{
    board_index, cell_blocked, compute_blocks, is_position_valid as board_is_position_valid,
};
use crate::constants::{BOARD_HEIGHT, BOARD_WIDTH, HIDDEN_ROWS, SPAWN_X, SPAWN_Y};
use crate::piece::{Piece, PieceKind, piece_id, piece_kind_from_id};
use crate::rng::EngineRng;
use crate::rotation::{rotation_candidates, rotation_delta_from_i8};
use crate::scoring::{
    AttackStats, B2BMode, SpinMode, SpinResult, base_attack_for_clear, build_attack_stats,
    classify_clear, combo_after_clear, combo_attack_down as combo_attack_down_value,
    update_b2b_state,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueSnapshot {
    pub current: Option<PieceKind>,
    pub hold: Option<PieceKind>,
    pub next_ids: Vec<i8>,
    pub next_kinds: Vec<PieceKind>,
    pub piece_ids: Vec<i8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BagRemainderCounts {
    pub counts: [u8; 7],
    pub remaining: usize,
    pub bag_position: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndPhaseResult {
    pub lines_cleared: i32,
    pub spawned: bool,
    pub clutch_clear: bool,
    pub game_over: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlacementPayload {
    pub x: Option<i16>,
    pub y: Option<i16>,
    pub rotation: Option<u8>,
    pub last_was_rot: Option<bool>,
    pub last_rot_dir: Option<i8>,
    pub last_kick_idx: Option<i8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExecutePlacementResult {
    pub ok: bool,
    pub lines_cleared: i32,
    pub stats: Option<AttackStats>,
    pub end_phase: Option<EndPhaseResult>,
    pub attack: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacementSnapshot {
    pub x: i16,
    pub y: i16,
    pub rotation: u8,
    pub kind: PieceKind,
    pub last_was_rot: bool,
    pub last_rot_dir: Option<i8>,
    pub last_kick_idx: Option<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PostLockPrediction {
    pub board: Board,
    pub stats: AttackStats,
    pub blocks: [(i16, i16); 4],
    pub placement: PlacementSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KickTableSelector {
    I,
    O,
    Jlstz,
}

pub struct TetrisEngine {
    pub board: Board,
    pub current_piece: Option<Piece>,
    pub bag: Vec<i8>,
    pub hold: Option<i8>,
    pub hold_locked: bool,
    pub bag_size: usize,
    pub game_over: bool,
    pub game_over_reason: Option<String>,
    pub last_spawn_was_clutch: bool,
    pub spin_mode: SpinMode,
    pub b2b_mode: B2BMode,
    pub b2b_chain: i32,
    pub surge_charge: i32,
    pub last_clear_stats: Option<AttackStats>,
    pub last_end_phase: Option<EndPhaseResult>,
    pub combo: i32,
    pub combo_active: bool,
    pub pieces_placed: i32,
    pub total_lines_cleared: i32,
    pub total_attack_sent: i32,
    pub rng: EngineRng,
}

impl Default for TetrisEngine {
    fn default() -> Self {
        Self::with_seed(0)
    }
}

impl TetrisEngine {
    pub fn new(spin_mode: SpinMode, b2b_mode: B2BMode, rng: EngineRng) -> Self {
        let mut engine = Self {
            board: [0; BOARD_WIDTH * BOARD_HEIGHT],
            current_piece: None,
            bag: Vec::new(),
            hold: None,
            hold_locked: false,
            bag_size: 0,
            game_over: false,
            game_over_reason: None,
            last_spawn_was_clutch: false,
            spin_mode,
            b2b_mode,
            b2b_chain: 0,
            surge_charge: 0,
            last_clear_stats: None,
            last_end_phase: None,
            combo: 0,
            combo_active: false,
            pieces_placed: 0,
            total_lines_cleared: 0,
            total_attack_sent: 0,
            rng,
        };
        engine.generate_bag();
        engine
    }

    pub fn with_seed(seed: u64) -> Self {
        Self::with_seed_and_modes(seed, SpinMode::AllSpin, B2BMode::Surge)
    }

    pub fn with_seed_and_modes(seed: u64, spin_mode: SpinMode, b2b_mode: B2BMode) -> Self {
        Self::new(spin_mode, b2b_mode, EngineRng::seeded(seed))
    }

    pub fn reset(&mut self) {
        self.board = [0; BOARD_WIDTH * BOARD_HEIGHT];
        self.current_piece = None;
        self.bag.clear();
        self.hold = None;
        self.hold_locked = false;
        self.bag_size = 0;
        self.game_over = false;
        self.game_over_reason = None;
        self.last_spawn_was_clutch = false;
        self.b2b_chain = 0;
        self.surge_charge = 0;
        self.last_clear_stats = None;
        self.last_end_phase = None;
        self.combo = 0;
        self.combo_active = false;
        self.pieces_placed = 0;
        self.total_lines_cleared = 0;
        self.total_attack_sent = 0;
        self.generate_bag();
    }

    pub fn spawn_position_for(&self, _kind: Option<PieceKind>) -> (i16, i16) {
        (SPAWN_X, SPAWN_Y)
    }

    pub fn generate_bag(&mut self) -> &[i8] {
        while self.bag.len() <= 14 {
            let mut new_bag = [1, 2, 3, 4, 5, 6, 7];
            self.rng.shuffle_bag(&mut new_bag);
            self.bag.extend_from_slice(&new_bag);
        }
        self.bag_size = self.bag.len();
        self.bag.as_slice()
    }

    pub fn pop_next_piece_id(&mut self) -> i8 {
        self.generate_bag();
        let piece_id = self.bag.remove(0);
        self.bag_size = self.bag.len();
        if self.bag_size <= 14 {
            self.generate_bag();
        }
        piece_id
    }

    pub fn spawn_piece(
        &mut self,
        kind: PieceKind,
        position: Option<(i16, i16)>,
        rotation: u8,
    ) -> Piece {
        let position = position.unwrap_or_else(|| self.spawn_position_for(Some(kind)));
        let piece = Piece::new(kind, rotation, position);
        self.current_piece = Some(piece);
        self.hold_locked = false;
        piece
    }

    pub fn spawn_next(&mut self, allow_clutch: bool) -> bool {
        if self.game_over {
            return false;
        }

        let next_piece_id = self.pop_next_piece_id();
        self.spawn_specific_piece_id(next_piece_id, allow_clutch)
    }

    pub fn spawn_specific_piece_id(&mut self, piece_id_value: i8, allow_clutch: bool) -> bool {
        let kind = piece_kind_from_id(piece_id_value).expect("piece_id must be in 1..=7");
        let spawn_pos = self.spawn_position_for(Some(kind));
        let piece = Piece::new(kind, 0, spawn_pos);

        self.last_spawn_was_clutch = false;

        if self.is_position_valid_for_piece(&piece, Some(spawn_pos), Some(0)) {
            self.current_piece = Some(piece);
            self.hold_locked = false;
            return true;
        }

        if allow_clutch {
            if let Some(clutch_pos) = self.find_clutch_spawn(&piece, spawn_pos) {
                let mut clutch_piece = piece;
                clutch_piece.position = clutch_pos;
                self.current_piece = Some(clutch_piece);
                self.last_spawn_was_clutch = true;
                self.hold_locked = false;
                return true;
            }
        }

        self.current_piece = None;
        self.game_over = true;
        self.game_over_reason = Some("block_out".to_string());
        false
    }

    pub fn hold_current(&mut self) -> bool {
        if self.game_over || self.hold_locked {
            return false;
        }

        let piece = match self.current_piece {
            Some(piece) => piece,
            None => return false,
        };

        let current_id = piece_id(piece.kind);
        let held_id = self.hold;
        self.hold = Some(current_id);
        self.current_piece = None;

        let spawned = match held_id {
            Some(held_id) => self.spawn_specific_piece_id(held_id, false),
            None => self.spawn_next(false),
        };

        if spawned {
            self.hold_locked = true;
        }

        spawned
    }

    pub fn get_queue_snapshot(&self, next_slots: usize) -> QueueSnapshot {
        let current = self.current_piece.map(|piece| piece.kind);
        let hold = self
            .hold
            .map(|id| piece_kind_from_id(id).expect("hold id must be in 1..=7"));
        let next_ids = self
            .bag
            .iter()
            .copied()
            .take(next_slots)
            .collect::<Vec<_>>();
        let next_kinds = next_ids
            .iter()
            .map(|&id| piece_kind_from_id(id).expect("engine bag contains valid piece ids"))
            .collect::<Vec<_>>();

        let mut piece_ids = Vec::with_capacity(2 + next_slots);
        piece_ids.push(current.map(piece_id).unwrap_or(0));
        piece_ids.push(hold.map(piece_id).unwrap_or(0));
        piece_ids.extend(next_ids.iter().copied());
        while piece_ids.len() < 2 + next_slots {
            piece_ids.push(0);
        }

        QueueSnapshot {
            current,
            hold,
            next_ids,
            next_kinds,
            piece_ids,
        }
    }

    pub fn get_bag_remainder_counts(&self) -> BagRemainderCounts {
        let mut counts = [0_u8; 7];
        let mut remaining = if self.bag.is_empty() {
            0
        } else {
            self.bag.len() % 7
        };

        if self.current_piece.is_none() && remaining == 0 {
            remaining = usize::min(7, self.bag.len());
        }

        for &id in self.bag.iter().take(remaining) {
            if let Some(kind) = piece_kind_from_id(id) {
                counts[kind_index(kind)] += 1;
            }
        }

        let bag_position = if self.current_piece.is_none() {
            0
        } else {
            7usize.saturating_sub(remaining)
        };

        BagRemainderCounts {
            counts,
            remaining,
            bag_position,
        }
    }

    pub fn end_phase(&mut self, cleared_lines: i32) -> EndPhaseResult {
        let mut result = EndPhaseResult {
            lines_cleared: cleared_lines,
            spawned: false,
            clutch_clear: false,
            game_over: self.game_over,
            reason: self.game_over_reason.clone(),
        };

        if self.game_over {
            return result;
        }

        let spawned = self.spawn_next(cleared_lines > 0);
        result.spawned = spawned;
        result.clutch_clear = self.last_spawn_was_clutch;
        result.game_over = self.game_over;
        result.reason = self.game_over_reason.clone();
        result
    }

    pub fn piece_blocks(
        &self,
        piece: &Piece,
        position: Option<(i16, i16)>,
        rotation: Option<u8>,
    ) -> [(i16, i16); 4] {
        compute_blocks(piece, position, rotation)
    }

    pub fn is_position_valid(
        &self,
        piece: &Piece,
        position: Option<(i16, i16)>,
        rotation: Option<u8>,
    ) -> bool {
        board_is_position_valid(&self.board, piece, position, rotation)
    }

    pub fn rotate_piece(&self, piece: &mut Piece, delta: i8) -> bool {
        let Some(delta) = rotation_delta_from_i8(delta) else {
            return false;
        };

        let old_state = piece.rotation % 4;
        let new_state = ((i16::from(old_state) + i16::from(delta)).rem_euclid(4)) as u8;
        piece.last_action_was_rotation = false;

        if delta.abs() != 2 && matches!(Self::kick_table_for(piece.kind), KickTableSelector::O) {
            piece.rotation = new_state;
            return true;
        }

        for (kick_idx, kick_x, kick_y) in
            rotation_candidates(piece.kind, old_state, new_state, delta)
        {
            if self.try_rotate_piece(piece, new_state, delta, kick_idx, kick_x, kick_y) {
                return true;
            }
        }

        false
    }

    pub fn rotate_current(&mut self, delta: i8) -> bool {
        let Some(mut piece) = self.current_piece.take() else {
            return false;
        };
        let rotated = self.rotate_piece(&mut piece, delta);
        self.current_piece = Some(piece);
        rotated
    }

    pub fn apply_placement(&mut self, placement: PlacementPayload) -> bool {
        let Some(mut piece) = self.current_piece.take() else {
            return false;
        };

        let x = placement.x.unwrap_or(piece.position.0);
        let y = placement.y.unwrap_or(piece.position.1);
        let rotation = placement.rotation.unwrap_or(piece.rotation) % 4;

        piece.position = (x, y);
        piece.rotation = rotation;
        piece.last_action_was_rotation = placement.last_was_rot.unwrap_or(false);
        piece.last_rotation_dir = match placement.last_rot_dir {
            Some(0) | None => None,
            Some(value) => Some(value),
        };
        piece.last_kick_index = match placement.last_kick_idx {
            Some(value) if value >= 0 => Some(value as u8),
            _ => None,
        };

        let valid = self.is_position_valid(&piece, Some(piece.position), Some(piece.rotation));
        self.current_piece = Some(piece);
        valid
    }

    pub fn execute_placement(
        &mut self,
        placement: PlacementPayload,
        run_end_phase: bool,
    ) -> ExecutePlacementResult {
        if !self.apply_placement(placement) {
            self.game_over = true;
            self.game_over_reason = Some("invalid_placement".to_string());
            return ExecutePlacementResult {
                ok: false,
                lines_cleared: 0,
                stats: None,
                end_phase: None,
                attack: 0,
            };
        }

        let cleared = self.lock_piece(None, run_end_phase, None);
        ExecutePlacementResult {
            ok: true,
            lines_cleared: cleared,
            stats: self.last_clear_stats.clone(),
            end_phase: self.last_end_phase.clone(),
            attack: self
                .last_clear_stats
                .as_ref()
                .map(|stats| stats.attack)
                .unwrap_or(0),
        }
    }

    pub fn combo_attack_down(&self, base_attack: i32, combo: Option<i32>) -> i32 {
        combo_attack_down_value(base_attack, combo.unwrap_or(self.combo))
    }

    pub fn compute_attack_for_clear(
        &self,
        cleared_lines: i32,
        spin_result: Option<SpinResult>,
        board_after_clear: &Board,
        combo: Option<i32>,
        combo_active: Option<bool>,
        b2b_chain: Option<i32>,
        surge_charge: Option<i32>,
        base_attack: Option<i32>,
    ) -> AttackStats {
        let combo = combo.unwrap_or(self.combo);
        let combo_active = combo_active.unwrap_or(self.combo_active);
        let b2b_chain = b2b_chain.unwrap_or(self.b2b_chain);
        let surge_charge = surge_charge.unwrap_or(self.surge_charge);

        let (computed_base_attack, perfect_clear) =
            base_attack_for_clear(cleared_lines, spin_result.as_ref(), board_after_clear);
        let classification = classify_clear(cleared_lines, spin_result.as_ref(), perfect_clear);
        let base_attack = base_attack.unwrap_or(computed_base_attack);

        let next_combo = combo_after_clear(cleared_lines, combo, combo_active);
        let b2b_update = update_b2b_state(
            self.b2b_mode,
            cleared_lines,
            classification.is_difficult,
            b2b_chain,
            surge_charge,
        );
        let combo_attack = combo_attack_down_value(base_attack, next_combo.combo);
        let combo_bonus = combo_attack - base_attack;
        let combo_multiplier = if base_attack > 0 {
            Some(1.0 + 0.25 * f64::from(next_combo.combo))
        } else {
            None
        };
        let attack_total = combo_attack + b2b_update.b2b_bonus + b2b_update.surge_send;

        build_attack_stats(
            self.b2b_mode,
            classification,
            perfect_clear,
            next_combo.combo,
            next_combo.combo_active,
            b2b_update.b2b_chain,
            b2b_update.b2b_bonus,
            b2b_update.surge_charge,
            b2b_update.surge_send,
            base_attack,
            combo_attack,
            combo_bonus,
            combo_multiplier,
            attack_total,
        )
    }

    pub fn detect_spin(&self, piece: &Piece) -> Option<SpinResult> {
        if !piece.last_action_was_rotation {
            return None;
        }

        match piece.kind {
            PieceKind::T => self.detect_t_spin(piece),
            PieceKind::J | PieceKind::L | PieceKind::S | PieceKind::Z | PieceKind::I
                if self.spin_mode == SpinMode::AllSpin =>
            {
                self.detect_all_spin(piece)
            }
            _ => None,
        }
    }

    pub fn predict_post_lock_stats(
        &self,
        piece: &Piece,
        base_attack: Option<i32>,
    ) -> PostLockPrediction {
        let (board_after, stats) = self.simulate_lock(piece, None, None, None, base_attack, None);

        PostLockPrediction {
            board: board_after,
            stats,
            blocks: self.piece_blocks(piece, None, None),
            placement: PlacementSnapshot {
                x: piece.position.0,
                y: piece.position.1,
                rotation: piece.rotation,
                kind: piece.kind,
                last_was_rot: piece.last_action_was_rotation,
                last_rot_dir: piece.last_rotation_dir,
                last_kick_idx: piece.last_kick_index,
            },
        }
    }

    pub fn lock_piece(
        &mut self,
        piece: Option<Piece>,
        run_end_phase: bool,
        base_attack: Option<i32>,
    ) -> i32 {
        let piece = match piece {
            Some(piece) => {
                self.current_piece = None;
                piece
            }
            None => match self.current_piece.take() {
                Some(piece) => piece,
                None => return 0,
            },
        };

        let spin_result = self.detect_spin(&piece);
        let blocks = self.piece_blocks(&piece, None, None);
        let current_piece_id = piece_id(piece.kind);
        for (x, y) in blocks {
            let index = board_index(x, y).expect("locked piece blocks must be in bounds");
            self.board[index] = current_piece_id;
        }

        let locked_in_hidden = blocks.iter().all(|&(_, y)| y < HIDDEN_ROWS as i16);
        let cleared = self.clear_lines();
        let stats = self.compute_attack_for_clear(
            cleared,
            spin_result.clone(),
            &self.board,
            Some(self.combo),
            Some(self.combo_active),
            Some(self.b2b_chain),
            Some(self.surge_charge),
            base_attack,
        );
        self.apply_lock_stats(
            cleared,
            self.augment_lock_stats(stats, spin_result.as_ref()),
        );

        if !self.game_over && locked_in_hidden && cleared == 0 {
            self.game_over = true;
            self.game_over_reason = Some("lock_out".to_string());
        }
        if run_end_phase {
            self.last_end_phase = Some(self.end_phase(cleared));
        }

        cleared
    }

    pub fn lock_and_spawn(&mut self, piece: Option<Piece>) -> (i32, EndPhaseResult) {
        let cleared = self.lock_piece(piece, false, None);
        let end_phase = self.end_phase(cleared);
        self.last_end_phase = Some(end_phase.clone());
        (cleared, end_phase)
    }

    pub fn clear_lines(&mut self) -> i32 {
        let (board, cleared) = Self::clear_lines_on_board(&self.board);
        self.board = board;
        cleared
    }

    pub(crate) fn simulate_lock(
        &self,
        piece: &Piece,
        b2b_chain: Option<i32>,
        combo: Option<i32>,
        combo_active: Option<bool>,
        base_attack: Option<i32>,
        surge_charge: Option<i32>,
    ) -> (Board, AttackStats) {
        let spin_result = self.detect_spin(piece);
        let (locked_board, _) = self.copy_board_with_piece_locked(piece);
        let (board_after_clear, cleared) = Self::clear_lines_on_board(&locked_board);
        let stats = self.compute_attack_for_clear(
            cleared,
            spin_result.clone(),
            &board_after_clear,
            combo,
            combo_active,
            b2b_chain,
            surge_charge,
            base_attack,
        );
        (
            board_after_clear,
            self.augment_lock_stats(stats, spin_result.as_ref()),
        )
    }

    fn find_clutch_spawn(&self, piece: &Piece, position: (i16, i16)) -> Option<(i16, i16)> {
        let (x, y) = position;
        for ny in (0..y).rev() {
            let candidate = (x, ny);
            if self.is_position_valid_for_piece(piece, Some(candidate), None) {
                return Some(candidate);
            }
        }
        None
    }

    fn is_position_valid_for_piece(
        &self,
        piece: &Piece,
        position: Option<(i16, i16)>,
        rotation: Option<u8>,
    ) -> bool {
        self.is_position_valid(piece, position, rotation)
    }

    fn kick_table_for(kind: PieceKind) -> KickTableSelector {
        match kind {
            PieceKind::I => KickTableSelector::I,
            PieceKind::O => KickTableSelector::O,
            PieceKind::T | PieceKind::S | PieceKind::Z | PieceKind::J | PieceKind::L => {
                KickTableSelector::Jlstz
            }
        }
    }

    fn try_rotate_piece(
        &self,
        piece: &mut Piece,
        new_state: u8,
        delta: i8,
        kick_idx: u8,
        kick_x: i8,
        kick_y: i8,
    ) -> bool {
        let candidate = (
            piece.position.0 + i16::from(kick_x),
            piece.position.1 - i16::from(kick_y),
        );

        if !self.is_position_valid(piece, Some(candidate), Some(new_state)) {
            return false;
        }

        piece.position = candidate;
        piece.rotation = new_state;
        piece.last_action_was_rotation = true;
        piece.last_rotation_dir = Some(delta);
        piece.last_kick_index = Some(kick_idx);
        true
    }

    fn copy_board_with_piece_locked(&self, piece: &Piece) -> (Board, [(i16, i16); 4]) {
        let mut board = self.board;
        let blocks = self.piece_blocks(piece, None, None);
        let piece_id_value = piece_id(piece.kind);
        for (x, y) in blocks {
            let index = board_index(x, y).expect("locked piece blocks must be in bounds");
            board[index] = piece_id_value;
        }
        (board, blocks)
    }

    fn augment_lock_stats(
        &self,
        mut stats: AttackStats,
        spin_result: Option<&SpinResult>,
    ) -> AttackStats {
        stats.t_spin = match spin_result {
            Some(result) if result.is_mini => "M",
            Some(_) => "F",
            None => "N",
        };
        stats.garbage_cleared = 0;
        stats.immediate_garbage = 0;
        stats
    }

    fn apply_lock_stats(&mut self, cleared: i32, stats: AttackStats) {
        self.last_clear_stats = Some(stats.clone());
        self.combo = stats.combo;
        self.combo_active = stats.combo_active;
        self.b2b_chain = stats.b2b_chain;
        self.surge_charge = stats.surge_charge;
        self.pieces_placed += 1;
        self.total_lines_cleared += cleared;
        self.total_attack_sent += stats.attack;
    }

    fn clear_lines_on_board(board: &Board) -> (Board, i32) {
        let mut cleared = 0;
        let mut compacted = [0; BOARD_WIDTH * BOARD_HEIGHT];
        let mut write_row = BOARD_HEIGHT;

        for read_row in (0..BOARD_HEIGHT).rev() {
            let start = read_row * BOARD_WIDTH;
            let end = start + BOARD_WIDTH;
            let is_full = board[start..end].iter().all(|&cell| cell != 0);
            if is_full {
                cleared += 1;
                continue;
            }

            write_row -= 1;
            let write_start = write_row * BOARD_WIDTH;
            compacted[write_start..write_start + BOARD_WIDTH].copy_from_slice(&board[start..end]);
        }

        (compacted, cleared)
    }

    fn is_piece_immobile(&self, piece: &Piece) -> bool {
        let (px, py) = piece.position;
        !self.is_position_valid(piece, Some((px - 1, py)), None)
            && !self.is_position_valid(piece, Some((px + 1, py)), None)
            && !self.is_position_valid(piece, Some((px, py - 1)), None)
    }

    fn detect_t_spin(&self, piece: &Piece) -> Option<SpinResult> {
        let corners_occupied = self.occupied_3x3_corners(piece);
        let rotation_dir = piece.last_rotation_dir.unwrap_or(0);
        let is_180 = rotation_dir.abs() == 2;
        let front_corners = self.count_t_front_corners(piece);

        let is_mini = if corners_occupied >= 3 {
            let is_full = is_180 || piece.last_kick_index == Some(4) || front_corners == 2;
            !is_full
        } else if self.is_piece_immobile(piece) {
            true
        } else {
            return None;
        };

        Some(SpinResult {
            piece: PieceKind::T,
            spin_type: "t-spin",
            is_mini,
            is_180,
            kick_index: piece.last_kick_index,
            rotation_dir: piece.last_rotation_dir,
            corners: Some(corners_occupied),
            front_corners: Some(front_corners),
            description: format!(
                "{}T-Spin{}",
                if is_180 { "180 " } else { "" },
                if is_mini { " Mini" } else { "" }
            ),
        })
    }

    fn detect_all_spin(&self, piece: &Piece) -> Option<SpinResult> {
        if !self.is_piece_immobile(piece) {
            return None;
        }

        Some(SpinResult {
            piece: piece.kind,
            spin_type: "spin",
            is_mini: true,
            is_180: piece.last_rotation_dir.unwrap_or(0).abs() == 2,
            kick_index: piece.last_kick_index,
            rotation_dir: piece.last_rotation_dir,
            corners: None,
            front_corners: None,
            description: format!("{:?}-Spin Mini", piece.kind),
        })
    }

    fn occupied_3x3_corners(&self, piece: &Piece) -> u8 {
        let (px, py) = piece.position;
        [(0_i16, 0_i16), (2, 0), (0, 2), (2, 2)]
            .into_iter()
            .filter(|(cx, cy)| cell_blocked(&self.board, px + cx, py + cy))
            .count() as u8
    }

    fn count_t_front_corners(&self, piece: &Piece) -> u8 {
        let (px, py) = piece.position;
        let corners = match piece.rotation % 4 {
            0 => [(0_i16, 0_i16), (2, 0)],
            1 => [(2_i16, 0_i16), (2, 2)],
            2 => [(0_i16, 2_i16), (2, 2)],
            _ => [(0_i16, 0_i16), (0, 2)],
        };

        corners
            .into_iter()
            .filter(|(cx, cy)| cell_blocked(&self.board, px + cx, py + cy))
            .count() as u8
    }
}

const fn kind_index(kind: PieceKind) -> usize {
    match kind {
        PieceKind::I => 0,
        PieceKind::O => 1,
        PieceKind::T => 2,
        PieceKind::S => 3,
        PieceKind::Z => 4,
        PieceKind::J => 5,
        PieceKind::L => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttackStats, B2BMode, BagRemainderCounts, EndPhaseResult, ExecutePlacementResult,
        PlacementPayload, PostLockPrediction, QueueSnapshot, SpinMode, TetrisEngine, kind_index,
    };
    use crate::board::{board_index, compute_blocks, is_position_valid as board_is_position_valid};
    use crate::constants::{BOARD_HEIGHT, BOARD_WIDTH, SPAWN_X, SPAWN_Y};
    use crate::piece::{Piece, PieceKind};
    use crate::rotation::rotation_candidates;
    use std::collections::BTreeSet;

    fn set_board_cell(engine: &mut TetrisEngine, x: i16, y: i16, value: i8) {
        let index = board_index(x, y).expect("test coordinates must be in bounds");
        engine.board[index] = value;
    }

    fn piece_block_set(
        engine: &TetrisEngine,
        piece: &Piece,
        position: Option<(i16, i16)>,
        rotation: Option<u8>,
    ) -> BTreeSet<(i16, i16)> {
        engine
            .piece_blocks(piece, position, rotation)
            .into_iter()
            .collect()
    }

    fn force_rotation_kick(
        engine: &mut TetrisEngine,
        kind: PieceKind,
        old_state: u8,
        delta: i8,
        target_kick_idx: usize,
    ) -> Piece {
        let piece = Piece::new(kind, old_state, (4, 20));
        let new_state = ((i16::from(old_state) + i16::from(delta)).rem_euclid(4)) as u8;
        let candidates = rotation_candidates(kind, old_state, new_state, delta);
        let (_, target_kick_x, target_kick_y) = candidates[target_kick_idx];
        let target_position = (
            piece.position.0 + i16::from(target_kick_x),
            piece.position.1 - i16::from(target_kick_y),
        );
        let target_blocks = piece_block_set(engine, &piece, Some(target_position), Some(new_state));

        for (_, kick_x, kick_y) in candidates.iter().copied().take(target_kick_idx) {
            let blocked_position = (
                piece.position.0 + i16::from(kick_x),
                piece.position.1 - i16::from(kick_y),
            );
            let blocked_blocks =
                piece_block_set(engine, &piece, Some(blocked_position), Some(new_state));
            let blocker = blocked_blocks
                .difference(&target_blocks)
                .next()
                .copied()
                .expect("forced kick scenario must produce a blocker");
            set_board_cell(engine, blocker.0, blocker.1, 9);
        }

        piece
    }

    fn make_piece_immobile(engine: &mut TetrisEngine, piece: &Piece) {
        let current_blocks = piece_block_set(engine, piece, None, None);
        for (dx, dy) in [(-1_i16, 0_i16), (1, 0), (0, -1)] {
            let moved_blocks = piece_block_set(
                engine,
                piece,
                Some((piece.position.0 + dx, piece.position.1 + dy)),
                None,
            );
            let blocker = moved_blocks
                .difference(&current_blocks)
                .next()
                .copied()
                .expect("immobile test must find blocker");
            set_board_cell(engine, blocker.0, blocker.1, 9);
        }
    }

    fn non_empty_board() -> crate::Board {
        let mut board = [0; BOARD_WIDTH * BOARD_HEIGHT];
        board[0] = 1;
        board
    }

    #[test]
    fn default_engine_starts_with_phase_four_defaults_and_prefilled_bag() {
        let engine = TetrisEngine::default();

        assert_eq!(engine.spin_mode, SpinMode::AllSpin);
        assert_eq!(engine.b2b_mode, B2BMode::Surge);
        assert_eq!(engine.board, [0; BOARD_WIDTH * BOARD_HEIGHT]);
        assert_eq!(engine.current_piece, None);
        assert_eq!(engine.last_clear_stats, None);
        assert_eq!(engine.last_end_phase, None);
        assert_eq!(engine.b2b_chain, 0);
        assert_eq!(engine.surge_charge, 0);
        assert_eq!(engine.combo, 0);
        assert!(!engine.combo_active);
        assert_eq!(engine.total_attack_sent, 0);
        assert!(engine.bag.len() > 14);
    }

    #[test]
    fn with_seed_and_modes_and_reset_preserve_modes_but_clear_runtime_state() {
        let mut engine = TetrisEngine::with_seed_and_modes(4, SpinMode::TOnly, B2BMode::Chaining);
        set_board_cell(&mut engine, 0, 0, 9);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));
        engine.hold = Some(1);
        engine.hold_locked = true;
        engine.game_over = true;
        engine.game_over_reason = Some("block_out".to_string());
        engine.last_spawn_was_clutch = true;
        engine.last_clear_stats = Some(AttackStats {
            attack: 6,
            b2b_bonus: 1,
            b2b_chain: 2,
            b2b_display: 1,
            b2b_mode: "chaining",
            base_attack: 5,
            breaks_b2b: false,
            combo: 0,
            combo_active: true,
            combo_attack: 5,
            combo_bonus: 0,
            combo_multiplier: Some(1.0),
            is_difficult: true,
            is_mini: false,
            is_spin: false,
            lines_cleared: 4,
            perfect_clear: true,
            qualifies_b2b: true,
            spin: None,
            spin_type: 0,
            surge_charge: 0,
            surge_segments: Vec::new(),
            surge_send: 0,
            t_spin: "N",
            garbage_cleared: 0,
            immediate_garbage: 0,
        });
        engine.last_end_phase = Some(EndPhaseResult {
            lines_cleared: 4,
            spawned: true,
            clutch_clear: false,
            game_over: false,
            reason: None,
        });
        engine.b2b_chain = 3;
        engine.surge_charge = 9;
        engine.combo = 4;
        engine.combo_active = true;
        engine.pieces_placed = 4;
        engine.total_lines_cleared = 6;
        engine.total_attack_sent = 11;

        engine.reset();

        assert_eq!(engine.spin_mode, SpinMode::TOnly);
        assert_eq!(engine.b2b_mode, B2BMode::Chaining);
        assert_eq!(engine.board, [0; BOARD_WIDTH * BOARD_HEIGHT]);
        assert_eq!(engine.current_piece, None);
        assert_eq!(engine.hold, None);
        assert!(!engine.hold_locked);
        assert!(!engine.game_over);
        assert_eq!(engine.game_over_reason, None);
        assert!(!engine.last_spawn_was_clutch);
        assert_eq!(engine.last_clear_stats, None);
        assert_eq!(engine.last_end_phase, None);
        assert_eq!(engine.b2b_chain, 0);
        assert_eq!(engine.surge_charge, 0);
        assert_eq!(engine.combo, 0);
        assert!(!engine.combo_active);
        assert_eq!(engine.pieces_placed, 0);
        assert_eq!(engine.total_lines_cleared, 0);
        assert_eq!(engine.total_attack_sent, 0);
        assert!(engine.bag.len() > 14);
    }

    #[test]
    fn spawn_position_and_first_bag_match_contract() {
        let engine = TetrisEngine::with_seed(1);
        let mut first_bag = engine.bag[..7].to_vec();
        first_bag.sort_unstable();

        assert_eq!(engine.spawn_position_for(None), (SPAWN_X, SPAWN_Y));
        assert_eq!(first_bag, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn pop_next_piece_id_replenishes_when_queue_runs_short() {
        let mut engine = TetrisEngine::with_seed(2);
        engine.bag = vec![1];
        engine.bag_size = 1;

        let first_piece = engine.pop_next_piece_id();

        assert_eq!(first_piece, 1);
        assert!(engine.bag_size > 14);
    }

    #[test]
    fn queue_snapshot_matches_python_shape_and_identity_fields() {
        let mut engine = TetrisEngine::with_seed(2);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));
        engine.hold = Some(1);

        let snapshot = engine.get_queue_snapshot(3);

        assert_eq!(snapshot.current, Some(PieceKind::T));
        assert_eq!(snapshot.hold, Some(PieceKind::I));
        assert_eq!(snapshot.next_ids.len(), 3);
        assert_eq!(snapshot.piece_ids.len(), 5);
    }

    #[test]
    fn hold_current_stashes_piece_and_spawns_next_from_bag() {
        let mut engine = TetrisEngine::with_seed(3);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));
        engine.bag = vec![1, 2, 3];
        engine.bag_size = engine.bag.len();

        assert!(engine.hold_current());
        assert_eq!(engine.hold, Some(3));
        assert_eq!(
            engine.current_piece.map(|piece| piece.kind),
            Some(PieceKind::I)
        );
        assert_eq!(
            engine.current_piece.map(|piece| piece.position),
            Some((SPAWN_X, SPAWN_Y))
        );
        assert!(engine.hold_locked);
    }

    #[test]
    fn hold_current_swaps_with_held_piece_and_is_single_use_per_turn() {
        let mut engine = TetrisEngine::with_seed(4);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));
        engine.hold = Some(1);

        assert!(engine.hold_current());
        assert_eq!(engine.hold, Some(3));
        assert_eq!(
            engine.current_piece.map(|piece| piece.kind),
            Some(PieceKind::I)
        );
        assert!(engine.hold_locked);

        let current_kind = engine.current_piece.map(|piece| piece.kind);
        let held_id = engine.hold;
        assert!(!engine.hold_current());
        assert_eq!(engine.current_piece.map(|piece| piece.kind), current_kind);
        assert_eq!(engine.hold, held_id);
    }

    #[test]
    fn successful_normal_spawn_is_not_marked_as_clutch() {
        let mut engine = TetrisEngine::with_seed(5);

        assert!(engine.spawn_specific_piece_id(1, false));
        assert_eq!(
            engine.current_piece.map(|piece| piece.kind),
            Some(PieceKind::I)
        );
        assert_eq!(
            engine.current_piece.map(|piece| piece.position),
            Some((SPAWN_X, SPAWN_Y))
        );
        assert!(!engine.last_spawn_was_clutch);
    }

    #[test]
    fn blocked_spawn_can_use_clutch_to_find_the_first_valid_higher_row() {
        let mut engine = TetrisEngine::with_seed(6);
        set_board_cell(&mut engine, 3, 19, 9);

        assert!(engine.spawn_specific_piece_id(1, true));
        assert_eq!(
            engine.current_piece.map(|piece| piece.kind),
            Some(PieceKind::I)
        );
        assert_eq!(
            engine.current_piece.map(|piece| piece.position),
            Some((3, 17))
        );
        assert!(engine.last_spawn_was_clutch);
    }

    #[test]
    fn blocked_spawn_without_any_clutch_path_sets_block_out() {
        let mut engine = TetrisEngine::with_seed(7);
        for y in 1..=19 {
            set_board_cell(&mut engine, 3, y, 9);
        }

        assert!(!engine.spawn_specific_piece_id(1, true));
        assert_eq!(engine.current_piece, None);
        assert!(engine.game_over);
        assert_eq!(engine.game_over_reason.as_deref(), Some("block_out"));
    }

    #[test]
    fn end_phase_uses_clutch_spawn_when_spawn_row_is_blocked() {
        let mut engine = TetrisEngine::with_seed(8);
        set_board_cell(&mut engine, 3, 19, 9);
        engine.bag = vec![1, 2, 3];
        engine.bag_size = engine.bag.len();

        let result = engine.end_phase(1);

        assert_eq!(
            result,
            EndPhaseResult {
                lines_cleared: 1,
                spawned: true,
                clutch_clear: true,
                game_over: false,
                reason: None,
            }
        );
        assert_eq!(
            engine.current_piece.map(|piece| piece.kind),
            Some(PieceKind::I)
        );
        assert_eq!(
            engine.current_piece.map(|piece| piece.position),
            Some((3, 17))
        );
    }

    #[test]
    fn successful_spawn_resets_hold_lock() {
        let mut engine = TetrisEngine::with_seed(9);
        engine.hold_locked = true;

        assert!(engine.spawn_specific_piece_id(1, false));
        assert!(!engine.hold_locked);
    }

    #[test]
    fn empty_bag_has_zero_remainder_counts() {
        let engine = TetrisEngine {
            board: [0; BOARD_WIDTH * BOARD_HEIGHT],
            current_piece: None,
            bag: Vec::new(),
            hold: None,
            hold_locked: false,
            bag_size: 0,
            game_over: false,
            game_over_reason: None,
            last_spawn_was_clutch: false,
            spin_mode: SpinMode::AllSpin,
            b2b_mode: B2BMode::Surge,
            b2b_chain: 0,
            surge_charge: 0,
            last_clear_stats: None,
            last_end_phase: None,
            combo: 0,
            combo_active: false,
            pieces_placed: 0,
            total_lines_cleared: 0,
            total_attack_sent: 0,
            rng: crate::rng::EngineRng::seeded(0),
        };

        assert_eq!(
            engine.get_bag_remainder_counts(),
            BagRemainderCounts {
                counts: [0; 7],
                remaining: 0,
                bag_position: 0,
            }
        );
    }

    #[test]
    fn full_bag_boundary_counts_use_one_full_bag_when_no_piece_is_active() {
        let mut engine = TetrisEngine::with_seed(10);
        engine.current_piece = None;
        engine.bag = vec![1, 2, 3, 4, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7];
        engine.bag_size = engine.bag.len();

        let remainder = engine.get_bag_remainder_counts();

        assert_eq!(
            remainder,
            BagRemainderCounts {
                counts: [1, 1, 1, 1, 1, 1, 1],
                remaining: 7,
                bag_position: 0,
            }
        );
    }

    #[test]
    fn active_piece_remainder_counts_track_current_bag_position() {
        let mut engine = TetrisEngine::with_seed(11);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));
        engine.bag = vec![1, 2, 3, 4, 5, 6, 7, 1, 2];
        engine.bag_size = engine.bag.len();

        let remainder = engine.get_bag_remainder_counts();

        assert_eq!(
            remainder,
            BagRemainderCounts {
                counts: [1, 1, 0, 0, 0, 0, 0],
                remaining: 2,
                bag_position: 5,
            }
        );
    }

    #[test]
    fn public_payload_types_are_constructible_with_expected_shapes() {
        let snapshot = QueueSnapshot {
            current: Some(PieceKind::I),
            hold: None,
            next_ids: vec![2, 3],
            next_kinds: vec![PieceKind::O, PieceKind::T],
            piece_ids: vec![1, 0, 2, 3],
        };
        let result = EndPhaseResult {
            lines_cleared: 0,
            spawned: true,
            clutch_clear: false,
            game_over: false,
            reason: None,
        };
        let placement = PlacementPayload {
            x: Some(1),
            y: Some(2),
            rotation: Some(3),
            last_was_rot: Some(true),
            last_rot_dir: Some(1),
            last_kick_idx: Some(0),
        };
        let post_lock = PostLockPrediction {
            board: [0; BOARD_WIDTH * BOARD_HEIGHT],
            stats: AttackStats {
                attack: 0,
                b2b_bonus: 0,
                b2b_chain: 0,
                b2b_display: 0,
                b2b_mode: "surge",
                base_attack: 0,
                breaks_b2b: false,
                combo: 0,
                combo_active: false,
                combo_attack: 0,
                combo_bonus: 0,
                combo_multiplier: None,
                is_difficult: false,
                is_mini: false,
                is_spin: false,
                lines_cleared: 0,
                perfect_clear: false,
                qualifies_b2b: false,
                spin: None,
                spin_type: 0,
                surge_charge: 0,
                surge_segments: Vec::new(),
                surge_send: 0,
                t_spin: "N",
                garbage_cleared: 0,
                immediate_garbage: 0,
            },
            blocks: [(0, 0); 4],
            placement: super::PlacementSnapshot {
                x: 1,
                y: 2,
                rotation: 3,
                kind: PieceKind::T,
                last_was_rot: true,
                last_rot_dir: Some(1),
                last_kick_idx: Some(0),
            },
        };
        let exec = ExecutePlacementResult {
            ok: true,
            lines_cleared: 0,
            stats: Some(post_lock.stats.clone()),
            end_phase: Some(result.clone()),
            attack: 0,
        };

        assert_eq!(snapshot.piece_ids.len(), 4);
        assert!(result.spawned);
        assert_eq!(placement.rotation, Some(3));
        assert_eq!(post_lock.stats.t_spin, "N");
        assert!(exec.ok);
        assert_eq!(kind_index(PieceKind::L), 6);
        assert_eq!(BOARD_HEIGHT, 40);
    }

    #[test]
    fn piece_blocks_wrapper_matches_helper() {
        let engine = TetrisEngine::with_seed(12);
        let piece = Piece::new(PieceKind::T, 0, (4, 10));

        assert_eq!(
            engine.piece_blocks(&piece, Some((7, 18)), Some(1)),
            compute_blocks(&piece, Some((7, 18)), Some(1))
        );
    }

    #[test]
    fn engine_position_validity_wrapper_matches_free_helper() {
        let mut engine = TetrisEngine::with_seed(13);
        let piece = Piece::new(PieceKind::T, 0, (3, 0));

        assert_eq!(
            engine.is_position_valid(&piece, Some((3, 0)), None),
            board_is_position_valid(&engine.board, &piece, Some((3, 0)), None)
        );

        set_board_cell(&mut engine, 4, 1, 9);
        assert_eq!(
            engine.is_position_valid(&piece, Some((3, 0)), None),
            board_is_position_valid(&engine.board, &piece, Some((3, 0)), None)
        );
        assert!(!engine.is_position_valid(&piece, Some((3, 0)), None));
    }

    #[test]
    fn rotate_piece_matches_python_parity_cases() {
        let engine = TetrisEngine::with_seed(14);
        let mut cases = [
            (
                Piece::new(PieceKind::T, 0, (0, 0)),
                1,
                (0, 0),
                1,
                true,
                Some(1),
                Some(0),
            ),
            (
                Piece::new(PieceKind::I, 0, (0, 0)),
                1,
                (1, 0),
                1,
                true,
                Some(1),
                Some(0),
            ),
            (
                Piece::new(PieceKind::O, 0, (4, 4)),
                1,
                (4, 4),
                1,
                false,
                None,
                None,
            ),
            (
                Piece::new(PieceKind::T, 0, (0, 0)),
                2,
                (0, 0),
                2,
                true,
                Some(2),
                Some(0),
            ),
            (
                Piece::new(PieceKind::T, 1, (0, 0)),
                2,
                (0, 0),
                3,
                true,
                Some(2),
                Some(0),
            ),
        ];

        for (
            piece,
            delta,
            expected_position,
            expected_rotation,
            expected_last_was_rotation,
            expected_last_dir,
            expected_kick_idx,
        ) in &mut cases
        {
            assert!(engine.rotate_piece(piece, *delta));
            assert_eq!(piece.position, *expected_position);
            assert_eq!(piece.rotation, *expected_rotation);
            assert_eq!(piece.last_action_was_rotation, *expected_last_was_rotation);
            assert_eq!(piece.last_rotation_dir, *expected_last_dir);
            assert_eq!(piece.last_kick_index, *expected_kick_idx);
        }
    }

    #[test]
    fn rotate_piece_can_reach_every_targeted_kick_candidate() {
        let scenarios = [
            (PieceKind::T, 0, 1, 0, (4, 20), 1),
            (PieceKind::T, 0, 1, 1, (3, 20), 1),
            (PieceKind::T, 0, 1, 2, (3, 19), 1),
            (PieceKind::T, 0, 1, 3, (4, 22), 1),
            (PieceKind::T, 0, 1, 4, (3, 22), 1),
            (PieceKind::I, 0, 1, 0, (5, 20), 1),
            (PieceKind::I, 0, 1, 1, (3, 20), 1),
            (PieceKind::I, 0, 1, 2, (6, 20), 1),
            (PieceKind::I, 0, 1, 3, (3, 21), 1),
            (PieceKind::I, 0, 1, 4, (6, 18), 1),
            (PieceKind::T, 0, 2, 0, (4, 20), 2),
            (PieceKind::T, 0, 2, 1, (4, 19), 2),
            (PieceKind::T, 0, 2, 2, (5, 19), 2),
            (PieceKind::T, 0, 2, 3, (3, 19), 2),
            (PieceKind::T, 0, 2, 4, (4, 21), 2),
            (PieceKind::T, 1, 2, 0, (4, 20), 3),
            (PieceKind::T, 1, 2, 1, (5, 20), 3),
            (PieceKind::T, 1, 2, 2, (5, 18), 3),
            (PieceKind::T, 1, 2, 3, (5, 19), 3),
            (PieceKind::T, 1, 2, 4, (4, 18), 3),
        ];

        for (kind, old_state, delta, target_kick_idx, expected_position, expected_rotation) in
            scenarios
        {
            let mut engine = TetrisEngine::with_seed(15);
            let mut piece =
                force_rotation_kick(&mut engine, kind, old_state, delta, target_kick_idx);

            assert!(engine.rotate_piece(&mut piece, delta));
            assert_eq!(piece.position, expected_position);
            assert_eq!(piece.rotation, expected_rotation);
            assert!(piece.last_action_was_rotation);
            assert_eq!(piece.last_kick_index, Some(target_kick_idx as u8));
        }
    }

    #[test]
    fn rotate_current_returns_false_without_active_piece() {
        let mut engine = TetrisEngine::with_seed(16);
        assert!(!engine.rotate_current(1));
    }

    #[test]
    fn rotate_current_mutates_active_piece() {
        let mut engine = TetrisEngine::with_seed(17);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (0, 0)));

        assert!(engine.rotate_current(1));
        let piece = engine.current_piece.expect("piece should remain active");
        assert_eq!(piece.position, (0, 0));
        assert_eq!(piece.rotation, 1);
        assert!(piece.last_action_was_rotation);
        assert_eq!(piece.last_rotation_dir, Some(1));
        assert_eq!(piece.last_kick_index, Some(0));
    }

    #[test]
    fn apply_placement_updates_piece_and_returns_true_for_valid_payload() {
        let mut engine = TetrisEngine::with_seed(18);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));

        let ok = engine.apply_placement(PlacementPayload {
            x: Some(3),
            y: Some(0),
            rotation: Some(1),
            last_was_rot: Some(true),
            last_rot_dir: Some(1),
            last_kick_idx: Some(0),
        });

        assert!(ok);
        let piece = engine.current_piece.expect("piece should still be present");
        assert_eq!(piece.position, (3, 0));
        assert_eq!(piece.rotation, 1);
        assert!(piece.last_action_was_rotation);
        assert_eq!(piece.last_rotation_dir, Some(1));
        assert_eq!(piece.last_kick_index, Some(0));
    }

    #[test]
    fn apply_placement_normalizes_rotation_modulo_four() {
        let mut engine = TetrisEngine::with_seed(19);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));

        assert!(engine.apply_placement(PlacementPayload {
            rotation: Some(5),
            ..PlacementPayload::default()
        }));
        assert_eq!(
            engine
                .current_piece
                .expect("piece should remain active")
                .rotation,
            1
        );
    }

    #[test]
    fn apply_placement_omitted_fields_keep_geometry_and_clear_rotation_metadata() {
        let mut engine = TetrisEngine::with_seed(20);
        let mut piece = Piece::new(PieceKind::T, 2, (3, 4));
        piece.last_action_was_rotation = true;
        piece.last_rotation_dir = Some(1);
        piece.last_kick_index = Some(3);
        engine.current_piece = Some(piece);

        assert!(engine.apply_placement(PlacementPayload {
            x: Some(5),
            ..PlacementPayload::default()
        }));

        let piece = engine.current_piece.expect("piece should remain active");
        assert_eq!(piece.position, (5, 4));
        assert_eq!(piece.rotation, 2);
        assert!(!piece.last_action_was_rotation);
        assert_eq!(piece.last_rotation_dir, None);
        assert_eq!(piece.last_kick_index, None);
    }

    #[test]
    fn apply_placement_invalid_result_returns_false_but_keeps_mutated_piece() {
        let mut engine = TetrisEngine::with_seed(21);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));

        assert!(!engine.apply_placement(PlacementPayload {
            x: Some(-10),
            y: Some(0),
            rotation: Some(0),
            ..PlacementPayload::default()
        }));

        let piece = engine.current_piece.expect("piece should still be present");
        assert_eq!(piece.position, (-10, 0));
        assert_eq!(piece.rotation, 0);
    }

    #[test]
    fn execute_placement_invalid_payload_matches_failure_shape() {
        let mut engine = TetrisEngine::with_seed(22);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));

        let failed = engine.execute_placement(
            PlacementPayload {
                x: Some(-10),
                y: Some(0),
                rotation: Some(0),
                ..PlacementPayload::default()
            },
            true,
        );

        assert_eq!(
            failed,
            ExecutePlacementResult {
                ok: false,
                lines_cleared: 0,
                stats: None,
                end_phase: None,
                attack: 0,
            }
        );
        assert_eq!(
            engine.game_over_reason.as_deref(),
            Some("invalid_placement")
        );
    }

    #[test]
    fn clear_lines_compacts_board_and_returns_cleared_count() {
        let mut engine = TetrisEngine::with_seed(23);
        for x in 0..BOARD_WIDTH as i16 {
            set_board_cell(&mut engine, x, 38, 8);
            set_board_cell(&mut engine, x, 39, 9);
        }
        set_board_cell(&mut engine, 1, 37, 4);
        set_board_cell(&mut engine, 3, 37, 5);

        let cleared = engine.clear_lines();

        assert_eq!(cleared, 2);
        assert_eq!(engine.board[board_index(1, 39).unwrap()], 4);
        assert_eq!(engine.board[board_index(3, 39).unwrap()], 5);
        for x in 0..BOARD_WIDTH as i16 {
            assert_eq!(engine.board[board_index(x, 0).unwrap()], 0);
            assert_eq!(engine.board[board_index(x, 1).unwrap()], 0);
        }
    }

    #[test]
    fn simulate_lock_is_non_mutating_and_predict_matches() {
        let engine = TetrisEngine::with_seed(24);
        let piece = Piece::new(PieceKind::O, 0, (3, 37));
        let board_before = engine.board;

        let (simulated_board, simulated_stats) =
            engine.simulate_lock(&piece, None, None, None, None, None);
        let predicted = engine.predict_post_lock_stats(&piece, None);

        assert_eq!(engine.board, board_before);
        assert_eq!(engine.current_piece, None);
        assert_eq!(simulated_board, predicted.board);
        assert_eq!(simulated_stats, predicted.stats);
        assert_eq!(predicted.blocks, engine.piece_blocks(&piece, None, None));
    }

    #[test]
    fn lock_piece_mutates_board_and_counters() {
        let mut engine = TetrisEngine::with_seed(25);
        let piece = Piece::new(PieceKind::O, 0, (3, 37));

        let cleared = engine.lock_piece(Some(piece), false, None);

        assert_eq!(cleared, 0);
        assert_eq!(engine.current_piece, None);
        assert_eq!(engine.pieces_placed, 1);
        assert_eq!(engine.total_lines_cleared, 0);
        assert_eq!(engine.total_attack_sent, 0);
        assert_eq!(engine.board[board_index(4, 38).unwrap()], 2);
        assert_eq!(
            engine.last_clear_stats.as_ref().map(|stats| stats.t_spin),
            Some("N")
        );
    }

    #[test]
    fn lock_and_spawn_resets_hold_lock_for_next_piece() {
        let mut engine = TetrisEngine::with_seed(26);
        engine.current_piece = Some(Piece::new(PieceKind::O, 0, (3, 37)));
        engine.hold_locked = true;
        engine.bag = vec![1, 2, 3];
        engine.bag_size = engine.bag.len();

        let (cleared, end_phase) = engine.lock_and_spawn(None);

        assert_eq!(cleared, 0);
        assert!(end_phase.spawned);
        assert_eq!(
            engine.current_piece.map(|piece| piece.kind),
            Some(PieceKind::I)
        );
        assert!(!engine.hold_locked);
    }

    #[test]
    fn spin_detection_variants_match_reference_cases() {
        let mut engine = TetrisEngine::with_seed(27);

        let mut t_full = Piece::new(PieceKind::T, 0, (4, 4));
        t_full.last_action_was_rotation = true;
        t_full.last_rotation_dir = Some(1);
        t_full.last_kick_index = Some(0);
        for (x, y) in [(4, 4), (6, 4), (4, 6)] {
            set_board_cell(&mut engine, x, y, 9);
        }
        let full_spin = engine.detect_spin(&t_full).expect("full t-spin");
        assert!(!full_spin.is_mini);
        assert_eq!(full_spin.description, "T-Spin");

        engine.board = [0; BOARD_WIDTH * BOARD_HEIGHT];
        let mut t_mini = Piece::new(PieceKind::T, 0, (4, 4));
        t_mini.last_action_was_rotation = true;
        t_mini.last_rotation_dir = Some(1);
        t_mini.last_kick_index = Some(0);
        for (x, y) in [(4, 4), (4, 6), (6, 6)] {
            set_board_cell(&mut engine, x, y, 9);
        }
        let mini_spin = engine.detect_spin(&t_mini).expect("mini t-spin");
        assert!(mini_spin.is_mini);
        assert_eq!(mini_spin.description, "T-Spin Mini");

        let mut t_180 = Piece::new(PieceKind::T, 0, (4, 4));
        t_180.last_action_was_rotation = true;
        t_180.last_rotation_dir = Some(2);
        t_180.last_kick_index = Some(0);
        let spin_180 = engine.detect_spin(&t_180).expect("180 t-spin");
        assert!(spin_180.is_180);
        assert_eq!(spin_180.description, "180 T-Spin");

        engine.board = [0; BOARD_WIDTH * BOARD_HEIGHT];
        let mut non_t = Piece::new(PieceKind::J, 0, (4, 4));
        non_t.last_action_was_rotation = true;
        non_t.last_rotation_dir = Some(1);
        non_t.last_kick_index = Some(0);
        for (x, y) in [(3, 4), (5, 4)] {
            set_board_cell(&mut engine, x, y, 9);
        }
        let all_spin = engine.detect_spin(&non_t).expect("all spin");
        assert_eq!(all_spin.description, "J-Spin Mini");

        let non_rotated = Piece::new(PieceKind::T, 0, (4, 4));
        assert_eq!(engine.detect_spin(&non_rotated), None);
    }

    #[test]
    fn default_spin_mode_is_all_spin() {
        let mut engine = TetrisEngine::with_seed(28);
        assert_eq!(engine.spin_mode, SpinMode::AllSpin);

        let mut non_t = Piece::new(PieceKind::J, 0, (4, 4));
        non_t.last_action_was_rotation = true;
        non_t.last_rotation_dir = Some(1);
        non_t.last_kick_index = Some(0);
        for (x, y) in [(3, 4), (5, 4)] {
            set_board_cell(&mut engine, x, y, 9);
        }

        let detected = engine.detect_spin(&non_t).expect("all-spin should detect");
        assert_eq!(detected.piece, PieceKind::J);
        assert_eq!(detected.description, "J-Spin Mini");
    }

    #[test]
    fn all_spin_detects_every_non_t_piece_kind() {
        for kind in [
            PieceKind::J,
            PieceKind::L,
            PieceKind::S,
            PieceKind::Z,
            PieceKind::I,
        ] {
            let mut engine = TetrisEngine::with_seed(29);
            let mut piece = Piece::new(kind, 0, (4, 10));
            piece.last_action_was_rotation = true;
            piece.last_rotation_dir = Some(1);
            piece.last_kick_index = Some(0);
            make_piece_immobile(&mut engine, &piece);

            let detected = engine
                .detect_spin(&piece)
                .expect("non-t all-spin should detect");
            assert_eq!(detected.piece, kind);
            assert_eq!(detected.spin_type, "spin");
            assert!(detected.is_mini);
            assert_eq!(detected.description, format!("{:?}-Spin Mini", kind));
        }
    }

    #[test]
    fn t_only_mode_disables_non_t_all_spin_detection() {
        let mut engine = TetrisEngine::with_seed_and_modes(30, SpinMode::TOnly, B2BMode::Surge);
        let mut piece = Piece::new(PieceKind::J, 0, (4, 10));
        piece.last_action_was_rotation = true;
        piece.last_rotation_dir = Some(1);
        piece.last_kick_index = Some(0);
        make_piece_immobile(&mut engine, &piece);

        assert_eq!(engine.detect_spin(&piece), None);
    }

    #[test]
    fn t_spin_immobile_fallback_counts_as_mini_without_three_corners() {
        let mut engine = TetrisEngine::with_seed(31);
        let mut piece = Piece::new(PieceKind::T, 0, (4, 10));
        piece.last_action_was_rotation = true;
        piece.last_rotation_dir = Some(1);
        piece.last_kick_index = Some(0);
        make_piece_immobile(&mut engine, &piece);

        let detected = engine
            .detect_spin(&piece)
            .expect("immobile t-spin fallback");

        assert_eq!(engine.occupied_3x3_corners(&piece), 2);
        assert_eq!(detected.spin_type, "t-spin");
        assert!(detected.is_mini);
        assert_eq!(detected.description, "T-Spin Mini");
    }

    #[test]
    fn kick_index_four_forces_full_t_spin() {
        let mut engine = TetrisEngine::with_seed(32);
        let mut piece = Piece::new(PieceKind::T, 0, (4, 10));
        piece.last_action_was_rotation = true;
        piece.last_rotation_dir = Some(1);
        piece.last_kick_index = Some(4);
        for (x, y) in [(4, 10), (4, 12), (6, 12)] {
            set_board_cell(&mut engine, x, y, 9);
        }

        let detected = engine.detect_spin(&piece).expect("kick 4 full t-spin");

        assert_eq!(detected.corners, Some(3));
        assert_eq!(detected.front_corners, Some(1));
        assert!(!detected.is_mini);
        assert_eq!(detected.description, "T-Spin");
    }

    #[test]
    fn compute_attack_for_clear_respects_b2b_chaining_mode() {
        let engine = TetrisEngine::with_seed_and_modes(33, SpinMode::AllSpin, B2BMode::Chaining);
        let board_after_clear = non_empty_board();

        let stats = engine.compute_attack_for_clear(
            4,
            None,
            &board_after_clear,
            Some(0),
            Some(false),
            Some(4),
            Some(12),
            None,
        );

        assert_eq!(stats.b2b_mode, "chaining");
        assert_eq!(stats.b2b_chain, 5);
        assert_eq!(stats.b2b_bonus, 2);
        assert_eq!(stats.surge_charge, 0);
        assert_eq!(stats.surge_send, 0);
        assert_eq!(stats.attack, 6);
    }

    #[test]
    fn compute_attack_for_clear_preserves_payload_values() {
        let engine = TetrisEngine::with_seed(34);
        let board_after_clear = non_empty_board();

        let stats = engine.compute_attack_for_clear(
            2,
            Some(crate::scoring::SpinResult {
                piece: PieceKind::T,
                spin_type: "t-spin",
                is_mini: false,
                is_180: false,
                kick_index: Some(0),
                rotation_dir: Some(1),
                corners: Some(3),
                front_corners: Some(2),
                description: "T-Spin".to_string(),
            }),
            &board_after_clear,
            Some(1),
            Some(true),
            Some(2),
            Some(0),
            None,
        );

        assert_eq!(stats.base_attack, 4);
        assert_eq!(stats.combo_attack, 6);
        assert_eq!(stats.combo_bonus, 2);
        assert_eq!(stats.combo_multiplier, Some(1.5));
        assert_eq!(stats.attack, 7);
        assert_eq!(stats.b2b_mode, "surge");
        assert_eq!(stats.spin_type, 2);
        assert!(stats.is_spin);
        assert!(stats.qualifies_b2b);
    }

    #[test]
    fn perfect_clear_is_b2b_qualifying_for_any_positive_clear_count() {
        let engine = TetrisEngine::with_seed(35);

        for cleared_lines in 1..=4 {
            let stats = engine.compute_attack_for_clear(
                cleared_lines,
                None,
                &[0; BOARD_WIDTH * BOARD_HEIGHT],
                None,
                None,
                Some(1),
                None,
                None,
            );

            assert!(stats.perfect_clear);
            assert!(stats.qualifies_b2b);
            assert_eq!(stats.base_attack, 5);
            assert_eq!(stats.b2b_chain, 2);
            assert_eq!(stats.b2b_bonus, 1);
            assert_eq!(stats.attack, 6);
        }
    }

    #[test]
    fn non_t_all_spin_clear_uses_normal_base_attack_and_keeps_b2b() {
        let engine = TetrisEngine::with_seed(36);
        let stats = engine.compute_attack_for_clear(
            2,
            Some(crate::scoring::SpinResult {
                piece: PieceKind::J,
                spin_type: "spin",
                is_mini: true,
                is_180: false,
                kick_index: Some(0),
                rotation_dir: Some(1),
                corners: None,
                front_corners: None,
                description: "J-Spin Mini".to_string(),
            }),
            &non_empty_board(),
            None,
            None,
            Some(2),
            None,
            None,
        );

        assert!(stats.qualifies_b2b);
        assert_eq!(stats.base_attack, 1);
        assert_eq!(stats.b2b_chain, 3);
        assert_eq!(stats.b2b_bonus, 1);
        assert_eq!(stats.attack, 2);
    }

    #[test]
    fn execute_placement_success_returns_real_stats() {
        let mut engine = TetrisEngine::with_seed(37);
        for y in [38_i16, 39_i16] {
            for x in 0..BOARD_WIDTH as i16 {
                if x != 4 && x != 5 {
                    set_board_cell(&mut engine, x, y, 9);
                }
            }
        }
        engine.current_piece = Some(Piece::new(PieceKind::O, 0, (3, 37)));
        engine.bag = vec![1, 2, 3];
        engine.bag_size = engine.bag.len();

        let result = engine.execute_placement(PlacementPayload::default(), true);

        assert!(result.ok);
        assert_eq!(result.lines_cleared, 2);
        assert_eq!(result.attack, 5);
        assert_eq!(engine.pieces_placed, 1);
        assert_eq!(engine.total_lines_cleared, 2);
        assert_eq!(engine.total_attack_sent, 5);
        assert_eq!(
            result.stats.as_ref().map(|stats| stats.base_attack),
            Some(5)
        );
        assert_eq!(result.stats.as_ref().map(|stats| stats.attack), Some(5));
        assert_eq!(result.stats.as_ref().map(|stats| stats.t_spin), Some("N"));
        assert!(engine.last_end_phase.is_some());
        assert!(result.end_phase.is_some());
        assert_eq!(
            engine.current_piece.map(|piece| piece.kind),
            Some(PieceKind::I)
        );
    }

    #[test]
    fn combo_attack_down_matches_reference_rounding() {
        let engine = TetrisEngine::with_seed(38);
        assert_eq!(engine.combo_attack_down(4, Some(2)), 6);
        assert_eq!(engine.combo_attack_down(0, Some(2)), 1);
        assert_eq!(engine.combo_attack_down(0, Some(1)), 0);
    }

    #[test]
    fn locking_piece_entirely_in_hidden_rows_sets_lock_out() {
        let mut engine = TetrisEngine::with_seed(39);
        engine.current_piece = Some(Piece::new(PieceKind::O, 0, (3, 0)));

        let result = engine.execute_placement(PlacementPayload::default(), false);

        assert!(result.ok);
        assert!(engine.game_over);
        assert_eq!(engine.game_over_reason.as_deref(), Some("lock_out"));
    }
}
