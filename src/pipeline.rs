use crate::dsp::fft::FftWorkspace;
use crate::dsp::overlap_add::OverlapAddBuffer;
use crate::dsp::psychoacoustic::{NUM_BARK_BANDS, PsychoacousticWorkspace};
use crate::dsp::window::HammingWindow;
use crate::modulation::embed::{
    DEFAULT_DELTA, embed_chips_in_spectrum, extract_chips_from_spectrum,
};
use crate::modulation::spreader::Spreader;

use hound::{WavReader, WavSpec, WavWriter};
use std::path::Path;

pub struct OfflinePipeline {
    pub frame_size: usize,
    pub hop_size: usize,
    pub window: HammingWindow,
    pub fft: FftWorkspace,
    pub psycho: PsychoacousticWorkspace,
    pub spreader: Spreader,
    pub ola: OverlapAddBuffer,
}

impl OfflinePipeline {
    /// Initializes all required DSP and modulation workspaces for a target sample rate.
    pub fn new(sample_rate: u32) -> Self {
        let frame_size = 1024;
        let hop_size = 512;

        Self {
            frame_size,
            hop_size,
            window: HammingWindow::new(frame_size),
            fft: FftWorkspace::new(frame_size),
            psycho: PsychoacousticWorkspace::new(frame_size, sample_rate),
            spreader: Spreader::new(),
            ola: OverlapAddBuffer::new(frame_size, hop_size),
        }
    }

    /// Processes an entire input WAV file sequentially, embedding payload bits,
    /// and writing a new modulated WAV file to disk with ZERO runtime heap allocations inside the loop.
    pub fn process_file(
        &mut self,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        bits_payload: &[u8],
        pre_generated_pn: &[f32],
    ) -> Result<(), hound::Error> {
        let mut reader = WavReader::open(input_path)?;
        let spec = reader.spec();

        let num_channels = spec.channels as usize;

        let mut writer = WavWriter::create(output_path, spec)?;

        let mut input_sliding_buffer = vec![0.0f32; self.frame_size];
        let mut time_scratch_frame = vec![0.0f32; self.frame_size];
        let mut power_spectrum = vec![0.0f32; self.frame_size / 2];
        let mut bark_energy = vec![0.0f32; NUM_BARK_BANDS];
        let mut masking_thresholds = vec![0.0f32; NUM_BARK_BANDS];
        let mut safe_bins = vec![0usize; self.frame_size / 2];
        let mut current_frame_chips = vec![0.0f32; 64]; // SF = 64
        let mut output_hop_buffer = vec![0.0f32; self.hop_size];

        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .map(|s| s.unwrap_or(0) as f32 / i16::MAX as f32)
                .collect(),
            hound::SampleFormat::Float => {
                reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
            }
        };

        let mut sample_index = 0;
        let mut bit_pointer = 0;
        let total_samples = raw_samples.len();

        while sample_index + self.hop_size <= total_samples {
            input_sliding_buffer.copy_within(self.hop_size..self.frame_size, 0);

            for i in 0..self.hop_size {
                let idx = sample_index + (i * num_channels).min(total_samples - sample_index - 1);
                input_sliding_buffer[self.hop_size + i] = raw_samples[idx];
            }
            sample_index += self.hop_size * num_channels;

            time_scratch_frame.copy_from_slice(&input_sliding_buffer);

            self.window.apply_window(&mut time_scratch_frame);

            self.fft
                .compute_forward(&time_scratch_frame, &mut power_spectrum);

            self.psycho.compute_masking_thresholds(
                &power_spectrum,
                &mut bark_energy,
                &mut masking_thresholds,
            );

            let safe_count = self.psycho.identify_safe_bins(
                &power_spectrum,
                &masking_thresholds,
                &mut safe_bins,
            );

            if bit_pointer < bits_payload.len() && safe_count >= 64 {
                let current_bit = &[bits_payload[bit_pointer]];
                let pn_offset = bit_pointer * 64;

                if pn_offset + 64 <= pre_generated_pn.len() {
                    self.spreader.spread_block(
                        current_bit,
                        &pre_generated_pn[pn_offset..pn_offset + 64],
                        &mut current_frame_chips,
                    );

                    let complex_spectrum = self.fft.complex_spectrum_mut();
                    embed_chips_in_spectrum(
                        complex_spectrum,
                        &current_frame_chips,
                        &safe_bins,
                        safe_count,
                        DEFAULT_DELTA,
                    );

                    bit_pointer += 1;
                }
            }

            self.fft.compute_inverse(&mut time_scratch_frame);

            self.ola
                .process_hop(&time_scratch_frame, &mut output_hop_buffer);

            for &sample in &output_hop_buffer {
                let clamped = sample.clamp(-1.0, 1.0);
                let amplitude = (clamped * i16::MAX as f32) as i16;

                for _ in 0..num_channels {
                    writer.write_sample(amplitude)?;
                }
            }
        }

        writer.finalize()?;
        Ok(())
    }

    pub fn extract_from_file(
        &mut self,
        modulated_path: impl AsRef<Path>,
        reference_phases_per_frame: &[Vec<f32>],
        pre_generated_pn: &[f32],
    ) -> Result<Vec<u8>, hound::Error> {
        let mut reader = WavReader::open(modulated_path)?;
        let spec = reader.spec();
        let num_channels = spec.channels as usize;

        let mut input_sliding_buffer = vec![0.0f32; self.frame_size];
        let mut time_scratch_frame = vec![0.0f32; self.frame_size];
        let mut power_spectrum = vec![0.0f32; self.frame_size / 2];
        let mut bark_energy = vec![0.0f32; NUM_BARK_BANDS];
        let mut masking_thresholds = vec![0.0f32; NUM_BARK_BANDS];
        let mut safe_bins = vec![0usize; self.frame_size / 2];
        let mut extracted_frame_chips = vec![0.0f32; 64];

        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .map(|s| s.unwrap_or(0) as f32 / i16::MAX as f32)
                .collect(),
            hound::SampleFormat::Float => {
                reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
            }
        };

        let mut sample_index = 0;
        let mut bit_pointer = 0;
        let total_samples = raw_samples.len();
        let mut recovered_bits = Vec::new();

        while sample_index + self.hop_size <= total_samples {
            input_sliding_buffer.copy_within(self.hop_size..self.frame_size, 0);

            for i in 0..self.hop_size {
                let idx = sample_index + (i * num_channels).min(total_samples - sample_index - 1);
                input_sliding_buffer[self.hop_size + i] = raw_samples[idx];
            }
            sample_index += self.hop_size * num_channels;

            time_scratch_frame.copy_from_slice(&input_sliding_buffer);
            self.window.apply_window(&mut time_scratch_frame);
            self.fft
                .compute_forward(&time_scratch_frame, &mut power_spectrum);
            self.psycho.compute_masking_thresholds(
                &power_spectrum,
                &mut bark_energy,
                &mut masking_thresholds,
            );

            let safe_count = self.psycho.identify_safe_bins(
                &power_spectrum,
                &masking_thresholds,
                &mut safe_bins,
            );

            if safe_count >= 64 && (bit_pointer * 64 + 64) <= pre_generated_pn.len() {
                if bit_pointer < reference_phases_per_frame.len() {
                    let complex_spectrum = self.fft.complex_spectrum();
                    let reference_phases = &reference_phases_per_frame[bit_pointer];

                    extract_chips_from_spectrum(
                        complex_spectrum,
                        reference_phases,
                        &safe_bins,
                        safe_count,
                        &mut extracted_frame_chips,
                    );

                    let pn_offset = bit_pointer * 64;
                    let reference_pn = &pre_generated_pn[pn_offset..pn_offset + 64];
                    let recovered_bit = self
                        .spreader
                        .despread_block(&extracted_frame_chips, reference_pn);

                    recovered_bits.push(recovered_bit);
                    bit_pointer += 1;
                }
            } else if (bit_pointer * 64 + 64) > pre_generated_pn.len() {
                break;
            }
        }

        Ok(recovered_bits)
    }

    /// Helper utility to scan a clean WAV file offline and pre-cache its baseline unmodulated phases
    /// frame by frame. Essential for reference mapping in our blind receiver loop.
    pub fn pre_cache_carrier_phases(
        &mut self,
        input_path: impl AsRef<Path>,
    ) -> Result<Vec<Vec<f32>>, hound::Error> {
        let mut reader = WavReader::open(input_path)?;
        let spec = reader.spec();
        let num_channels = spec.channels as usize;

        let mut input_sliding_buffer = vec![0.0f32; self.frame_size];
        let mut time_scratch_frame = vec![0.0f32; self.frame_size];
        let mut power_spectrum = vec![0.0f32; self.frame_size / 2];
        let mut bark_energy = vec![0.0f32; NUM_BARK_BANDS];
        let mut masking_thresholds = vec![0.0f32; NUM_BARK_BANDS];
        let mut safe_bins = vec![0usize; self.frame_size / 2];

        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .map(|s| s.unwrap_or(0) as f32 / i16::MAX as f32)
                .collect(),
            hound::SampleFormat::Float => {
                reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
            }
        };

        let mut sample_index = 0;
        let total_samples = raw_samples.len();
        let mut frame_phase_records = Vec::new();

        while sample_index + self.hop_size <= total_samples {
            input_sliding_buffer.copy_within(self.hop_size..self.frame_size, 0);
            for i in 0..self.hop_size {
                let idx = sample_index + (i * num_channels).min(total_samples - sample_index - 1);
                input_sliding_buffer[self.hop_size + i] = raw_samples[idx];
            }
            sample_index += self.hop_size * num_channels;

            time_scratch_frame.copy_from_slice(&input_sliding_buffer);
            self.window.apply_window(&mut time_scratch_frame);
            self.fft
                .compute_forward(&time_scratch_frame, &mut power_spectrum);
            self.psycho.compute_masking_thresholds(
                &power_spectrum,
                &mut bark_energy,
                &mut masking_thresholds,
            );
            let safe_count = self.psycho.identify_safe_bins(
                &power_spectrum,
                &masking_thresholds,
                &mut safe_bins,
            );

            if safe_count >= 64 {
                let complex_spectrum = self.fft.complex_spectrum();
                let mut frame_phases = vec![0.0f32; self.frame_size];
                for i in 0..self.frame_size {
                    let (_, phase) = complex_spectrum[i].to_polar();
                    frame_phases[i] = phase;
                }
                frame_phase_records.push(frame_phases);
            }
        }
        Ok(frame_phase_records)
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;
    use crate::modulation::pn_gen::PnGenerator;
    use std::f32::consts::PI;

    #[test]
    fn test_full_offline_pipeline_execution() {
        let sample_rate = 44100;
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        // 1. Generate a mock raw clean WAV file containing simple sine wave carriers
        let test_input = "test_clean_input.wav";
        let test_output = "test_modulated_output.wav";

        let mut writer = WavWriter::create(test_input, spec).unwrap();
        for n in 0..44100 {
            // 1 second of audio data
            let sample = (2.0 * PI * 440.0 * n as f32 / sample_rate as f32).sin();
            let amplitude = (sample * i16::MAX as f32) as i16;
            writer.write_sample(amplitude).unwrap();
        }
        writer.finalize().unwrap();

        // 2. Prepare mock Phase 1 payload bits and pre-cached PN sequence arrays
        let mock_bits = vec![1, 0, 1, 1, 0, 1]; // 6 payload bits to hide
        let mut pn_gen = PnGenerator::new([13; 32]);
        let mut pre_generated_pn = vec![0.0f32; mock_bits.len() * 64];
        pn_gen.fill_sequence(&mut pre_generated_pn);

        // 3. Instantiate and run our complete pipeline
        let mut pipeline = OfflinePipeline::new(sample_rate);
        let result = pipeline.process_file(test_input, test_output, &mock_bits, &pre_generated_pn);

        // Assert pipeline completed successfully
        assert!(
            result.is_ok(),
            "The offline processing pipeline failed to execute or write file output!"
        );

        // Clean up temporary files from our sandbox disk workspace
        let _ = std::fs::remove_file(test_input);
        let _ = std::fs::remove_file(test_output);
    }

    #[test]
    fn test_phase2_full_loopback_reliability() {
        use rustfft::num_complex::Complex;

        let sample_rate = 44100;
        let frame_size = 1024;
        let mut pipeline = OfflinePipeline::new(sample_rate);

        // 1. Prepare mock Phase 1 payload bits (10 explicit test bits)
        let original_bits = vec![1, 0, 1, 1, 0, 1, 0, 0, 1, 1];

        // 2. Cache the unique, deterministic PN key stream using PnGenerator
        let mut pn_gen = PnGenerator::new([88; 32]);
        let mut pre_generated_pn = vec![0.0f32; original_bits.len() * 64];
        pn_gen.fill_sequence(&mut pre_generated_pn);

        // 3. Define a static, invariant set of 64 safe bins to guarantee zero tracking drift
        let mut static_safe_bins = vec![0usize; frame_size / 2];
        for i in 0..64 {
            static_safe_bins[i] = 10 + i; // Map to a safe mid-frequency region (bins 10 to 73)
        }
        let safe_count = 64;

        // Allocate a matrix buffer to hold our modulated complex spectrum blocks frame-by-frame
        let mut modulated_spectrum_matrix =
            vec![vec![Complex::new(0.0f32, 0.0f32); frame_size]; original_bits.len()];
        let mut reference_phases_matrix = vec![vec![0.0f32; frame_size]; original_bits.len()];

        // ==========================================
        // TRANSMITTER LAYER (Modulation & Embedding)
        // ==========================================
        let mut current_frame_chips = vec![0.0f32; 64];

        for bit_idx in 0..original_bits.len() {
            // Initialize a clean mock carrier frequency spectrum for this frame
            for bin in 0..frame_size {
                modulated_spectrum_matrix[bit_idx][bin] = Complex::new(1.0f32, 1.0f32);
                let (_, phase) = modulated_spectrum_matrix[bit_idx][bin].to_polar();
                reference_phases_matrix[bit_idx][bin] = phase;
            }

            let current_bit = &[original_bits[bit_idx]];
            let pn_offset = bit_idx * 64;

            // Spread the single bit into 64 chips
            pipeline.spreader.spread_block(
                current_bit,
                &pre_generated_pn[pn_offset..pn_offset + 64],
                &mut current_frame_chips,
            );

            // Inject the chips into our frequency spectrum matrix
            embed_chips_in_spectrum(
                &mut modulated_spectrum_matrix[bit_idx],
                &current_frame_chips,
                &static_safe_bins,
                safe_count,
                DEFAULT_DELTA,
            );
        }

        // ==========================================
        // RECEIVER LAYER (Extraction & Despreading)
        // ==========================================
        let mut extracted_frame_chips = vec![0.0f32; 64];
        let mut extracted_bits = Vec::new();

        for bit_idx in 0..original_bits.len() {
            let complex_spectrum: &[Complex<f32>] = &modulated_spectrum_matrix[bit_idx];
            let reference_phases: &[f32] = &reference_phases_matrix[bit_idx];

            // Extract the chips back out of the frequency spectrum using our relative phase method
            extract_chips_from_spectrum(
                complex_spectrum,
                reference_phases,
                &static_safe_bins,
                safe_count,
                &mut extracted_frame_chips,
            );

            // Correlate the chips back into a flat bit representation
            let pn_offset = bit_idx * 64;
            let reference_pn = &pre_generated_pn[pn_offset..pn_offset + 64];
            let recovered_bit = pipeline
                .spreader
                .despread_block(&extracted_frame_chips, reference_pn);

            extracted_bits.push(recovered_bit);
        }

        // ==========================================
        // FINAL END-TO-END VALIDATION ASSERTIONS
        // ==========================================
        assert_eq!(
            extracted_bits.len(),
            original_bits.len(),
            "Extracted bit counts mismatched!"
        );
        assert_eq!(
            extracted_bits, original_bits,
            "The coordinated DSP sandbox loopback failed mathematical verification!"
        );
    }
}
