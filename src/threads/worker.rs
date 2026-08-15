use crate::dsp::fft::FftWorkspace;
use crate::dsp::psychoacoustic::{NUM_BARK_BANDS, PsychoacousticWorkspace};
use crate::dsp::window::HammingWindow;
use crate::modulation::embed::{DEFAULT_DELTA, embed_chips_in_spectrum};
use crate::modulation::spreader::Spreader;

use rtrb::Consumer;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The Real-Time DSP Worker coordinates allocation-free signal loops inside Thread C.
pub struct DspWorker {
    frame_size: usize,
    hop_size: usize,
    consumer: Consumer<f32>,
    window: HammingWindow,
    fft: FftWorkspace,
    psycho: PsychoacousticWorkspace,
    spreader: Spreader,
}

impl DspWorker {
    /// Pre-allocates all mathematical structures on the heap before the thread loop begins.
    /// This should be called exclusively from Thread A (Main Orchestrator).
    pub fn new(sample_rate: u32, consumer: Consumer<f32>) -> Self {
        let frame_size = 1024;
        let hop_size = 512;

        Self {
            frame_size,
            hop_size,
            consumer,
            window: HammingWindow::new(frame_size),
            fft: FftWorkspace::new(frame_size),
            psycho: PsychoacousticWorkspace::new(frame_size, sample_rate),
            spreader: Spreader::new(),
        }
    }

    /// The continuous processing loop execution environment for Thread C.
    /// Strictly adheres to real-time execution constraints: NO inline allocations, NO locks, NO logging.
    pub fn run_loop(
        &mut self,
        running_flag: Arc<AtomicBool>,
        bits_payload: &[u8],
        pre_generated_pn: &[f32],
    ) {
        // runtime trackers and scratchpaces allocation
        let mut time_accumulator_frame = vec![0.0f32; self.frame_size];
        let mut time_scratch_frame = vec![0.0f32; self.frame_size];
        let mut power_spectrum = vec![0.0f32; self.frame_size / 2];
        let mut bark_energy = vec![0.0f32; NUM_BARK_BANDS];
        let mut masking_thresholds = vec![0.0f32; NUM_BARK_BANDS];
        let mut safe_bins = vec![0usize; self.frame_size / 2];
        let mut current_frame_chips = vec![0.0f32; 64];

        let mut samples_accumulated = 0;
        let mut bit_pointer = 0;

        // Continuous processing block
        while running_flag.load(Ordering::Acquire) {
            let mut read_any = false;

            // Polling bridge for raw incoming samples from Thread B
            while let Ok(sample) = self.consumer.pop() {
                read_any = true;

                if samples_accumulated < self.frame_size {
                    time_accumulator_frame[samples_accumulated] = sample;
                    samples_accumulated += 1;
                }

                // Once we reach a full 1024-sample frame boundary, trigger our DSP pipeline blocks
                if samples_accumulated == self.frame_size {
                    // Duplicate into scratch buffer to protect the main time timeline history
                    time_scratch_frame.copy_from_slice(&time_accumulator_frame);

                    // Smooth signal edges with our pre-calculated Hamming curve
                    self.window.apply_window(&mut time_scratch_frame);

                    // Compute forward Fourier transform to extract complex spectrum data
                    self.fft
                        .compute_forward(&time_scratch_frame, &mut power_spectrum);

                    // Compute psychoacoustic mask energy thresholds
                    self.psycho.compute_masking_thresholds(
                        &power_spectrum,
                        &mut bark_energy,
                        &mut masking_thresholds,
                    );

                    // Extract eligible safe frequency bins
                    let safe_count = self.psycho.identify_safe_bins(
                        &power_spectrum,
                        &masking_thresholds,
                        &mut safe_bins,
                    );

                    // Inject payload chips if space is available
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

                    // Run IFFT loopback to generate output time-domain samples
                    self.fft.compute_inverse(&mut time_scratch_frame);

                    // Note: In a live full Tx setup, `time_scratch_frame` would now be routed
                    // through an output speaker buffer or written to a dedicated playback stream.

                    // Slide historical data left by the 512-sample hop size to preserve overlap context
                    time_accumulator_frame.copy_within(self.hop_size..self.frame_size, 0);
                    samples_accumulated = self.hop_size;
                }
            }

            // If the lock-free bridge is temporarily empty, yield the core gracefully to minimize CPU spikes
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
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    #[test]
    fn test_dsp_worker_thread_continuous_execution() {
        let sample_rate = 44100;
        let bridge = AudioBridge::new(2048);
        let (mut producer, consumer) = bridge.split();

        // 1. Initialize our pre-allocated worker structure
        let mut worker = DspWorker::new(sample_rate, consumer);

        let running_flag = Arc::new(AtomicBool::new(true));
        let worker_flag = Arc::clone(&running_flag);

        // Prepare dummy payload trackers
        let mock_bits = vec![1, 0, 1];
        let mock_pn = vec![1.0f32; 3 * 64];

        // 2. Spawn Thread C containing our allocation-free execution environment
        let worker_handle = thread::spawn(move || {
            worker.run_loop(worker_flag, &mock_bits, &mock_pn);
        });

        // 3. Feed a continuous mock stream of zeros from our transmitter side
        for _ in 0..1500 {
            while producer.push(0.0f32).is_err() {
                thread::yield_now();
            }
        }

        // Give the worker thread a brief execution window to consume samples
        thread::sleep(Duration::from_millis(50));

        // 4. Shutdown the worker thread loop safely
        running_flag.store(false, Ordering::Release);
        let join_result = worker_handle.join();

        // Assert thread ran continuously without triggering any runtime panics
        assert!(
            join_result.is_ok(),
            "Thread C crashed or panicked during real-time sample processing!"
        );
    }
}
