use rand::{Rng, RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub struct PnGenerator {
    rng: ChaCha8Rng,
}

impl PnGenerator {
    /// Initializes a new PN generator using a deterministic 32-byte seed.
    pub fn new(seed: [u8; 32]) -> Self {
        Self {
            rng: ChaCha8Rng::from_seed(seed),
        }
    }

    /// Fills a pre-allocated slice with bipolar DSSS chips (+1.0 or -1.0).
    /// Performs ZERO heap allocations. Mutates caller-owned memory in place.
    pub fn fill_sequence(&mut self, buffer: &mut [f32]) {
        for chip in buffer.iter_mut() {
            *chip = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
        }
    }
}

#[cfg(test)]

/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_pn_generator_determinism() {
        let seed_a: [u8; 32] = [42; 32];
        let seed_b: [u8; 32] = [42; 32];
        let seed_c: [u8; 32] = [99; 32]; // Different seed

        let mut gen_a = PnGenerator::new(seed_a);
        let mut gen_b = PnGenerator::new(seed_b);
        let mut gen_c = PnGenerator::new(seed_c);

        let mut buffer_a = vec![0.0f32; 100];
        let mut buffer_b = vec![0.0f32; 100];
        let mut buffer_c = vec![0.0f32; 100];

        // Fill all sequences
        gen_a.fill_sequence(&mut buffer_a);
        gen_b.fill_sequence(&mut buffer_b);
        gen_c.fill_sequence(&mut buffer_c);

        // Assert that generators with matching seeds yield identical chips
        assert_eq!(buffer_a, buffer_b);

        // Assert that a different seed yields a totally distinct sequence
        assert_ne!(buffer_a, buffer_c);

        // Verify values are strictly bipolar (+1.0 or -1.0)
        for &val in &buffer_a {
            assert!(val == 1.0 || val == -1.0);
        }
    }

    #[test]
    fn test_pn_generator_receiver_recreation() {
        let shared_secret_seed: [u8; 32] = [101; 32];
        let stream_length = 512;

        // 1. Transmitter Side: Generates chips to embed
        let mut tx_generator = PnGenerator::new(shared_secret_seed);
        let mut tx_cached_chips = vec![0.0f32; stream_length];
        tx_generator.fill_sequence(&mut tx_cached_chips);

        // 2. Receiver Side: Re-instantiates generator later with the same seed
        let mut rx_generator = PnGenerator::new(shared_secret_seed);
        let mut rx_recreated_chips = vec![0.0f32; stream_length];
        rx_generator.fill_sequence(&mut rx_recreated_chips);

        // 3. Verify absolute matching across the execution divide
        assert_eq!(
            tx_cached_chips, rx_recreated_chips,
            "Receiver failed to perfectly recreate the deterministic reference PN stream!"
        );
    }
}
