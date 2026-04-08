pub const BOARD_WIDTH: usize = 10;
pub const BOARD_HEIGHT: usize = 40;
pub const VISIBLE_HEIGHT: usize = 20;
pub const HIDDEN_ROWS: usize = 20;
pub const GARBAGE_ID: i8 = -1;
pub const SPAWN_X: i16 = 3;
pub const SPAWN_Y: i16 = 18;
pub const ROTATION_NAMES: [&str; 4] = ["N", "E", "S", "W"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_and_spawn_constants_match_reference_values() {
        assert_eq!(BOARD_WIDTH, 10);
        assert_eq!(BOARD_HEIGHT, 40);
        assert_eq!(VISIBLE_HEIGHT, 20);
        assert_eq!(HIDDEN_ROWS, 20);
        assert_eq!(GARBAGE_ID, -1);
        assert_eq!(SPAWN_X, 3);
        assert_eq!(SPAWN_Y, 18);
        assert_eq!(ROTATION_NAMES, ["N", "E", "S", "W"]);
    }
}
