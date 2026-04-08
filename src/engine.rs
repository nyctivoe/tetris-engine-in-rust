use crate::Board;
use crate::board::is_position_valid;
use crate::constants::{BOARD_HEIGHT, BOARD_WIDTH, SPAWN_X, SPAWN_Y};
use crate::piece::{Piece, PieceKind, piece_id, piece_kind_from_id};
use crate::rng::EngineRng;

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
    pub rng: EngineRng,
}

impl Default for TetrisEngine {
    fn default() -> Self {
        Self::new(EngineRng::default())
    }
}

impl TetrisEngine {
    pub fn new(rng: EngineRng) -> Self {
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
            rng,
        };
        engine.generate_bag();
        engine
    }

    pub fn with_seed(seed: u64) -> Self {
        Self::new(EngineRng::seeded(seed))
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
        is_position_valid(&self.board, piece, position, rotation)
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
    use super::{BagRemainderCounts, EndPhaseResult, QueueSnapshot, TetrisEngine, kind_index};
    use crate::board::board_index;
    use crate::constants::{BOARD_HEIGHT, BOARD_WIDTH, SPAWN_X, SPAWN_Y};
    use crate::piece::{Piece, PieceKind};

    fn set_board_cell(engine: &mut TetrisEngine, x: i16, y: i16, value: i8) {
        let index = board_index(x, y).expect("test coordinates must be in bounds");
        engine.board[index] = value;
    }

    #[test]
    fn default_engine_starts_with_phase_two_defaults_and_prefilled_bag() {
        let engine = TetrisEngine::default();

        assert_eq!(engine.board, [0; BOARD_WIDTH * BOARD_HEIGHT]);
        assert_eq!(engine.current_piece, None);
        assert_eq!(engine.hold, None);
        assert!(!engine.hold_locked);
        assert!(!engine.game_over);
        assert_eq!(engine.game_over_reason, None);
        assert!(!engine.last_spawn_was_clutch);
        assert!(engine.bag.len() > 14);
        assert_eq!(engine.bag_size, engine.bag.len());
    }

    #[test]
    fn reset_restores_phase_two_defaults_and_regenerates_bag() {
        let mut engine = TetrisEngine::with_seed(4);
        set_board_cell(&mut engine, 0, 0, 9);
        engine.current_piece = Some(Piece::new(PieceKind::T, 0, (3, 0)));
        engine.hold = Some(1);
        engine.hold_locked = true;
        engine.bag = vec![1];
        engine.bag_size = 1;
        engine.game_over = true;
        engine.game_over_reason = Some("block_out".to_string());
        engine.last_spawn_was_clutch = true;

        engine.reset();

        assert_eq!(engine.board, [0; BOARD_WIDTH * BOARD_HEIGHT]);
        assert_eq!(engine.current_piece, None);
        assert_eq!(engine.hold, None);
        assert!(!engine.hold_locked);
        assert!(!engine.game_over);
        assert_eq!(engine.game_over_reason, None);
        assert!(!engine.last_spawn_was_clutch);
        assert!(engine.bag.len() > 14);
        assert_eq!(engine.bag_size, engine.bag.len());
    }

    #[test]
    fn spawn_position_and_first_bag_match_phase_two_contract() {
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
        assert_eq!(engine.current_piece.map(|piece| piece.rotation), Some(0));
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
        assert!(!engine.game_over);
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
        assert!(!engine.game_over);
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

        assert_eq!(snapshot.piece_ids.len(), 4);
        assert!(result.spawned);
        assert_eq!(kind_index(PieceKind::L), 6);
        assert_eq!(BOARD_HEIGHT, 40);
    }
}
