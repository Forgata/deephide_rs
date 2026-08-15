pub const NUM_BARK_BANDS: usize = 24;

/// The masking factor determines the percentage of energy in a Bark band
/// that can be used to hide signal without becoming audible.
pub const MASKING_FACTOR: f32 = 0.15;

pub struct PsychoacousticWorkspace {
    sample_rate: u32,
    frame_size: usize,
    bin_to_bark_map: Vec<usize>,
}

impl PsychoacousticWorkspace {
    /// Pre-calculates the static linear-frequency-to-Bark-band mappings for a given sample rate.
    pub fn new(frame_size: usize, sample_rate: u32) -> Self {
        let half_size = frame_size / 2;
        let mut bin_to_bark_map = vec![0; half_size];

        for i in 0..half_size {
            let freq = (i as f32 * sample_rate as f32) / frame_size as f32;
            let bark =
                13.0 * (0.00076 * freq).atan() + 3.5 * ((freq / 7500.0) * (freq / 7500.0)).atan();

            let band_index = (bark.floor() as usize).clamp(0, NUM_BARK_BANDS - 1);
            bin_to_bark_map[i] = band_index;
        }

        Self {
            sample_rate,
            frame_size,
            bin_to_bark_map,
        }
    }

    /// Computes the accumulated energy and masking thresholds per Bark band.
    ///
    /// # Thread Safety Notice
    /// This function performs ZERO heap allocations and mutates caller-allocated slices.

    pub fn compute_masking_thresholds(
        &self,
        power_spectrum: &[f32],
        bark_energy_out: &mut [f32],
        thresholds_out: &mut [f32],
    ) {
        assert_eq!(
            power_spectrum.len(),
            self.frame_size / 2,
            "Power spectrum size mismatch!"
        );
        assert_eq!(
            bark_energy_out.len(),
            NUM_BARK_BANDS,
            "Bark energy output size mismatch!"
        );
        assert_eq!(
            thresholds_out.len(),
            NUM_BARK_BANDS,
            "Thresholds output size mismatch!"
        );

        bark_energy_out.fill(0.0);

        for i in 0..power_spectrum.len() {
            let band_idx = self.bin_to_bark_map[i];
            bark_energy_out[band_idx] += power_spectrum[i];
        }

        for i in 0..NUM_BARK_BANDS {
            thresholds_out[i] = MASKING_FACTOR * bark_energy_out[i];
        }
    }

    /// Scans the spectrum and identifies bins safe for data injection.
    /// A bin is considered safe if its current power is strictly below the masking threshold.
    ///
    /// # Return Value
    /// Returns the absolute count of safe bins written into `safe_bins_out`.
    ///
    /// # Thread Safety Notice
    /// This function performs ZERO heap allocations. It uses a tracking index to avoid dynamic resizing arrays.

    pub fn identify_safe_bins(
        &self,
        power_spectrum: &[f32],
        thresholds: &[f32],
        safe_bins_out: &mut [usize],
    ) -> usize {
        assert_eq!(
            power_spectrum.len(),
            self.frame_size / 2,
            "Power spectrum size mismatch!"
        );
        assert_eq!(
            thresholds.len(),
            NUM_BARK_BANDS,
            "Masking thresholds size mismatch!"
        );

        let mut safe_count = 0;

        for i in 0..power_spectrum.len() {
            let band_idx = self.bin_to_bark_map[i];
            let bin_power = power_spectrum[i];
            let band_threshold = thresholds[band_idx];

            if bin_power < band_threshold {
                if safe_count < safe_bins_out.len() {
                    safe_bins_out[safe_count] = i;
                    safe_count += 1;
                } else {
                    break;
                }
            }
        }

        safe_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psychoacoustic_masking_and_safe_bins() {
        let frame_size = 1024;
        let sample_rate = 44100; // Testing with standard high-quality audio rate
        let workspace = PsychoacousticWorkspace::new(frame_size, sample_rate);

        // 1. Setup mock power spectrum with a massive spike in a specific region
        let mut power_spectrum = vec![1.0f32; frame_size / 2];
        power_spectrum[10] = 5000.0; // Artificial high-energy tone inside a lower band

        let mut bark_energy = vec![0.0f32; NUM_BARK_BANDS];
        let mut thresholds = vec![0.0f32; NUM_BARK_BANDS];

        // 2. Run allocation-free threshold extraction
        workspace.compute_masking_thresholds(&power_spectrum, &mut bark_energy, &mut thresholds);

        // Verify that energy was captured and thresholds calculated
        let target_band = workspace.bin_to_bark_map[10];
        assert!(bark_energy[target_band] > 5000.0);
        assert!(thresholds[target_band] > 750.0); // 5000 * 0.15

        // 3. Setup a pre-allocated array to collect safe bin indices
        let mut safe_bins = vec![0usize; frame_size / 2];

        // 4. Extract safe bins
        let safe_count = workspace.identify_safe_bins(&power_spectrum, &thresholds, &mut safe_bins);

        // Verification: The high-energy bin 10 should fail the safety test because bin_power (5000) > threshold (approx 751)
        let mut bin_10_is_safe = false;
        for i in 0..safe_count {
            if safe_bins[i] == 10 {
                bin_10_is_safe = true;
                break;
            }
        }
        assert!(
            !bin_10_is_safe,
            "High energy spectral component was incorrectly flagged as safe!"
        );
    }

    #[test]
    fn test_psychoacoustic_receiver_recreation() {
        let frame_size = 1024;
        let sample_rate = 44100;

        let tx_psycho = PsychoacousticWorkspace::new(frame_size, sample_rate);
        let rx_psycho = PsychoacousticWorkspace::new(frame_size, sample_rate);

        // Simulated power spectrum arrived at the receiver
        let power_spectrum = vec![2.0f32; frame_size / 2];

        let mut tx_bark = vec![0.0f32; NUM_BARK_BANDS];
        let mut tx_thresh = vec![0.0f32; NUM_BARK_BANDS];
        let mut rx_bark = vec![0.0f32; NUM_BARK_BANDS];
        let mut rx_thresh = vec![0.0f32; NUM_BARK_BANDS];

        tx_psycho.compute_masking_thresholds(&power_spectrum, &mut tx_bark, &mut tx_thresh);
        rx_psycho.compute_masking_thresholds(&power_spectrum, &mut rx_bark, &mut rx_thresh);

        assert_eq!(
            tx_thresh, rx_thresh,
            "Receiver threshold derivation drifted!"
        );

        let mut tx_safe_bins = vec![0usize; frame_size / 2];
        let mut rx_safe_bins = vec![0usize; frame_size / 2];

        let tx_count = tx_psycho.identify_safe_bins(&power_spectrum, &tx_thresh, &mut tx_safe_bins);
        let rx_count = rx_psycho.identify_safe_bins(&power_spectrum, &rx_thresh, &mut rx_safe_bins);

        assert_eq!(tx_count, rx_count);
        assert_eq!(
            tx_safe_bins[0..tx_count],
            rx_safe_bins[0..rx_count],
            "Receiver safe bin map mismatched transmitter!"
        );
    }
}
