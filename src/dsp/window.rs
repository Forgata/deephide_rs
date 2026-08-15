use std::f32::consts::PI;

pub struct HammingWindow {
    coeffs: Vec<f32>,
}

impl HammingWindow {
    /// Generates a new window workspace of a fixed size (e.g., 1024).
    /// Pre-calculates the Hamming window coefficients entirely on the heap.
    pub fn new(size: usize) -> Self {
        assert!(size > 1, "Window size must be greater than 1");

        let mut coeffs = vec![0.0f32; size];
        let denominator = (size - 1) as f32;

        for n in 0..size {
            // Precise implementation of the Hamming window formula
            coeffs[n] = 0.54 - 0.46 * ((2.0 * PI * n as f32) / denominator).cos();
        }

        Self { coeffs }
    }

    /// Applies the pre-calculated Hamming window to an audio frame in-place.
    ///
    /// # Arguments
    /// * `frame` - A mutable slice of audio samples (must match the workspace size).
    ///
    /// # Thread Safety Notice
    /// This function performs ZERO heap allocations and runs deterministically in-place.
    pub fn apply_window(&self, frame: &mut [f32]) {
        assert_eq!(
            frame.len(),
            self.coeffs.len(),
            "Audio frame size must match the pre-allocated window size!"
        );

        for (i, sample) in frame.iter_mut().enumerate() {
            *sample *= self.coeffs[i];
        }
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_hamming_window_bounds_and_symmetry() {
        let size = 1024;
        let workspace = HammingWindow::new(size);

        // 1. Create a mock frame of solid 1.0 values
        let mut mock_frame = vec![1.0f32; size];

        // 2. Apply our window function in-place
        workspace.apply_window(&mut mock_frame);

        // 3. Verify window properties:
        // Edges should drop significantly (Hamming starts/ends at 0.08)
        assert!((mock_frame[0] - 0.08).abs() < 1e-4);
        assert!((mock_frame[size - 1] - 0.08).abs() < 1e-4);

        // Center sample (512) should be at peak amplitude (1.0)
        assert!((mock_frame[size / 2] - 1.0).abs() < 1e-4);

        // Verify mathematical symmetry across the window halves using epsilon threshold
        for i in 0..(size / 2) {
            let diff = (mock_frame[i] - mock_frame[size - 1 - i]).abs();
            assert!(
                diff < 1e-6,
                "Asymmetry found at index {} and {}: diff was {}",
                i,
                size - 1 - i,
                diff
            );
        }
    }

    #[test]
    fn test_hamming_window_receiver_equivalence() {
        let size = 1024;
        let tx_window = HammingWindow::new(size);
        let rx_window = HammingWindow::new(size);

        // Simulate identical raw time-domain audio blocks captured at Tx and Rx sides
        let mut tx_frame = vec![0.5f32; size];
        let mut rx_frame = vec![0.5f32; size];

        // Process frames independently
        tx_window.apply_window(&mut tx_frame);
        rx_window.apply_window(&mut rx_frame);

        // Verify mathematical equivalence across the transmission divide
        for i in 0..size {
            assert_eq!(
                tx_frame[i], rx_frame[i],
                "Receiver windowing calculation diverged from transmitter baseline at index {}!",
                i
            );
        }
    }
}
