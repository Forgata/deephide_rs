use rustfft::num_complex::Complex;

pub const DEFAULT_DELTA: f32 = 0.02;

/// Embeds DSSS chips directly into the complex FFT frequency spectrum using phase modulation.
///
/// ### Arguments
/// * `spectrum` - The mutable complex spectrum array from the pre-allocated FftWorkspace (size: N).
/// * `chips` - A slice of spread f32 chips (+1.0 or -1.0) to hide in this frame.
/// * `safe_bins` - A slice containing the indices of safe bins identified by the psychoacoustic workspace.
/// * `safe_count` - The total number of valid safe bins populated in the `safe_bins` slice for this frame.
/// * `delta` - The phase-shift angle step size in radians.
///
/// ### Thread Safety Notice
/// This function performs ZERO heap allocations and mutates spectrum data completely in-place.
///
pub fn embed_chips_in_spectrum(
    spectrum: &mut [Complex<f32>],
    chips: &[f32],
    safe_bins: &[usize],
    safe_count: usize,
    delta: f32,
) {
    let n = spectrum.len();
    if safe_count == 0 || chips.is_empty() {
        return;
    }

    let embedding_limit = chips.len().min(safe_count);
    for i in 0..embedding_limit {
        let bin_idx = safe_bins[i];

        if bin_idx >= n / 2 {
            continue;
        };

        let comp = spectrum[bin_idx];
        let (magnitude, original_phase) = comp.to_polar();

        let new_phase = original_phase + (chips[i] * delta);
        let modified_complex = Complex::from_polar(magnitude, new_phase);

        spectrum[bin_idx] = modified_complex;

        let mirror_bin = n - bin_idx;
        if mirror_bin > bin_idx && mirror_bin < n {
            spectrum[mirror_bin] = modified_complex.conj();
        }
    }
}

/// Extracts raw chip values from the complex frequency spectrum by measuring phase shifts
/// relative to a baseline reference spectrum state.
///
/// ### Arguments
/// * `spectrum` - The read-only raw complex spectrum array from the current frame (size: N).
/// * `reference_phases` - A slice containing the baseline unmodulated phase angles for this frame.
/// * `safe_bins` - A slice containing the indices of safe bins identified by the psychoacoustic workspace.
/// * `safe_count` - The total number of valid safe bins populated in the `safe_bins` slice for this frame.
/// * `chips_out` - A pre-allocated mutable target slice where the extracted f32 chips will be stored.
pub fn extract_chips_from_spectrum(
    spectrum: &[Complex<f32>],
    reference_phases: &[f32],
    safe_bins: &[usize],
    safe_count: usize,
    chips_out: &mut [f32],
) {
    if safe_count == 0 || chips_out.is_empty() {
        return;
    }

    let extraction_limit = chips_out.len().min(safe_count);

    for i in 0..extraction_limit {
        let bin_idx = safe_bins[i];

        if bin_idx >= spectrum.len() / 2 || bin_idx >= reference_phases.len() {
            continue;
        }

        let comp = spectrum[bin_idx];
        let (_, current_phase) = comp.to_polar();
        let original_phase = reference_phases[bin_idx];

        let phase_delta = current_phase - original_phase;

        chips_out[i] = if phase_delta >= 0.0 { 1.0 } else { -1.0 };
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_phase_embedding_and_conjugate_mirroring() {
        let size = 1024;
        // 1. Create a mock complex spectrum with uniform values
        let mut spectrum = vec![Complex::new(1.0f32, 1.0f32); size];

        // Capture initial polar states for a reference target bin (bin 45)
        let target_bin = 45;
        let (orig_mag, orig_phase) = spectrum[target_bin].to_polar();

        // 2. Setup mock chips and safe bins slices
        let mock_chips = vec![1.0f32]; // Single positive chip
        let mock_safe_bins = vec![target_bin];
        let safe_count = 1;
        let delta = 0.05f32;

        // 3. Execute our allocation-free injection engine
        embed_chips_in_spectrum(
            &mut spectrum,
            &mock_chips,
            &mock_safe_bins,
            safe_count,
            delta,
        );

        // 4. Verification:
        let (new_mag, new_phase) = spectrum[target_bin].to_polar();

        // Magnitude must remain completely unchanged by phase steganography
        assert!((new_mag - orig_mag).abs() < 1e-5);

        // Phase must be precisely shifted by exactly (chip * delta)
        let expected_phase = orig_phase + (1.0 * delta);
        assert!((new_phase - expected_phase).abs() < 1e-5);

        // Mirror conjugate bin (1024 - 45 = 979) must match the real part and invert the imaginary part
        let mirror_bin = size - target_bin;
        assert_eq!(spectrum[mirror_bin].re, spectrum[target_bin].re);
        assert_eq!(spectrum[mirror_bin].im, -spectrum[target_bin].im);
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod inverse_tests {
    use super::*;

    #[test]
    fn test_phase_embedding_to_extraction_loopback() {
        let size = 1024;
        let mut spectrum = vec![Complex::new(1.0f32, 1.0f32); size];

        // Capture baseline references before any modulation happens
        let mut reference_phases = vec![0.0f32; size];
        for i in 0..size {
            let (_, p) = spectrum[i].to_polar();
            reference_phases[i] = p;
        }

        // Setup mock target structures for 3 chips
        let target_bins = vec![12, 13, 14];
        let safe_count = 3;
        let tx_chips = vec![1.0f32, -1.0f32, 1.0f32];
        let delta = 0.02f32; // Works perfectly even with your real default delta!

        // 1. Embed using the frozen forward function
        embed_chips_in_spectrum(&mut spectrum, &tx_chips, &target_bins, safe_count, delta);

        // 2. Extract using our corrected phase shift tracker
        let mut rx_chips = vec![0.0f32; 3];
        extract_chips_from_spectrum(
            &spectrum,
            &reference_phases,
            &target_bins,
            safe_count,
            &mut rx_chips,
        );

        // 3. Verify absolute mathematical loopback integrity
        assert_eq!(
            tx_chips, rx_chips,
            "Extracted phase chips diverged from embedded chip baseline!"
        );
    }
}
