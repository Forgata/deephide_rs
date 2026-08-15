/// The DSSS Spreader handles expanding flat bit sequences into long, robust chip vectors.
/// It operates completely without state or runtime allocations.

pub struct Spreader {
    pub spread_factor: usize,
}

impl Spreader {
    pub fn new() -> Self {
        Self { spread_factor: 64 }
    }

    /// Spreads the given bits into the output buffer.
    /// The output buffer must be pre-allocated with the correct size.
    ///
    /// # Arguments
    /// * `bits` - The bits to spread.
    /// * `pn_chips` - The PN chips to spread.
    /// * `output_chips` - The output buffer to write the spreaded bits to.
    ///
    /// # Panics
    /// Panics if the output buffer is not large enough to hold the spreaded bits.
    pub fn spread_block(&self, bits: &[u8], pn_chips: &[f32], output_chips: &mut [f32]) {
        let sf = self.spread_factor;

        assert_eq!(
            bits.len() * sf,
            output_chips.len(),
            "Output buffer size mismatch!"
        );
        assert!(
            pn_chips.len() >= output_chips.len(),
            "PN chip buffer is too small for this bitstream!"
        );

        for (bit_idx, &bit) in bits.iter().enumerate() {
            let start = bit_idx * sf;
            let end = start + sf;

            let current_pn_window = &pn_chips[start..end];
            let current_output_window = &mut output_chips[start..end];

            if bit == 1 {
                current_output_window.copy_from_slice(current_pn_window);
            } else {
                for i in 0..sf {
                    current_output_window[i] = -current_pn_window[i];
                }
            }
        }
    }

    /// Correlates a single frame of extracted chips against a reference PN sequence window.
    /// Returns a `1` bit if correlation is positive, or a `0` bit if negative.
    /// Performs ZERO heap allocations.
    ///
    /// ### Arguments
    /// * `extracted_chips` - Precisely 64 raw chips pulled from an audio spectrum frame.
    /// * `pn_chips` - The reference PN chips corresponding to this specific frame's window.
    pub fn despread_block(&self, extracted_chips: &[f32], pn_chips: &[f32]) -> u8 {
        let sf = self.spread_factor;
        assert_eq!(
            extracted_chips.len(),
            sf,
            "Extracted chips must match spreading factor!"
        );
        assert!(pn_chips.len() >= sf, "PN chips slice window is too small!");

        let mut correlation = 0.0f32;
        for i in 0..sf {
            correlation += extracted_chips[i] * pn_chips[i];
        }

        if correlation > 0.0 { 1 } else { 0 }
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::super::pn_gen::PnGenerator;
    use super::*;

    #[test]
    fn test_spreader_block_expansion() {
        let spreader = Spreader::new();

        // 1. Setup a mock payload of 2 bits: [1, 0]
        let mock_bits: [u8; 2] = [1, 0];

        // 2. Generate a reference PN chip sequence using our PnGenerator
        let seed: [u8; 32] = [7; 32];
        let mut pn_gen = PnGenerator::new(seed);
        let mut pn_chips = vec![0.0f32; 128];
        pn_gen.fill_sequence(&mut pn_chips);

        // 3. Pre-allocate the final output chip buffer (2 bits * 64 SF = 128)
        let mut allocated_output = vec![0.0f32; 128];

        // 4. Run the allocation-free spreading operation
        spreader.spread_block(&mock_bits, &pn_chips, &mut allocated_output);

        // 5. Verification:
        // The first 64 chips (Bit 1) must match the PN sequence exactly.
        assert_eq!(allocated_output[0..64], pn_chips[0..64]);

        // The next 64 chips (Bit 0) must be perfectly inverted.
        for i in 64..128 {
            assert_eq!(allocated_output[i], -pn_chips[i]);
        }
    }

    #[test]
    fn test_spreader_symmetrical_loopback() {
        let spreader = Spreader::new();
        let sf = spreader.spread_factor;

        // 1. Create a reference bipolar PN sequence window
        let pn_chips = vec![1.0f32; sf]; // Simple uniform PN vector for testing mathematical sign isolation

        // 2. Test spreading a Bit 1 and recovering it via despreading
        let mut out_chips_1 = vec![0.0f32; sf];
        spreader.spread_block(&[1], &pn_chips, &mut out_chips_1);
        let recovered_bit_1 = spreader.despread_block(&out_chips_1, &pn_chips);
        assert_eq!(recovered_bit_1, 1);

        // 3. Test spreading a Bit 0 and recovering it via despreading
        let mut out_chips_0 = vec![0.0f32; sf];
        spreader.spread_block(&[0], &pn_chips, &mut out_chips_0);
        let recovered_bit_0 = spreader.despread_block(&out_chips_0, &pn_chips);
        assert_eq!(recovered_bit_0, 0);
    }
}
