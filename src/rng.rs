use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

#[derive(Clone)]
pub struct EngineRng {
    inner: StdRng,
}

impl EngineRng {
    pub fn seeded(seed: u64) -> Self {
        Self {
            inner: StdRng::seed_from_u64(seed),
        }
    }

    pub fn shuffle_bag(&mut self, pieces: &mut [i8; 7]) {
        pieces.shuffle(&mut self.inner);
    }

    pub fn choose_index(&mut self, upper: usize) -> usize {
        assert!(upper > 0, "choose_index requires upper > 0");
        self.inner.gen_range(0..upper)
    }
}

impl Default for EngineRng {
    fn default() -> Self {
        Self::seeded(0)
    }
}

#[cfg(test)]
mod tests {
    use super::EngineRng;

    #[test]
    fn seeded_rng_is_deterministic() {
        let mut left = EngineRng::seeded(7);
        let mut right = EngineRng::seeded(7);

        let mut left_bag = [1, 2, 3, 4, 5, 6, 7];
        let mut right_bag = [1, 2, 3, 4, 5, 6, 7];
        left.shuffle_bag(&mut left_bag);
        right.shuffle_bag(&mut right_bag);

        assert_eq!(left_bag, right_bag);
        assert_eq!(left.choose_index(7), right.choose_index(7));
    }
}
