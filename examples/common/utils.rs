use rand::Rng;
use rand::seq::SliceRandom;

/// Randomizes offset ordering in-place.
pub fn shuffle_offsets<R: Rng + ?Sized>(offsets: &mut [usize], rng: &mut R) {
    if offsets.len() <= 1 {
        return;
    }
    offsets.shuffle(rng);
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::shuffle_offsets;

    #[test]
    fn shuffle_offsets_preserves_offsets_and_changes_seeded_order() {
        let original = vec![0, 4, 8, 12, 16, 20, 24, 28];
        let mut offsets = original.clone();
        let mut rng = StdRng::seed_from_u64(42);

        shuffle_offsets(&mut offsets, &mut rng);

        assert_ne!(offsets, original);

        let mut sorted = offsets;
        sorted.sort_unstable();
        assert_eq!(sorted, original);
    }
}
