use crate::constants::{BOARD_HEIGHT, BOARD_WIDTH};
use crate::piece::Piece;
use crate::rotation::rotation_states;

pub type Board = [i8; BOARD_WIDTH * BOARD_HEIGHT];

pub fn board_index(x: i16, y: i16) -> Option<usize> {
    if x < 0 || x >= BOARD_WIDTH as i16 || y < 0 || y >= BOARD_HEIGHT as i16 {
        return None;
    }

    Some(y as usize * BOARD_WIDTH + x as usize)
}

pub fn compute_blocks(
    piece: &Piece,
    position: Option<(i16, i16)>,
    rotation: Option<u8>,
) -> [(i16, i16); 4] {
    let position = position.unwrap_or(piece.position);
    let rotation = rotation.unwrap_or(piece.rotation) % 4;
    let local_blocks = &rotation_states(piece.kind)[rotation as usize];

    let mut blocks = [(0, 0); 4];
    for (idx, (x, y)) in local_blocks.iter().copied().enumerate() {
        blocks[idx] = (position.0 + i16::from(x), position.1 + i16::from(y));
    }
    blocks
}

pub fn cell_blocked(board: &Board, x: i16, y: i16) -> bool {
    match board_index(x, y) {
        Some(index) => board[index] != 0,
        None => true,
    }
}

pub fn is_position_valid(
    board: &Board,
    piece: &Piece,
    position: Option<(i16, i16)>,
    rotation: Option<u8>,
) -> bool {
    for (x, y) in compute_blocks(piece, position, rotation) {
        if cell_blocked(board, x, y) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::{Piece, PieceKind};

    #[test]
    fn board_index_is_row_major_and_bounds_checked() {
        assert_eq!(board_index(0, 0), Some(0));
        assert_eq!(board_index(9, 0), Some(9));
        assert_eq!(board_index(0, 1), Some(10));
        assert_eq!(board_index(9, 39), Some(399));
        assert_eq!(board_index(-1, 0), None);
        assert_eq!(board_index(10, 0), None);
        assert_eq!(board_index(0, 40), None);
    }

    #[test]
    fn compute_blocks_matches_reference_global_coordinates() {
        let piece = Piece::new(PieceKind::T, 0, (4, 10));
        assert_eq!(
            compute_blocks(&piece, None, None),
            [(5, 10), (4, 11), (5, 11), (6, 11)]
        );
        assert_eq!(
            compute_blocks(&piece, Some((7, 18)), Some(1)),
            [(9, 19), (8, 18), (8, 19), (8, 20)]
        );

        let i_piece = Piece::new(PieceKind::I, 0, (3, 18));
        assert_eq!(
            compute_blocks(&i_piece, Some((3, 18)), Some(2)),
            [(6, 20), (5, 20), (4, 20), (3, 20)]
        );
    }

    #[test]
    fn is_position_valid_accepts_open_placements_and_rejects_out_of_bounds() {
        let board = [0; BOARD_WIDTH * BOARD_HEIGHT];
        let piece = Piece::new(PieceKind::T, 0, (4, 10));
        assert!(is_position_valid(&board, &piece, None, None));

        let left_oob = Piece::new(PieceKind::I, 0, (-1, 18));
        assert!(!is_position_valid(&board, &left_oob, None, None));

        let top_oob = Piece::new(PieceKind::T, 0, (4, -1));
        assert!(!is_position_valid(&board, &top_oob, None, None));

        let bottom_oob = Piece::new(PieceKind::O, 0, (4, 38));
        assert!(!is_position_valid(&board, &bottom_oob, None, None));

        let right_oob = Piece::new(PieceKind::I, 0, (7, 18));
        assert!(!is_position_valid(&board, &right_oob, None, None));
    }

    #[test]
    fn is_position_valid_rejects_occupied_cell_collisions() {
        let mut board = [0; BOARD_WIDTH * BOARD_HEIGHT];
        let piece = Piece::new(PieceKind::T, 0, (4, 10));
        let collision_index = board_index(5, 11).unwrap();
        board[collision_index] = 9;

        assert!(!is_position_valid(&board, &piece, None, None));
    }

    #[test]
    fn cell_blocked_treats_bounds_and_non_zero_cells_as_blocked() {
        let mut board = [0; BOARD_WIDTH * BOARD_HEIGHT];
        board[board_index(2, 3).unwrap()] = 7;

        assert!(cell_blocked(&board, -1, 0));
        assert!(cell_blocked(&board, 10, 0));
        assert!(cell_blocked(&board, 2, 3));
        assert!(!cell_blocked(&board, 0, 0));
    }
}
