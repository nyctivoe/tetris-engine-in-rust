use crate::board::is_position_valid;
use crate::constants::ROTATION_NAMES;
use crate::engine::TetrisEngine;
use crate::piece::{Piece, PieceKind};
use crate::rotation::rotation_candidates;
use crate::{AttackStats, Board};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, VecDeque};
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BfsInputs {
    pub kind: PieceKind,
    pub start_x: i16,
    pub start_y: i16,
    pub start_rot: u8,
    pub piece_is_o: bool,
    pub last_rot_dir: Option<i8>,
    pub last_kick_idx: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PlacementRecord {
    Skip,
    Placed {
        x: i16,
        y: i16,
        r: &'static str,
        rotation: u8,
        kind: PieceKind,
        last_was_rot: bool,
        last_rot_dir: Option<i8>,
        last_kick_idx: Option<u8>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct BfsResult {
    pub board: Option<Board>,
    pub stats: Option<AttackStats>,
    pub placement: PlacementRecord,
    pub placements: Vec<PlacementRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SpinResultKey {
    piece: PieceKind,
    spin_type: &'static str,
    is_mini: bool,
    is_180: bool,
    kick_index: Option<u8>,
    rotation_dir: Option<i8>,
    corners: Option<u8>,
    front_corners: Option<u8>,
    description: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct BfsResultKey {
    board: Board,
    attack: i32,
    b2b_bonus: i32,
    b2b_chain: i32,
    b2b_display: i32,
    b2b_mode: &'static str,
    base_attack: i32,
    breaks_b2b: bool,
    combo: i32,
    combo_active: bool,
    combo_attack: i32,
    combo_bonus: i32,
    combo_multiplier_bits: Option<u64>,
    is_difficult: bool,
    is_mini: bool,
    is_spin: bool,
    lines_cleared: i32,
    perfect_clear: bool,
    qualifies_b2b: bool,
    spin: Option<SpinResultKey>,
    spin_type: i32,
    surge_charge: i32,
    surge_segments: Vec<i32>,
    surge_send: i32,
    t_spin: &'static str,
    garbage_cleared: i32,
    immediate_garbage: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SearchState {
    x: i16,
    y: i16,
    rotation: u8,
    last_was_rot: bool,
    last_rot_dir: Option<i8>,
    last_kick_idx: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalState {
    pub x: i16,
    pub y: i16,
    pub rotation: u8,
    pub last_was_rot: bool,
    pub last_rot_dir: Option<i8>,
    pub last_kick_idx: Option<u8>,
}

impl PlacementRecord {
    pub(crate) fn placed(
        kind: PieceKind,
        x: i16,
        y: i16,
        rotation: u8,
        last_was_rot: bool,
        last_rot_dir: Option<i8>,
        last_kick_idx: Option<u8>,
    ) -> Self {
        Self::Placed {
            x,
            y,
            r: ROTATION_NAMES[usize::from(rotation % 4)],
            rotation: rotation % 4,
            kind,
            last_was_rot,
            last_rot_dir,
            last_kick_idx,
        }
    }
}

impl Serialize for PlacementRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            PlacementRecord::Skip => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("skip", &true)?;
                map.end()
            }
            PlacementRecord::Placed {
                x,
                y,
                r,
                rotation,
                kind,
                last_was_rot,
                last_rot_dir,
                last_kick_idx,
            } => {
                let mut map = serializer.serialize_map(Some(8))?;
                map.serialize_entry("x", x)?;
                map.serialize_entry("y", y)?;
                map.serialize_entry("r", r)?;
                map.serialize_entry("rotation", rotation)?;
                map.serialize_entry("kind", kind)?;
                map.serialize_entry("last_was_rot", last_was_rot)?;
                map.serialize_entry("last_rot_dir", last_rot_dir)?;
                map.serialize_entry("last_kick_idx", last_kick_idx)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for PlacementRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Skip,
            X,
            Y,
            R,
            Rotation,
            Kind,
            LastWasRot,
            LastRotDir,
            LastKickIdx,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a placement record field")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "skip" => Ok(Field::Skip),
                            "x" => Ok(Field::X),
                            "y" => Ok(Field::Y),
                            "r" => Ok(Field::R),
                            "rotation" => Ok(Field::Rotation),
                            "kind" => Ok(Field::Kind),
                            "last_was_rot" => Ok(Field::LastWasRot),
                            "last_rot_dir" => Ok(Field::LastRotDir),
                            "last_kick_idx" => Ok(Field::LastKickIdx),
                            _ => Err(de::Error::unknown_field(
                                value,
                                &[
                                    "skip",
                                    "x",
                                    "y",
                                    "r",
                                    "rotation",
                                    "kind",
                                    "last_was_rot",
                                    "last_rot_dir",
                                    "last_kick_idx",
                                ],
                            )),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PlacementVisitor;

        impl<'de> Visitor<'de> for PlacementVisitor {
            type Value = PlacementRecord;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a BFS placement record")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut skip = None;
                let mut x = None;
                let mut y = None;
                let mut r: Option<String> = None;
                let mut rotation = None;
                let mut kind = None;
                let mut last_was_rot = None;
                let mut last_rot_dir = None;
                let mut last_kick_idx = None;

                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Skip => skip = Some(map.next_value::<bool>()?),
                        Field::X => x = Some(map.next_value()?),
                        Field::Y => y = Some(map.next_value()?),
                        Field::R => r = Some(map.next_value()?),
                        Field::Rotation => rotation = Some(map.next_value()?),
                        Field::Kind => kind = Some(map.next_value()?),
                        Field::LastWasRot => last_was_rot = Some(map.next_value()?),
                        Field::LastRotDir => last_rot_dir = Some(map.next_value()?),
                        Field::LastKickIdx => last_kick_idx = Some(map.next_value()?),
                    }
                }

                if skip.unwrap_or(false) {
                    return Ok(PlacementRecord::Skip);
                }

                let r_value = r.ok_or_else(|| de::Error::missing_field("r"))?;
                let r = match r_value.as_str() {
                    "N" => ROTATION_NAMES[0],
                    "E" => ROTATION_NAMES[1],
                    "S" => ROTATION_NAMES[2],
                    "W" => ROTATION_NAMES[3],
                    _ => return Err(de::Error::custom("invalid rotation name")),
                };

                Ok(PlacementRecord::Placed {
                    x: x.ok_or_else(|| de::Error::missing_field("x"))?,
                    y: y.ok_or_else(|| de::Error::missing_field("y"))?,
                    r,
                    rotation: rotation.ok_or_else(|| de::Error::missing_field("rotation"))?,
                    kind: kind.ok_or_else(|| de::Error::missing_field("kind"))?,
                    last_was_rot: last_was_rot
                        .ok_or_else(|| de::Error::missing_field("last_was_rot"))?,
                    last_rot_dir: last_rot_dir.unwrap_or(None),
                    last_kick_idx: last_kick_idx.unwrap_or(None),
                })
            }
        }

        deserializer.deserialize_map(PlacementVisitor)
    }
}

impl BfsResultKey {
    pub(crate) fn from_board_stats(board: &Board, stats: &AttackStats) -> Self {
        Self {
            board: *board,
            attack: stats.attack,
            b2b_bonus: stats.b2b_bonus,
            b2b_chain: stats.b2b_chain,
            b2b_display: stats.b2b_display,
            b2b_mode: stats.b2b_mode,
            base_attack: stats.base_attack,
            breaks_b2b: stats.breaks_b2b,
            combo: stats.combo,
            combo_active: stats.combo_active,
            combo_attack: stats.combo_attack,
            combo_bonus: stats.combo_bonus,
            combo_multiplier_bits: stats.combo_multiplier.map(f64::to_bits),
            is_difficult: stats.is_difficult,
            is_mini: stats.is_mini,
            is_spin: stats.is_spin,
            lines_cleared: stats.lines_cleared,
            perfect_clear: stats.perfect_clear,
            qualifies_b2b: stats.qualifies_b2b,
            spin: stats.spin.as_ref().map(|spin| SpinResultKey {
                piece: spin.piece,
                spin_type: spin.spin_type,
                is_mini: spin.is_mini,
                is_180: spin.is_180,
                kick_index: spin.kick_index,
                rotation_dir: spin.rotation_dir,
                corners: spin.corners,
                front_corners: spin.front_corners,
                description: spin.description.clone(),
            }),
            spin_type: stats.spin_type,
            surge_charge: stats.surge_charge,
            surge_segments: stats.surge_segments.clone(),
            surge_send: stats.surge_send,
            t_spin: stats.t_spin,
            garbage_cleared: stats.garbage_cleared,
            immediate_garbage: stats.immediate_garbage,
        }
    }
}

pub(crate) fn python_semantics_bfs(
    engine: &TetrisEngine,
    inputs: &BfsInputs,
    start_last_was_rot: bool,
    include_180: bool,
) -> Vec<TerminalState> {
    let mut results = Vec::new();
    let mut queue = VecDeque::from([SearchState {
        x: inputs.start_x,
        y: inputs.start_y,
        rotation: inputs.start_rot % 4,
        last_was_rot: start_last_was_rot,
        last_rot_dir: inputs.last_rot_dir,
        last_kick_idx: inputs.last_kick_idx,
    }]);
    let mut visited = vec![false; crate::BOARD_WIDTH * crate::BOARD_HEIGHT * 4];
    visited[visited_index(inputs.start_x, inputs.start_y, inputs.start_rot % 4)] = true;
    let rotation_actions: &[i8] = if include_180 { &[1, -1, 2] } else { &[1, -1] };
    let probe_piece = Piece::new(inputs.kind, inputs.start_rot, (inputs.start_x, inputs.start_y));

    while let Some(state) = queue.pop_front() {
        if !is_position_valid(
            &engine.board,
            &probe_piece,
            Some((state.x, state.y + 1)),
            Some(state.rotation),
        ) {
            results.push(TerminalState {
                x: state.x,
                y: state.y,
                rotation: state.rotation,
                last_was_rot: state.last_was_rot,
                last_rot_dir: state.last_rot_dir,
                last_kick_idx: state.last_kick_idx,
            });
        }

        for (dx, dy) in [(-1_i16, 0_i16), (1, 0), (0, 1)] {
            let nx = state.x + dx;
            let ny = state.y + dy;
            if !is_position_valid(
                &engine.board,
                &probe_piece,
                Some((nx, ny)),
                Some(state.rotation),
            ) {
                continue;
            }
            let index = visited_index(nx, ny, state.rotation);
            if visited[index] {
                continue;
            }
            visited[index] = true;
            queue.push_back(SearchState {
                x: nx,
                y: ny,
                rotation: state.rotation,
                last_was_rot: false,
                last_rot_dir: None,
                last_kick_idx: None,
            });
        }

        for &rot_dir in rotation_actions {
            let new_rotation =
                ((i16::from(state.rotation) + i16::from(rot_dir)).rem_euclid(4)) as u8;
            let mut success = false;
            let mut final_x = state.x;
            let mut final_y = state.y;
            let mut final_kick_idx = 0_u8;

            if rot_dir.abs() != 2 && inputs.piece_is_o {
                if is_position_valid(
                    &engine.board,
                    &probe_piece,
                    Some((state.x, state.y)),
                    Some(new_rotation),
                ) {
                    success = true;
                }
            } else {
                for (kick_idx, kick_x, kick_y) in
                    rotation_candidates(inputs.kind, state.rotation, new_rotation, rot_dir)
                {
                    let tx = state.x + i16::from(kick_x);
                    let ty = state.y - i16::from(kick_y);
                    if is_position_valid(
                        &engine.board,
                        &probe_piece,
                        Some((tx, ty)),
                        Some(new_rotation),
                    ) {
                        success = true;
                        final_x = tx;
                        final_y = ty;
                        final_kick_idx = kick_idx;
                        break;
                    }
                }
            }

            if !success {
                continue;
            }

            let index = visited_index(final_x, final_y, new_rotation);
            if visited[index] {
                continue;
            }
            visited[index] = true;
            queue.push_back(SearchState {
                x: final_x,
                y: final_y,
                rotation: new_rotation,
                last_was_rot: true,
                last_rot_dir: Some(rot_dir),
                last_kick_idx: Some(final_kick_idx),
            });
        }
    }

    results
}

impl TetrisEngine {
    pub fn bfs_inputs_for_piece(&self, piece: &Piece) -> BfsInputs {
        BfsInputs {
            kind: piece.kind,
            start_x: piece.position.0,
            start_y: piece.position.1,
            start_rot: piece.rotation % 4,
            piece_is_o: piece.kind == PieceKind::O,
            last_rot_dir: piece.last_rotation_dir,
            last_kick_idx: piece.last_kick_index,
        }
    }

    pub fn empty_bfs_results(&self, include_no_place: bool) -> Vec<BfsResult> {
        if !include_no_place {
            return Vec::new();
        }
        vec![BfsResult {
            board: Some(self.board),
            stats: None,
            placement: PlacementRecord::Skip,
            placements: Vec::new(),
        }]
    }

    pub fn bfs_result_from_state(
        &self,
        kind: PieceKind,
        px: i16,
        py: i16,
        pr: u8,
        last_was_rot: bool,
        last_dir: Option<i8>,
        last_kick: Option<u8>,
        base_attack: Option<i32>,
    ) -> BfsResult {
        let mut piece = Piece::new(kind, pr, (px, py));
        piece.last_action_was_rotation = last_was_rot;
        piece.last_rotation_dir = last_dir;
        piece.last_kick_index = last_kick;
        let (board, stats) = self.simulate_lock(&piece, None, None, None, base_attack, None);

        BfsResult {
            board: Some(board),
            stats: Some(stats),
            placement: PlacementRecord::placed(
                kind,
                px,
                py,
                pr,
                last_was_rot,
                last_dir,
                last_kick,
            ),
            placements: Vec::new(),
        }
    }

    pub fn bfs_all_placements(
        &self,
        piece: Option<&Piece>,
        include_180: bool,
        base_attack: Option<i32>,
        include_no_place: bool,
        dedupe_final: bool,
    ) -> Vec<BfsResult> {
        let Some(piece) = piece.or(self.current_piece.as_ref()) else {
            return self.empty_bfs_results(include_no_place);
        };

        let inputs = self.bfs_inputs_for_piece(piece);
        let mut results = self.empty_bfs_results(include_no_place);
        for terminal in python_semantics_bfs(self, &inputs, piece.last_action_was_rotation, include_180)
        {
            results.push(self.bfs_result_from_state(
                inputs.kind,
                terminal.x,
                terminal.y,
                terminal.rotation,
                terminal.last_was_rot,
                terminal.last_rot_dir,
                terminal.last_kick_idx,
                base_attack,
            ));
        }

        if dedupe_final {
            dedupe_bfs_results(results)
        } else {
            results
        }
    }
}

fn dedupe_bfs_results(results: Vec<BfsResult>) -> Vec<BfsResult> {
    let mut deduped = Vec::with_capacity(results.len());
    let mut seen = HashMap::<BfsResultKey, usize>::new();

    for mut result in results {
        let Some(board) = result.board else {
            deduped.push(result);
            continue;
        };
        let Some(stats) = result.stats.as_ref() else {
            deduped.push(result);
            continue;
        };
        if matches!(result.placement, PlacementRecord::Skip) {
            deduped.push(result);
            continue;
        }

        let key = BfsResultKey::from_board_stats(&board, stats);
        if let Some(&existing_idx) = seen.get(&key) {
            deduped[existing_idx]
                .placements
                .push(result.placement.clone());
            continue;
        }

        result.placements = vec![result.placement.clone()];
        seen.insert(key, deduped.len());
        deduped.push(result);
    }

    deduped
}

fn visited_index(x: i16, y: i16, rotation: u8) -> usize {
    let x = wrap_index(x, crate::BOARD_WIDTH);
    let y = wrap_index(y, crate::BOARD_HEIGHT);
    (y * crate::BOARD_WIDTH + x) * 4 + usize::from(rotation % 4)
}

fn wrap_index(value: i16, upper: usize) -> usize {
    if value < 0 {
        (upper as i16 + value) as usize
    } else {
        value as usize
    }
}

#[cfg(test)]
mod tests {
    use super::{BfsResultKey, PlacementRecord, python_semantics_bfs};
    use crate::board::board_index;
    use crate::{Piece, PieceKind, TetrisEngine};

    fn result_signature(result: &crate::BfsResult) -> (Option<[i8; 400]>, Option<i32>, PlacementRecord) {
        (
            result.board,
            result.stats.as_ref().map(|stats| stats.attack),
            result.placement.clone(),
        )
    }

    #[test]
    fn bfs_skip_toggle_and_dedupe_behavior() {
        let engine = TetrisEngine::default();
        let piece = Piece::new(PieceKind::I, 0, (3, 0));

        let with_skip = engine.bfs_all_placements(Some(&piece), true, None, true, true);
        let without_skip = engine.bfs_all_placements(Some(&piece), true, None, false, true);
        let raw = engine.bfs_all_placements(Some(&piece), true, None, false, false);

        assert_eq!(with_skip[0].placement, PlacementRecord::Skip);
        assert!(without_skip
            .iter()
            .all(|result| result.placement != PlacementRecord::Skip));
        assert_eq!(raw.len(), 34);
        assert_eq!(without_skip.len(), 17);
        assert!(without_skip
            .iter()
            .all(|result| result.placements.len() == 2));
    }

    #[test]
    fn bfs_single_backend_matches_raw_terminal_projection() {
        let engine = TetrisEngine::default();
        let piece = Piece::new(PieceKind::T, 0, (3, 0));
        let inputs = engine.bfs_inputs_for_piece(&piece);

        let terminal_states = python_semantics_bfs(&engine, &inputs, false, true);
        let projected = terminal_states
            .iter()
            .map(|terminal| {
                engine.bfs_result_from_state(
                    inputs.kind,
                    terminal.x,
                    terminal.y,
                    terminal.rotation,
                    terminal.last_was_rot,
                    terminal.last_rot_dir,
                    terminal.last_kick_idx,
                    None,
                )
            })
            .map(|result| result_signature(&result))
            .collect::<Vec<_>>();
        let actual = engine
            .bfs_all_placements(Some(&piece), true, None, false, false)
            .iter()
            .map(result_signature)
            .collect::<Vec<_>>();

        assert_eq!(actual, projected);
    }

    #[test]
    fn bfs_include_180_only_changes_180_metadata() {
        let engine = TetrisEngine::default();
        let piece = Piece::new(PieceKind::T, 0, (3, 0));

        let without_180 = engine.bfs_all_placements(Some(&piece), false, None, false, false);
        let with_180 = engine.bfs_all_placements(Some(&piece), true, None, false, false);

        assert!(without_180.iter().all(|result| match &result.placement {
            PlacementRecord::Placed { last_rot_dir, .. } => *last_rot_dir != Some(2),
            PlacementRecord::Skip => true,
        }));
        assert!(with_180.iter().any(|result| match &result.placement {
            PlacementRecord::Placed { last_rot_dir, .. } => *last_rot_dir == Some(2),
            PlacementRecord::Skip => false,
        }));
    }

    #[test]
    fn dedupe_key_stability_uses_board_and_full_stats_payload() {
        let engine = TetrisEngine::default();
        let result = engine.bfs_result_from_state(PieceKind::O, 3, 37, 0, false, None, None, None);
        let board = result.board.expect("placed result must include a board");
        let stats = result.stats.expect("placed result must include stats");

        let left = BfsResultKey::from_board_stats(&board, &stats);
        let right = BfsResultKey::from_board_stats(&board, &stats.clone());
        assert_eq!(left, right);

        let mut altered_stats = stats.clone();
        altered_stats.combo_bonus += 1;
        let altered = BfsResultKey::from_board_stats(&board, &altered_stats);
        assert_ne!(left, altered);
    }

    #[test]
    fn skip_result_never_gets_folded_into_board_based_dedupe() {
        let engine = TetrisEngine::default();
        let piece = Piece::new(PieceKind::O, 0, (3, 37));

        let results = engine.bfs_all_placements(Some(&piece), true, None, true, true);

        assert_eq!(results[0].placement, PlacementRecord::Skip);
        assert!(results[0].placements.is_empty());
    }

    #[test]
    fn placements_preserve_rotation_names() {
        let engine = TetrisEngine::default();
        let piece = Piece::new(PieceKind::T, 0, (3, 0));

        let results = engine.bfs_all_placements(Some(&piece), true, None, false, false);

        assert!(results.iter().all(|result| match &result.placement {
            PlacementRecord::Placed { r, rotation, .. } => {
                *r == crate::ROTATION_NAMES[usize::from(*rotation)]
            }
            PlacementRecord::Skip => false,
        }));
    }

    #[test]
    fn bfs_o_piece_non_180_rotation_stays_in_place() {
        let mut engine = TetrisEngine::default();
        for x in 0..crate::BOARD_WIDTH as i16 {
            let index = board_index(x, 7).expect("floor row index exists");
            engine.board[index] = 9;
        }
        let mut piece = Piece::new(PieceKind::O, 0, (4, 4));
        piece.last_action_was_rotation = false;

        let results = engine.bfs_all_placements(Some(&piece), false, None, false, false);

        assert!(results.iter().any(|result| match result.placement {
            PlacementRecord::Placed {
                x,
                y,
                rotation,
                last_rot_dir,
                ..
            } => x == 4 && y == 4 && rotation == 1 && last_rot_dir == Some(1),
            PlacementRecord::Skip => false,
        }));
    }

    #[test]
    fn bfs_terminal_states_can_be_locked_without_mutating_engine() {
        let mut engine = TetrisEngine::default();
        let index = board_index(0, 39).expect("board index exists");
        engine.board[index] = 9;
        let before = engine.board;

        let results = engine.bfs_all_placements(Some(&Piece::new(PieceKind::I, 0, (3, 0))), true, None, false, false);

        assert_eq!(engine.board, before);
        assert!(!results.is_empty());
    }
}
