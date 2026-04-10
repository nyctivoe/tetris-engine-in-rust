use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash, Serialize)]
pub enum PieceKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl PieceKind {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::I => 0,
            Self::O => 1,
            Self::T => 2,
            Self::S => 3,
            Self::Z => 4,
            Self::J => 5,
            Self::L => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Piece {
    pub kind: PieceKind,
    pub rotation: u8,
    pub position: (i16, i16),
    pub last_action_was_rotation: bool,
    pub last_rotation_dir: Option<i8>,
    pub last_kick_index: Option<u8>,
}

impl Piece {
    pub fn new(kind: PieceKind, rotation: u8, position: (i16, i16)) -> Self {
        Self {
            kind,
            rotation: rotation % 4,
            position,
            last_action_was_rotation: false,
            last_rotation_dir: None,
            last_kick_index: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PieceDefinition {
    pub size: i8,
    pub blocks: [(i8, i8); 4],
}

pub(crate) const PIECE_DEFS: [PieceDefinition; 7] = [
    PieceDefinition {
        size: 4,
        blocks: [(0, 1), (1, 1), (2, 1), (3, 1)],
    },
    PieceDefinition {
        size: 4,
        blocks: [(1, 1), (2, 1), (1, 2), (2, 2)],
    },
    PieceDefinition {
        size: 3,
        blocks: [(1, 0), (0, 1), (1, 1), (2, 1)],
    },
    PieceDefinition {
        size: 3,
        blocks: [(1, 0), (2, 0), (0, 1), (1, 1)],
    },
    PieceDefinition {
        size: 3,
        blocks: [(0, 0), (1, 0), (1, 1), (2, 1)],
    },
    PieceDefinition {
        size: 3,
        blocks: [(0, 0), (0, 1), (1, 1), (2, 1)],
    },
    PieceDefinition {
        size: 3,
        blocks: [(2, 0), (0, 1), (1, 1), (2, 1)],
    },
];

pub(crate) const JLSTZ_OFFSETS: [[(i8, i8); 5]; 4] = [
    [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
    [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
];

pub(crate) const I_OFFSETS: [[(i8, i8); 5]; 4] = [
    [(0, 0), (-1, 0), (2, 0), (-1, 0), (2, 0)],
    [(0, 0), (1, 0), (1, 0), (1, 1), (1, -2)],
    [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, -1)],
    [(0, 0), (-1, 0), (-1, 0), (-1, 1), (-1, -2)],
];

pub(crate) const O_OFFSETS: [[(i8, i8); 1]; 4] = [[(0, 0)], [(0, 0)], [(0, 0)], [(0, 0)]];

pub(crate) const KICKS_180_NS: [(i8, i8); 5] = [(0, 0), (0, 1), (1, 1), (-1, 1), (0, -1)];
pub(crate) const KICKS_180_EW: [(i8, i8); 5] = [(0, 0), (1, 0), (1, 2), (1, 1), (0, 2)];

pub fn piece_kind_from_id(id: i8) -> Option<PieceKind> {
    match id {
        1 => Some(PieceKind::I),
        2 => Some(PieceKind::O),
        3 => Some(PieceKind::T),
        4 => Some(PieceKind::S),
        5 => Some(PieceKind::Z),
        6 => Some(PieceKind::J),
        7 => Some(PieceKind::L),
        _ => None,
    }
}

pub fn piece_id(kind: PieceKind) -> i8 {
    match kind {
        PieceKind::I => 1,
        PieceKind::O => 2,
        PieceKind::T => 3,
        PieceKind::S => 4,
        PieceKind::Z => 5,
        PieceKind::J => 6,
        PieceKind::L => 7,
    }
}

pub(crate) const fn piece_definition(kind: PieceKind) -> PieceDefinition {
    PIECE_DEFS[kind.index()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KINDS: [PieceKind; 7] = [
        PieceKind::I,
        PieceKind::O,
        PieceKind::T,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::J,
        PieceKind::L,
    ];

    #[test]
    fn piece_id_round_trip_matches_reference_mapping() {
        for kind in ALL_KINDS {
            let id = piece_id(kind);
            assert_eq!(piece_kind_from_id(id), Some(kind));
        }
        assert_eq!(piece_kind_from_id(0), None);
        assert_eq!(piece_kind_from_id(8), None);
        assert_eq!(piece_kind_from_id(-1), None);
    }

    #[test]
    fn piece_new_normalizes_rotation_modulo_four() {
        let piece = Piece::new(PieceKind::T, 5, (3, 18));
        assert_eq!(piece.rotation, 1);
        assert_eq!(piece.position, (3, 18));
        assert!(!piece.last_action_was_rotation);
        assert_eq!(piece.last_rotation_dir, None);
        assert_eq!(piece.last_kick_index, None);
    }

    #[test]
    fn piece_tables_match_reference_values() {
        assert_eq!(
            piece_definition(PieceKind::I).blocks,
            [(0, 1), (1, 1), (2, 1), (3, 1)]
        );
        assert_eq!(
            piece_definition(PieceKind::O).blocks,
            [(1, 1), (2, 1), (1, 2), (2, 2)]
        );
        assert_eq!(JLSTZ_OFFSETS[1], [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)]);
        assert_eq!(I_OFFSETS[2], [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, -1)]);
        assert_eq!(O_OFFSETS[0], [(0, 0)]);
        assert_eq!(KICKS_180_NS, [(0, 0), (0, 1), (1, 1), (-1, 1), (0, -1)]);
        assert_eq!(KICKS_180_EW, [(0, 0), (1, 0), (1, 2), (1, 1), (0, 2)]);
    }
}
