use crate::constants::{BOARD_HEIGHT, BOARD_WIDTH, GARBAGE_ID};
use crate::engine::TetrisEngine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GarbageBatch {
    pub lines: i32,
    pub timer: i32,
    pub col: u8,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PendingGarbageSummary {
    pub total_lines: i32,
    pub min_timer: i32,
    pub max_timer: i32,
    pub batch_count: i32,
    pub landing_within_one_ply: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutgoingAttackResolution {
    pub incoming_before: PendingGarbageSummary,
    pub incoming_after: PendingGarbageSummary,
    pub outgoing_attack: i32,
    pub canceled: i32,
    pub sent: i32,
    pub used_opener_multiplier: bool,
    pub opener_phase: bool,
}

impl TetrisEngine {
    pub fn get_pending_garbage_summary(&self) -> PendingGarbageSummary {
        let total_lines = self.incoming_garbage.iter().map(|batch| batch.lines).sum::<i32>();
        let min_timer = self
            .incoming_garbage
            .iter()
            .map(|batch| batch.timer)
            .min()
            .unwrap_or(0);
        let max_timer = self
            .incoming_garbage
            .iter()
            .map(|batch| batch.timer)
            .max()
            .unwrap_or(0);

        PendingGarbageSummary {
            total_lines,
            min_timer,
            max_timer,
            batch_count: self.incoming_garbage.len() as i32,
            landing_within_one_ply: self.incoming_garbage.iter().any(|batch| batch.timer <= 1),
        }
    }

    pub fn add_incoming_garbage(&mut self, lines: i32, timer: i32, col: Option<u8>) {
        if lines <= 0 {
            return;
        }

        let col = col.unwrap_or_else(|| self.next_garbage_hole_column());
        self.incoming_garbage.push(GarbageBatch { lines, timer, col });
    }

    pub fn cancel_garbage(&mut self, attack: i32) -> i32 {
        let mut remaining = attack.max(0);
        while remaining > 0 && !self.incoming_garbage.is_empty() {
            let oldest = &mut self.incoming_garbage[0];
            if oldest.lines <= remaining {
                remaining -= oldest.lines;
                self.incoming_garbage.remove(0);
            } else {
                oldest.lines -= remaining;
                remaining = 0;
            }
        }
        remaining
    }

    pub fn resolve_outgoing_attack(
        &mut self,
        outgoing_attack: i32,
        opener_phase: Option<bool>,
    ) -> OutgoingAttackResolution {
        let _ = opener_phase;
        let outgoing_attack = outgoing_attack.max(0);
        let incoming_before = self.get_pending_garbage_summary();
        let total_before = incoming_before.total_lines;
        let sent = self.cancel_garbage(outgoing_attack);
        let incoming_after = self.get_pending_garbage_summary();
        let canceled = total_before - incoming_after.total_lines;
        self.total_attack_canceled += canceled;

        OutgoingAttackResolution {
            incoming_before,
            incoming_after,
            outgoing_attack,
            canceled,
            sent,
            used_opener_multiplier: false,
            opener_phase: false,
        }
    }

    pub fn apply_garbage(&mut self, lines: i32, col: u8) {
        if lines <= 0 {
            return;
        }

        let lines = usize::try_from(lines.min(BOARD_HEIGHT as i32))
            .expect("garbage lines are positive after clamping");
        let col = usize::from(col);
        assert!(col < BOARD_WIDTH, "garbage hole column must be in bounds");

        let pushed_off = &self.board[..lines * BOARD_WIDTH];
        let topped_out = pushed_off.iter().any(|&cell| cell != 0);

        let mut shifted = [0; BOARD_WIDTH * BOARD_HEIGHT];
        let remaining = BOARD_HEIGHT - lines;
        shifted[..remaining * BOARD_WIDTH].copy_from_slice(&self.board[lines * BOARD_WIDTH..]);

        for row in remaining..BOARD_HEIGHT {
            let start = row * BOARD_WIDTH;
            let end = start + BOARD_WIDTH;
            shifted[start..end].fill(GARBAGE_ID);
            shifted[start + col] = 0;
        }

        self.board = shifted;
        self.garbage_col = Some(col as u8);
        if topped_out {
            self.game_over = true;
            self.game_over_reason = Some("garbage_top_out".to_string());
        }
    }

    pub fn tick_garbage(&mut self) -> i32 {
        let mut landed = 0;
        let mut still_pending = Vec::with_capacity(self.incoming_garbage.len());
        let pending = std::mem::take(&mut self.incoming_garbage);
        for mut batch in pending {
            batch.timer -= 1;
            if batch.timer <= 0 {
                self.apply_garbage(batch.lines, batch.col);
                landed += batch.lines;
            } else {
                still_pending.push(batch);
            }
        }
        self.incoming_garbage = still_pending;
        landed
    }

    fn next_garbage_hole_column(&mut self) -> u8 {
        let previous = self
            .incoming_garbage
            .last()
            .map(|batch| batch.col)
            .or(self.garbage_col);
        let mut available = [0_u8; BOARD_WIDTH - 1];
        let mut len = 0;
        for col in 0..BOARD_WIDTH as u8 {
            if Some(col) != previous {
                available[len] = col;
                len += 1;
            }
        }
        let idx = self.rng.choose_index(len);
        available[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::{GarbageBatch, PendingGarbageSummary};
    use crate::board::board_index;
    use crate::{BOARD_HEIGHT, BOARD_WIDTH, GARBAGE_ID, TetrisEngine};

    fn set_board_cell(engine: &mut TetrisEngine, x: i16, y: i16, value: i8) {
        let index = board_index(x, y).expect("test coordinates must be in bounds");
        engine.board[index] = value;
    }

    #[test]
    fn add_incoming_garbage_with_explicit_and_random_columns() {
        let mut engine = TetrisEngine::with_seed(0);

        engine.add_incoming_garbage(3, 10, Some(4));
        engine.add_incoming_garbage(2, 5, None);

        assert_eq!(
            engine.incoming_garbage[0],
            GarbageBatch {
                lines: 3,
                timer: 10,
                col: 4,
            }
        );
        assert_ne!(engine.incoming_garbage[1].col, 4);
    }

    #[test]
    fn next_garbage_hole_column_uses_last_applied_column_when_queue_is_empty() {
        let mut engine = TetrisEngine::with_seed(1);
        engine.garbage_col = Some(6);

        engine.add_incoming_garbage(1, 3, None);

        assert_ne!(engine.incoming_garbage[0].col, 6);
    }

    #[test]
    fn cancel_garbage_consumes_oldest_batches_first() {
        let mut engine = TetrisEngine::default();
        engine.incoming_garbage = vec![
            GarbageBatch {
                lines: 2,
                timer: 10,
                col: 1,
            },
            GarbageBatch {
                lines: 3,
                timer: 11,
                col: 2,
            },
        ];

        let remaining_attack = engine.cancel_garbage(4);

        assert_eq!(remaining_attack, 0);
        assert_eq!(
            engine.incoming_garbage,
            vec![GarbageBatch {
                lines: 1,
                timer: 11,
                col: 2,
            }]
        );
    }

    #[test]
    fn resolve_outgoing_attack_ignores_opener_multiplier() {
        let mut engine = TetrisEngine::default();
        engine.incoming_garbage = vec![GarbageBatch {
            lines: 5,
            timer: 10,
            col: 2,
        }];

        let result = engine.resolve_outgoing_attack(3, Some(true));

        assert!(!result.used_opener_multiplier);
        assert!(!result.opener_phase);
        assert_eq!(result.canceled, 3);
        assert_eq!(result.sent, 0);
        assert_eq!(result.incoming_after.total_lines, 2);
        assert_eq!(engine.total_attack_canceled, 3);
    }

    #[test]
    fn apply_garbage_shifts_board_up() {
        let mut engine = TetrisEngine::default();
        set_board_cell(&mut engine, 0, (BOARD_HEIGHT - 3) as i16, 7);

        engine.apply_garbage(2, 4);

        let bottom_row = BOARD_HEIGHT - 1;
        for x in 0..BOARD_WIDTH as i16 {
            let index = board_index(x, bottom_row as i16).expect("bottom row index must exist");
            let expected = if x == 4 { 0 } else { GARBAGE_ID };
            assert_eq!(engine.board[index], expected);
        }
        assert_eq!(engine.garbage_col, Some(4));
        let shifted_idx = board_index(0, (BOARD_HEIGHT - 5) as i16).expect("shifted cell exists");
        assert_eq!(engine.board[shifted_idx], 7);
    }

    #[test]
    fn tick_garbage_lands_expired_batches_and_keeps_pending() {
        let mut engine = TetrisEngine::default();
        engine.incoming_garbage = vec![
            GarbageBatch {
                lines: 2,
                timer: 1,
                col: 3,
            },
            GarbageBatch {
                lines: 1,
                timer: 3,
                col: 5,
            },
        ];

        let landed = engine.tick_garbage();

        assert_eq!(landed, 2);
        assert_eq!(
            engine.incoming_garbage,
            vec![GarbageBatch {
                lines: 1,
                timer: 2,
                col: 5,
            }]
        );
        let hole_idx = board_index(3, (BOARD_HEIGHT - 1) as i16).expect("bottom hole exists");
        assert_eq!(engine.board[hole_idx], 0);
        assert_eq!(
            engine.get_pending_garbage_summary(),
            PendingGarbageSummary {
                total_lines: 1,
                min_timer: 2,
                max_timer: 2,
                batch_count: 1,
                landing_within_one_ply: false,
            }
        );
    }

    #[test]
    fn tick_garbage_applies_multiple_expired_batches_in_order() {
        let mut engine = TetrisEngine::default();
        engine.incoming_garbage = vec![
            GarbageBatch {
                lines: 1,
                timer: 1,
                col: 2,
            },
            GarbageBatch {
                lines: 1,
                timer: 1,
                col: 7,
            },
        ];

        let landed = engine.tick_garbage();

        assert_eq!(landed, 2);
        assert!(engine.incoming_garbage.is_empty());
        let final_hole = board_index(7, (BOARD_HEIGHT - 1) as i16).expect("bottom row exists");
        assert_eq!(engine.board[final_hole], 0);
    }

    #[test]
    fn apply_garbage_marks_top_out_when_blocks_are_pushed_off() {
        let mut engine = TetrisEngine::default();
        set_board_cell(&mut engine, 2, 0, 9);

        engine.apply_garbage(1, 4);

        assert!(engine.game_over);
        assert_eq!(engine.game_over_reason.as_deref(), Some("garbage_top_out"));
        assert_eq!(engine.board.len(), BOARD_WIDTH * BOARD_HEIGHT);
    }

    #[test]
    fn apply_garbage_clamps_line_count_and_preserves_board_shape() {
        let mut engine = TetrisEngine::default();
        set_board_cell(&mut engine, 0, (BOARD_HEIGHT - 1) as i16, 8);

        engine.apply_garbage(BOARD_HEIGHT as i32 + 5, 6);

        for row in 0..BOARD_HEIGHT as i16 {
            for col in 0..BOARD_WIDTH as i16 {
                let index = board_index(col, row).expect("board index must exist");
                let expected = if col == 6 { 0 } else { GARBAGE_ID };
                assert_eq!(engine.board[index], expected);
            }
        }
        assert!(engine.game_over);
        assert_eq!(engine.game_over_reason.as_deref(), Some("garbage_top_out"));
    }

    #[test]
    fn add_incoming_garbage_ignores_non_positive_line_counts() {
        let mut engine = TetrisEngine::default();

        engine.add_incoming_garbage(0, 10, Some(3));
        engine.add_incoming_garbage(-2, 10, Some(3));

        assert!(engine.incoming_garbage.is_empty());
    }

    #[test]
    fn apply_garbage_ignores_non_positive_line_counts() {
        let mut engine = TetrisEngine::default();
        let before = engine.board;

        engine.apply_garbage(0, 3);
        engine.apply_garbage(-4, 3);

        assert_eq!(engine.board, before);
        assert!(!engine.game_over);
    }
}
