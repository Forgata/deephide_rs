use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// Coordinates memory buffers and runners for allocation-free Fourier transforms.
pub struct FftWorkspace {
    forward_fft: Arc<dyn Fft<f32>>,
    inverse_fft: Arc<dyn Fft<f32>>,
    complex_buffer: Vec<Complex<f32>>,
    scratch_space: Vec<Complex<f32>>,
}

impl FftWorkspace {
    pub fn new(size: usize) -> Self {
        let mut planner = FftPlanner::new();
        let forward_fft = planner.plan_fft_forward(size);
        let inverse_fft = planner.plan_fft_inverse(size);

        let scratch_len = forward_fft
            .get_inplace_scratch_len()
            .max(inverse_fft.get_inplace_scratch_len());

        Self {
            forward_fft,
            inverse_fft,
            complex_buffer: vec![Complex::default(); size],
            scratch_space: vec![Complex::default(); scratch_len],
        }
    }

    /// Computes the forward FFT of time-domain samples and writes to a pre-allocated power spectrum buffer.
    ///
    /// # Arguments
    /// * `time_samples` - A slice of input audio floats (must match workspace size).
    /// * `power_spectrum` - A mutable target slice for output magnitude data (size: workspace size / 2).
    ///
    /// # Thread Safety Notice
    /// This function performs ZERO heap allocations and uses pre-cached scratch blocks.
    ///
    pub fn compute_forward(&mut self, time_samples: &[f32], power_spectrum: &mut [f32]) {
        let size = self.complex_buffer.len();
        assert_eq!(
            time_samples.len(),
            size,
            "Input time samples size mismatch!"
        );
        assert_eq!(
            power_spectrum.len(),
            size / 2,
            "Power spectrum output size mismatch!"
        );

        for i in 0..size {
            self.complex_buffer[i] = Complex::new(time_samples[i], 0.0);
        }

        self.forward_fft
            .process_with_scratch(&mut self.complex_buffer, &mut self.scratch_space);

        for i in 0..size / 2 {
            let comp = self.complex_buffer[i];
            power_spectrum[i] = comp.norm_sqr();
        }
    }

    /// Exposes a read-only reference to the internal raw complex frequency spectrum buffer.
    /// This will be utilized by our mapper and steganographic embedding layers.
    pub fn complex_spectrum(&self) -> &[Complex<f32>] {
        &self.complex_buffer
    }

    /// Exposes a mutable reference to the internal complex spectrum buffer.
    /// This lets our embedding modules inject chips directly before running an IFFT.
    pub fn complex_spectrum_mut(&mut self) -> &mut [Complex<f32>] {
        &mut self.complex_buffer
    }

    /// Computes the Inverse FFT from the current modified complex spectrum back into a time-domain slice.
    ///
    /// # Thread Safety Notice
    /// This function performs ZERO heap allocations. It normalizes the rustfft output by 1/N.
    ///
    pub fn compute_inverse(&mut self, output_time_samples: &mut [f32]) {
        let size = self.complex_buffer.len();
        assert_eq!(
            output_time_samples.len(),
            size,
            "Output time sample target size mismatch!"
        );

        self.inverse_fft
            .process_with_scratch(&mut self.complex_buffer, &mut self.scratch_space);

        let scale = 1.0 / size as f32;
        for i in 0..size {
            output_time_samples[i] = self.complex_buffer[i].re * scale;
        }
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_fft_sine_wave_energy_concentration() {
        let size = 1024;
        let mut workspace = FftWorkspace::new(size);

        // 1. Generate a pure, discrete sine wave signal targeting a specific frequency bin (e.g., bin 32)
        let target_bin = 32;
        let mut time_samples = vec![0.0f32; size];
        for n in 0..size {
            time_samples[n] = (2.0 * PI * target_bin as f32 * n as f32 / size as f32).sin();
        }

        // 2. Pre-allocate our target power spectrum vector
        let mut power_spectrum = vec![0.0f32; size / 2];

        // 3. Run our allocation-free execution engine
        workspace.compute_forward(&time_samples, &mut power_spectrum);

        // 4. Verification:
        // Find the bin with the absolute highest energy concentration
        let mut max_energy = 0.0f32;
        let mut peak_bin = 0;
        for (bin, &energy) in power_spectrum.iter().enumerate() {
            if energy > max_energy {
                max_energy = energy;
                peak_bin = bin;
            }
        }

        // The energy concentration peak must land exactly on our target frequency bin
        assert_eq!(
            peak_bin, target_bin,
            "FFT energy did not concentrate in the expected frequency bin!"
        );
        assert!(
            max_energy > 100.0,
            "Peak energy concentration is abnormally low!"
        );

        // 5. Test Inverse Loopback: Reconstruct time samples directly from the spectrum
        let mut reconstructed_time = vec![0.0f32; size];
        workspace.compute_inverse(&mut reconstructed_time);

        // Assert that the loopback reconstruction matches our input sine wave within an epsilon boundary
        for i in 0..size {
            let delta = (reconstructed_time[i] - time_samples[i]).abs();
            assert!(
                delta < 1e-4,
                "Inverse reconstruction error too high at sample {}: delta {}",
                i,
                delta
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::f32::consts::PI;

        #[test]
        fn test_fft_receiver_phase_recovery() {
            let size = 1024;
            let mut tx_fft = FftWorkspace::new(size);
            let mut rx_fft = FftWorkspace::new(size);

            // Generate a mock time-domain signal
            let mut time_samples = vec![0.0f32; size];
            for n in 0..size {
                time_samples[n] = (2.0 * PI * 16.0 * n as f32 / size as f32).sin();
            }

            let mut tx_power = vec![0.0f32; size / 2];
            let mut rx_power = vec![0.0f32; size / 2];

            tx_fft.compute_forward(&time_samples, &mut tx_power);
            rx_fft.compute_forward(&time_samples, &mut rx_power);

            // Verify power spectra match
            assert_eq!(tx_power, rx_power);

            // Verify complex phase matches exactly between isolated workspaces
            let tx_spec = tx_fft.complex_spectrum();
            let rx_spec = rx_fft.complex_spectrum();

            for i in 0..size {
                let (_, tx_phase) = tx_spec[i].to_polar();
                let (_, rx_phase) = rx_spec[i].to_polar();
                assert_eq!(
                    tx_phase, rx_phase,
                    "Receiver phase spectrum diverged from transmitter source at bin {}!",
                    i
                );
            }
        }
    }
}
