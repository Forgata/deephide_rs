use crate::dsp::window::HammingWindow;
use crate::dsp::fft::FftWorkspace;
use crate::dsp::psychoacoustic::{PsychoacousticWorkspace, NUM_BARK_BANDS};
use crate::dsp::overlap_add::OverlapAddBuffer; // Add this back
use crate::modulation::spreader::Spreader;
use crate::modulation::embed::{embed_chips_in_spectrum, DEFAULT_DELTA};

use rtrb::{Consumer, Producer}; // Import Producer as well
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The Real-Time DSP Worker coordinates allocation-free signal loops inside Thread C.
pub struct DspWorker {
    frame_size: usize,
    hop_size: usize,
    consumer: Consumer<f32>,
    output_producer: Producer<f32>, // Track the lock-free output queue handle
    window: HammingWindow,
    fft: FftWorkspace,
    psycho: PsychoacousticWorkspace,
    spreader: Spreader,
    ola: OverlapAddBuffer, // Instantiate our pre-allocated reconstruction slider
}

impl DspWorker {
    /// Pre-allocates all mathematical structures on the heap before the thread loop begins.
    pub fn new(sample_rate: u32, consumer: Consumer<f32>, output_producer: Producer<f32>) -> Self {
        let frame_size = 1024;
        let hop_size = 512;

        Self {
            frame_size,
            hop_size,
            consumer,
            output_producer,
            window: HammingWindow::new(frame_size),
            fft: FftWorkspace::new(frame_size),
            psycho: PsychoacousticWorkspace::new(frame_size, sample_rate),
            spreader: Spreader::new(),
            ola: OverlapAddBuffer::new(frame_size, hop_size),
        }
    }

    /// The continuous processing loop execution environment for Thread C.
    pub fn run_loop(
        &mut self, 
        running_flag: Arc<AtomicBool>,
        bits_payload: &[u8],
        pre_generated_pn: &[f32],
    ) {
        let mut time_accumulator_frame = vec![0.0f32; self.frame_size];
        let mut time_scratch_frame = vec![0.0f32; self.frame_size];
        let mut power_spectrum = vec![0.0f32; self.frame_size / 2];
        let mut bark_energy = vec![0.0f32; NUM_BARK_BANDS];
        let mut masking_thresholds = vec![0.0f32; NUM_BARK_BANDS];
        let mut safe_bins = vec![0usize; self.frame_size / 2];
        let mut current_frame_chips = vec![0.0f32; 64];
        let mut output_hop_buffer = vec![0.0f32; self.hop_size]; // 512 size target

        let mut samples_accumulated = 0;
        let mut bit_pointer = 0;

        while running_flag.load(Ordering::Acquire) {
            let mut read_any = false;

            while let Ok(sample) = self.consumer.pop() {
                read_any = true;

                if samples_accumulated < self.frame_size {
                    time_accumulator_frame[samples_accumulated] = sample;
                    samples_accumulated += 1;
                }

                if samples_accumulated == self.frame_size {
                    time_scratch_frame.copy_from_slice(&time_accumulator_frame);

                    self.window.apply_window(&mut time_scratch_frame);
                    self.fft.compute_forward(&time_scratch_frame, &mut power_spectrum);
                    self.psycho.compute_masking_thresholds(&power_spectrum, &mut bark_energy, &mut masking_thresholds);
                    
                    let safe_count = self.psycho.identify_safe_bins(&power_spectrum, &masking_thresholds, &mut safe_bins);

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

                    // ==========================================
                    // ROUTING ENHANCEMENT: Reconstruct & Stream Out
                    // ==========================================
                    // Mix the synthesized frame back into continuous 512-sample audio chunks
                    self.ola.process_hop(&time_scratch_frame, &mut output_hop_buffer);

                    // Push the modulated samples lock-free to Thread A's recorder loop
                    for &modulated_sample in &output_hop_buffer {
                        if self.output_producer.push(modulated_sample).is_err() {
                            // Output queue cushion boundary hit. Drop gracefully if disk can't keep up.
                        }
                    }

                    time_accumulator_frame.copy_within(self.hop_size..self.frame_size, 0);
                    samples_accumulated = self.hop_size;
                }
            }

            if !read_any {
                std::thread::sleep(Duration::from_micros(250));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threads::bridge::AudioBridge;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_dsp_worker_thread_continuous_execution() {
        let sample_rate = 44100;
        
        // Setup input audio hardware bridge
        let input_bridge = AudioBridge::new(2048);
        let (mut input_producer, input_consumer) = input_bridge.split();

        // Setup output recording bridge to fulfill new constructor signatures
        let output_bridge = AudioBridge::new(2048);
        let (output_producer, mut output_consumer) = output_bridge.split();

        let mut worker = DspWorker::new(sample_rate, input_consumer, output_producer);

        let running_flag = Arc::new(AtomicBool::new(true));
        let worker_flag = Arc::clone(&running_flag);

        let mock_bits = vec![];
        let mock_pn = vec![1.0f32; 3 * 64];

        let worker_handle = thread::spawn(move || {
            worker.run_loop(worker_flag, &mock_bits, &mock_pn);
        });

        // Feed mock sample data
        for _ in 0..1500 {
            while input_producer.push(0.0f32).is_err() {
                thread::yield_now();
            }
        }

        thread::sleep(Duration::from_millis(50));
        running_flag.store(false, Ordering::Release);
        let _ = worker_handle.join();

        // Pop data from output consumer to verify active generation loopback
        let mut data_pushed_out = false;
        while output_consumer.pop().is_ok() {
            data_pushed_out = true;
        }
        assert!(data_pushed_out, "The updated real-time DSP worker loop failed to push any output chunks downstream!");
    }
}
