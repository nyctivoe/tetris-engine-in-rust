use crate::Board;
use crate::piece::PieceKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpinMode {
    TOnly,
    AllSpin,
}

impl SpinMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TOnly => "t_only",
            Self::AllSpin => "all_spin",
        }
    }
}

impl Default for SpinMode {
    fn default() -> Self {
        Self::AllSpin
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum B2BMode {
    Surge,
    Chaining,
}

impl B2BMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Surge => "surge",
            Self::Chaining => "chaining",
        }
    }
}

impl Default for B2BMode {
    fn default() -> Self {
        Self::Surge
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpinResult {
    pub piece: PieceKind,
    pub spin_type: &'static str,
    pub is_mini: bool,
    pub is_180: bool,
    pub kick_index: Option<u8>,
    pub rotation_dir: Option<i8>,
    pub corners: Option<u8>,
    pub front_corners: Option<u8>,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttackStats {
    pub attack: i32,
    pub b2b_bonus: i32,
    pub b2b_chain: i32,
    pub b2b_display: i32,
    pub b2b_mode: &'static str,
    pub base_attack: i32,
    pub breaks_b2b: bool,
    pub combo: i32,
    pub combo_active: bool,
    pub combo_attack: i32,
    pub combo_bonus: i32,
    pub combo_multiplier: Option<f64>,
    pub is_difficult: bool,
    pub is_mini: bool,
    pub is_spin: bool,
    pub lines_cleared: i32,
    pub perfect_clear: bool,
    pub qualifies_b2b: bool,
    pub spin: Option<SpinResult>,
    pub spin_type: i32,
    pub surge_charge: i32,
    pub surge_segments: Vec<i32>,
    pub surge_send: i32,
    pub t_spin: &'static str,
    pub garbage_cleared: i32,
    pub immediate_garbage: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClearClassification {
    pub lines_cleared: i32,
    pub spin: Option<SpinResult>,
    pub is_spin: bool,
    pub spin_type: i32,
    pub is_mini: bool,
    pub is_difficult: bool,
    pub qualifies_b2b: bool,
    pub breaks_b2b: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct B2BUpdate {
    pub b2b_chain: i32,
    pub surge_charge: i32,
    pub b2b_bonus: i32,
    pub surge_send: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ComboUpdate {
    pub combo: i32,
    pub combo_active: bool,
}

pub(crate) fn classify_clear(
    cleared_lines: i32,
    spin_result: Option<&SpinResult>,
    perfect_clear: bool,
) -> ClearClassification {
    let spin = spin_result.cloned();
    let is_spin = spin.is_some();
    let spin_type = match spin.as_ref() {
        Some(result) if result.is_mini => 1,
        Some(_) => 2,
        None => 0,
    };
    let is_mini = spin.as_ref().map(|result| result.is_mini).unwrap_or(false);
    let is_difficult = is_difficult_clear(cleared_lines, spin_result, perfect_clear);

    ClearClassification {
        lines_cleared: cleared_lines,
        spin,
        is_spin,
        spin_type,
        is_mini,
        is_difficult,
        qualifies_b2b: is_difficult && cleared_lines > 0,
        breaks_b2b: cleared_lines > 0 && !is_difficult,
    }
}

pub(crate) fn b2b_bonus_for_chain(chain_len: i32) -> i32 {
    let effective = (chain_len - 1).max(0);
    match effective {
        0 => 0,
        1..=2 => 1,
        3..=7 => 2,
        8..=23 => 3,
        24..=66 => 4,
        67..=184 => 5,
        185..=503 => 6,
        504..=1369 => 7,
        _ => 8,
    }
}

pub(crate) fn is_difficult_clear(
    cleared_lines: i32,
    spin_result: Option<&SpinResult>,
    perfect_clear: bool,
) -> bool {
    if cleared_lines <= 0 {
        return false;
    }
    if perfect_clear || cleared_lines == 4 || spin_result.is_some() {
        return true;
    }
    false
}

pub(crate) fn update_b2b_state(
    b2b_mode: B2BMode,
    cleared_lines: i32,
    difficult: bool,
    b2b_chain: i32,
    surge_charge: i32,
) -> B2BUpdate {
    if cleared_lines <= 0 {
        return B2BUpdate {
            b2b_chain,
            surge_charge,
            b2b_bonus: 0,
            surge_send: 0,
        };
    }

    if difficult {
        let next_b2b_chain = b2b_chain + 1;
        let b2b_bonus = b2b_bonus_for_chain(next_b2b_chain);
        let next_surge_charge = match b2b_mode {
            B2BMode::Surge => {
                let b2b_display = next_b2b_chain - 1;
                if b2b_display >= 3 { b2b_display } else { 0 }
            }
            B2BMode::Chaining => 0,
        };
        return B2BUpdate {
            b2b_chain: next_b2b_chain,
            surge_charge: next_surge_charge,
            b2b_bonus,
            surge_send: 0,
        };
    }

    B2BUpdate {
        b2b_chain: 0,
        surge_charge: 0,
        b2b_bonus: 0,
        surge_send: match b2b_mode {
            B2BMode::Surge => surge_charge,
            B2BMode::Chaining => 0,
        },
    }
}

pub(crate) fn surge_segments(total: i32) -> Vec<i32> {
    if total <= 0 {
        return Vec::new();
    }

    let base = total / 3;
    let rem = total % 3;
    vec![
        base + if rem > 0 { 1 } else { 0 },
        base + if rem > 1 { 1 } else { 0 },
        base,
    ]
}

pub(crate) fn base_attack_for_clear(
    cleared_lines: i32,
    spin_result: Option<&SpinResult>,
    board_after_clear: &Board,
) -> (i32, bool) {
    if cleared_lines <= 0 {
        return (0, false);
    }

    let perfect_clear = board_after_clear.iter().all(|&cell| cell == 0);
    if perfect_clear {
        return (5, true);
    }

    if let Some(spin) = spin_result.filter(|spin| spin.spin_type == "t-spin") {
        if spin.is_mini {
            return match cleared_lines {
                1 => (0, false),
                2 => (1, false),
                _ => (0, false),
            };
        }

        return match cleared_lines {
            1 => (2, false),
            2 => (4, false),
            3 => (6, false),
            _ => (0, false),
        };
    }

    match cleared_lines {
        1 => (0, false),
        2 => (1, false),
        3 => (2, false),
        4 => (4, false),
        _ => (0, false),
    }
}

pub(crate) fn combo_after_clear(cleared_lines: i32, combo: i32, combo_active: bool) -> ComboUpdate {
    if cleared_lines <= 0 {
        return ComboUpdate {
            combo: 0,
            combo_active: false,
        };
    }
    if combo_active {
        return ComboUpdate {
            combo: combo + 1,
            combo_active: true,
        };
    }
    ComboUpdate {
        combo: 0,
        combo_active: true,
    }
}

pub(crate) fn combo_attack_down(base_attack: i32, combo: i32) -> i32 {
    let value = if base_attack > 0 {
        f64::from(base_attack) * (1.0 + 0.25 * f64::from(combo))
    } else if combo >= 2 {
        (1.0 + 1.25 * f64::from(combo)).ln()
    } else {
        0.0
    };
    value.floor() as i32
}

pub(crate) fn build_attack_stats(
    b2b_mode: B2BMode,
    classification: ClearClassification,
    perfect_clear: bool,
    combo: i32,
    combo_active: bool,
    b2b_chain: i32,
    b2b_bonus: i32,
    surge_charge: i32,
    surge_send: i32,
    base_attack: i32,
    combo_attack: i32,
    combo_bonus: i32,
    combo_multiplier: Option<f64>,
    attack_total: i32,
) -> AttackStats {
    AttackStats {
        attack: attack_total,
        b2b_bonus,
        b2b_chain,
        b2b_display: if classification.qualifies_b2b {
            (b2b_chain - 1).max(0)
        } else {
            0
        },
        b2b_mode: b2b_mode.as_str(),
        base_attack,
        breaks_b2b: classification.breaks_b2b,
        combo,
        combo_active,
        combo_attack,
        combo_bonus,
        combo_multiplier,
        is_difficult: classification.is_difficult,
        is_mini: classification.is_mini,
        is_spin: classification.is_spin,
        lines_cleared: classification.lines_cleared,
        perfect_clear,
        qualifies_b2b: classification.qualifies_b2b,
        spin: classification.spin.clone(),
        spin_type: classification.spin_type,
        surge_charge,
        surge_segments: surge_segments(surge_send),
        surge_send,
        t_spin: "N",
        garbage_cleared: 0,
        immediate_garbage: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        B2BMode, PieceKind, SpinResult, b2b_bonus_for_chain, build_attack_stats, combo_after_clear,
        combo_attack_down, surge_segments, update_b2b_state,
    };
    use crate::scoring::{base_attack_for_clear, classify_clear};

    fn non_empty_board() -> crate::Board {
        let mut board = [0; 400];
        board[0] = 1;
        board
    }

    #[test]
    fn base_attack_for_standard_clears_matches_reference() {
        for (cleared_lines, expected) in [(1, 0), (2, 1), (3, 2), (4, 4)] {
            let (base_attack, perfect_clear) =
                base_attack_for_clear(cleared_lines, None, &non_empty_board());
            assert_eq!(base_attack, expected);
            assert!(!perfect_clear);
        }
    }

    #[test]
    fn perfect_clear_base_attack_is_five() {
        let (base_attack, perfect_clear) = base_attack_for_clear(4, None, &[0; 400]);
        assert_eq!(base_attack, 5);
        assert!(perfect_clear);
    }

    #[test]
    fn combo_rounding_matches_reference() {
        assert_eq!(combo_attack_down(4, 2), 6);
        assert_eq!(combo_attack_down(0, 2), 1);
        assert_eq!(combo_attack_down(0, 1), 0);
    }

    #[test]
    fn b2b_bonus_ladder_matches_reference_thresholds() {
        assert_eq!(b2b_bonus_for_chain(2), 1);
        assert_eq!(b2b_bonus_for_chain(4), 2);
        assert_eq!(b2b_bonus_for_chain(9), 3);
        assert_eq!(b2b_bonus_for_chain(25), 4);
        assert_eq!(b2b_bonus_for_chain(68), 5);
    }

    #[test]
    fn surge_mode_charges_and_releases_at_b2bx3() {
        assert_eq!(
            update_b2b_state(B2BMode::Surge, 4, true, 3, 0),
            super::B2BUpdate {
                b2b_chain: 4,
                surge_charge: 3,
                b2b_bonus: 2,
                surge_send: 0,
            }
        );
        assert_eq!(
            update_b2b_state(B2BMode::Surge, 1, false, 5, 4),
            super::B2BUpdate {
                b2b_chain: 0,
                surge_charge: 0,
                b2b_bonus: 0,
                surge_send: 4,
            }
        );
        assert_eq!(surge_segments(4), vec![2, 1, 1]);
    }

    #[test]
    fn chaining_mode_disables_stored_surge() {
        assert_eq!(
            update_b2b_state(B2BMode::Chaining, 4, true, 4, 9),
            super::B2BUpdate {
                b2b_chain: 5,
                surge_charge: 0,
                b2b_bonus: 2,
                surge_send: 0,
            }
        );
        assert_eq!(
            update_b2b_state(B2BMode::Chaining, 1, false, 5, 9),
            super::B2BUpdate {
                b2b_chain: 0,
                surge_charge: 0,
                b2b_bonus: 0,
                surge_send: 0,
            }
        );
    }

    #[test]
    fn build_attack_stats_preserves_python_payload_shape() {
        let classification = classify_clear(
            2,
            Some(&SpinResult {
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
            false,
        );
        let stats = build_attack_stats(
            B2BMode::Surge,
            classification,
            false,
            2,
            true,
            3,
            1,
            0,
            0,
            4,
            6,
            2,
            Some(1.5),
            7,
        );

        assert_eq!(stats.base_attack, 4);
        assert_eq!(stats.combo_attack, 6);
        assert_eq!(stats.combo_bonus, 2);
        assert_eq!(stats.combo_multiplier, Some(1.5));
        assert_eq!(stats.attack, 7);
        assert_eq!(stats.b2b_mode, "surge");
        assert_eq!(stats.t_spin, "N");
        assert_eq!(stats.garbage_cleared, 0);
        assert_eq!(stats.immediate_garbage, 0);
    }

    #[test]
    fn combo_after_clear_keeps_first_clear_at_zero_combo() {
        assert_eq!(
            combo_after_clear(1, 7, false),
            super::ComboUpdate {
                combo: 0,
                combo_active: true,
            }
        );
        assert_eq!(
            combo_after_clear(2, 1, true),
            super::ComboUpdate {
                combo: 2,
                combo_active: true,
            }
        );
        assert_eq!(
            combo_after_clear(0, 9, true),
            super::ComboUpdate {
                combo: 0,
                combo_active: false,
            }
        );
    }
}
