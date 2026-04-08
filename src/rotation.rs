use std::sync::OnceLock;

use crate::piece::{
    I_OFFSETS, JLSTZ_OFFSETS, KICKS_180_EW, KICKS_180_NS, O_OFFSETS, PieceKind, piece_definition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RotationDirection {
    Cw,
    Ccw,
    Half,
}

impl RotationDirection {
    pub const fn delta(self) -> i8 {
        match self {
            Self::Cw => 1,
            Self::Ccw => -1,
            Self::Half => 2,
        }
    }
}

pub fn rotation_delta_from_i8(value: i8) -> Option<i8> {
    match value {
        1 | -1 => Some(value),
        2 | -2 => Some(2),
        _ => None,
    }
}

pub fn rotation_delta_from_str(value: &str) -> Option<i8> {
    match value {
        "CW" | "cw" => Some(1),
        "CCW" | "ccw" => Some(-1),
        "180" => Some(2),
        _ => None,
    }
}

type RotationTable = [[[(i8, i8); 4]; 4]; 7];

static ROTATION_TABLES: OnceLock<RotationTable> = OnceLock::new();

pub fn rotation_states(kind: PieceKind) -> &'static [[(i8, i8); 4]; 4] {
    &ROTATION_TABLES.get_or_init(build_rotation_tables)[kind.index()]
}

pub fn rotation_candidates(
    kind: PieceKind,
    old_state: u8,
    new_state: u8,
    delta: i8,
) -> Vec<(u8, i8, i8)> {
    let old_state = (old_state % 4) as usize;
    let new_state = (new_state % 4) as usize;

    if delta.abs() == 2 {
        let kicks = if old_state % 2 == 0 {
            &KICKS_180_NS
        } else {
            &KICKS_180_EW
        };

        return kicks
            .iter()
            .enumerate()
            .map(|(idx, (x, y))| (idx as u8, *x, *y))
            .collect();
    }

    if matches!(kind, PieceKind::O) {
        return O_OFFSETS[old_state]
            .iter()
            .enumerate()
            .map(|(idx, (x, y))| (idx as u8, *x, *y))
            .collect();
    }

    let offsets = match kind {
        PieceKind::I => &I_OFFSETS,
        PieceKind::T | PieceKind::S | PieceKind::Z | PieceKind::J | PieceKind::L => &JLSTZ_OFFSETS,
        PieceKind::O => unreachable!(),
    };

    let mut candidates = Vec::with_capacity(5);
    for kick_idx in 0..5 {
        let (ox, oy) = offsets[old_state][kick_idx];
        let (nx, ny) = offsets[new_state][kick_idx];
        candidates.push((kick_idx as u8, ox - nx, oy - ny));
    }
    candidates
}

fn build_rotation_tables() -> RotationTable {
    [
        build_piece_rotation_states(PieceKind::I),
        build_piece_rotation_states(PieceKind::O),
        build_piece_rotation_states(PieceKind::T),
        build_piece_rotation_states(PieceKind::S),
        build_piece_rotation_states(PieceKind::Z),
        build_piece_rotation_states(PieceKind::J),
        build_piece_rotation_states(PieceKind::L),
    ]
}

fn build_piece_rotation_states(kind: PieceKind) -> [[(i8, i8); 4]; 4] {
    let definition = piece_definition(kind);
    let mut rotations = [[(0, 0); 4]; 4];
    rotations[0] = definition.blocks;

    let mut current = definition.blocks;
    for state in rotations.iter_mut().skip(1) {
        current = rotate_coords(current, definition.size, 1);
        *state = current;
    }
    rotations
}

fn rotate_coords(coords: [(i8, i8); 4], size: i8, direction: i8) -> [(i8, i8); 4] {
    let center = f64::from(size - 1) / 2.0;
    let mut rotated = [(0, 0); 4];

    for (idx, (x, y)) in coords.into_iter().enumerate() {
        let dx = f64::from(x) - center;
        let dy = f64::from(y) - center;
        let (rx, ry) = match direction {
            1 => (-dy, dx),
            -1 => (dy, -dx),
            2 | -2 => (-dx, -dy),
            _ => panic!("invalid rotation direction"),
        };

        rotated[idx] = ((center + rx).round() as i8, (center + ry).round() as i8);
    }

    rotated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::PieceKind;

    #[test]
    fn rotation_delta_helpers_match_python_semantics() {
        assert_eq!(rotation_delta_from_i8(1), Some(1));
        assert_eq!(rotation_delta_from_i8(-1), Some(-1));
        assert_eq!(rotation_delta_from_i8(2), Some(2));
        assert_eq!(rotation_delta_from_i8(-2), Some(2));
        assert_eq!(rotation_delta_from_i8(0), None);

        assert_eq!(rotation_delta_from_str("CW"), Some(1));
        assert_eq!(rotation_delta_from_str("cw"), Some(1));
        assert_eq!(rotation_delta_from_str("CCW"), Some(-1));
        assert_eq!(rotation_delta_from_str("ccw"), Some(-1));
        assert_eq!(rotation_delta_from_str("180"), Some(2));
        assert_eq!(rotation_delta_from_str("half"), None);
    }

    #[test]
    fn generated_rotation_states_match_reference_geometry() {
        assert_eq!(
            *rotation_states(PieceKind::I),
            [
                [(0, 1), (1, 1), (2, 1), (3, 1)],
                [(2, 0), (2, 1), (2, 2), (2, 3)],
                [(3, 2), (2, 2), (1, 2), (0, 2)],
                [(1, 3), (1, 2), (1, 1), (1, 0)],
            ]
        );
        assert_eq!(
            *rotation_states(PieceKind::O),
            [
                [(1, 1), (2, 1), (1, 2), (2, 2)],
                [(2, 1), (2, 2), (1, 1), (1, 2)],
                [(2, 2), (1, 2), (2, 1), (1, 1)],
                [(1, 2), (1, 1), (2, 2), (2, 1)],
            ]
        );
        assert_eq!(
            *rotation_states(PieceKind::T),
            [
                [(1, 0), (0, 1), (1, 1), (2, 1)],
                [(2, 1), (1, 0), (1, 1), (1, 2)],
                [(1, 2), (2, 1), (1, 1), (0, 1)],
                [(0, 1), (1, 2), (1, 1), (1, 0)],
            ]
        );
        assert_eq!(
            *rotation_states(PieceKind::S),
            [
                [(1, 0), (2, 0), (0, 1), (1, 1)],
                [(2, 1), (2, 2), (1, 0), (1, 1)],
                [(1, 2), (0, 2), (2, 1), (1, 1)],
                [(0, 1), (0, 0), (1, 2), (1, 1)],
            ]
        );
        assert_eq!(
            *rotation_states(PieceKind::Z),
            [
                [(0, 0), (1, 0), (1, 1), (2, 1)],
                [(2, 0), (2, 1), (1, 1), (1, 2)],
                [(2, 2), (1, 2), (1, 1), (0, 1)],
                [(0, 2), (0, 1), (1, 1), (1, 0)],
            ]
        );
        assert_eq!(
            *rotation_states(PieceKind::J),
            [
                [(0, 0), (0, 1), (1, 1), (2, 1)],
                [(2, 0), (1, 0), (1, 1), (1, 2)],
                [(2, 2), (2, 1), (1, 1), (0, 1)],
                [(0, 2), (1, 2), (1, 1), (1, 0)],
            ]
        );
        assert_eq!(
            *rotation_states(PieceKind::L),
            [
                [(2, 0), (0, 1), (1, 1), (2, 1)],
                [(2, 2), (1, 0), (1, 1), (1, 2)],
                [(0, 2), (2, 1), (1, 1), (0, 1)],
                [(0, 0), (1, 2), (1, 1), (1, 0)],
            ]
        );
    }

    #[test]
    fn rotation_candidates_match_reference_expectations() {
        assert_eq!(
            rotation_candidates(PieceKind::T, 0, 1, 1),
            vec![(0, 0, 0), (1, -1, 0), (2, -1, 1), (3, 0, -2), (4, -1, -2)]
        );
        assert_eq!(
            rotation_candidates(PieceKind::T, 1, 0, -1),
            vec![(0, 0, 0), (1, 1, 0), (2, 1, -1), (3, 0, 2), (4, 1, 2)]
        );
        assert_eq!(
            rotation_candidates(PieceKind::I, 0, 1, 1),
            vec![(0, 1, 0), (1, -1, 0), (2, 2, 0), (3, -1, -1), (4, 2, 2)]
        );
        assert_eq!(
            rotation_candidates(PieceKind::I, 1, 0, -1),
            vec![(0, -1, 0), (1, 1, 0), (2, -2, 0), (3, 1, 1), (4, -2, -2)]
        );
        assert_eq!(
            rotation_candidates(PieceKind::T, 0, 2, 2),
            vec![(0, 0, 0), (1, 0, 1), (2, 1, 1), (3, -1, 1), (4, 0, -1)]
        );
        assert_eq!(
            rotation_candidates(PieceKind::T, 1, 3, 2),
            vec![(0, 0, 0), (1, 1, 0), (2, 1, 2), (3, 1, 1), (4, 0, 2)]
        );
    }

    #[test]
    fn one_eighty_candidates_switch_between_ns_and_ew_tables() {
        assert_eq!(
            rotation_candidates(PieceKind::T, 2, 0, 2),
            vec![(0, 0, 0), (1, 0, 1), (2, 1, 1), (3, -1, 1), (4, 0, -1)]
        );
        assert_eq!(
            rotation_candidates(PieceKind::T, 3, 1, 2),
            vec![(0, 0, 0), (1, 1, 0), (2, 1, 2), (3, 1, 1), (4, 0, 2)]
        );
    }

    #[test]
    fn o_piece_non_one_eighty_rotation_has_only_zero_offset_candidate() {
        assert_eq!(rotation_candidates(PieceKind::O, 0, 1, 1), vec![(0, 0, 0)]);
        assert_eq!(rotation_candidates(PieceKind::O, 1, 0, -1), vec![(0, 0, 0)]);
    }
}
