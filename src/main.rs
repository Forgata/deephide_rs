mod dsp;
mod modulation;
mod payload_engine;
mod pipeline;
mod threads;
mod utils;

use crate::modulation::pn_gen::PnGenerator;
use crate::threads::bridge::AudioBridge;
use crate::threads::hardware::HardwareInputDevice;
use crate::threads::worker::DspWorker;

use hound::{WavSpec, WavWriter};
use std::io::{self, BufRead};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=======================================================");

    let filename = "example.txt";
    let password = "1234";
    let sample_rate = 48000;

    // 1. THREAD A: Low Priority Data Preparation Phase
    println!("Thread A: Loading target file and executing Phase 1 Data Prep...");
    let tx_payload = match payload_engine::prep_payload(filename, password) {
        Ok(payload) => payload,
        Err(e) => {
            eprintln!("Thread A Fatal Error: Data serialization failed: {}", e);
            return;
        }
    };

    let flat_payload_bits = tx_payload.encrypted_payload_bits;
    println!(
        "Thread A: Phase 1 bitstream compiled successfully. Capacity: {} bits.",
        flat_payload_bits.len()
    );

    // 2. THREAD A: Pre-calculate the deterministic DSSS key sequences
    println!("Thread A: Generating deterministic DSSS PN key streams...");
    let shared_secret_seed: [u8; 32] = [42; 32];
    let mut pn_gen = PnGenerator::new(shared_secret_seed);
    let mut pre_generated_pn = vec![0.0f32; flat_payload_bits.len() * 64];
    pn_gen.fill_sequence(&mut pre_generated_pn);

    // 3. THREAD A: Pre-allocate BOTH Lock-Free SPSC Bridges
    println!("Thread A: Initializing SPSC Ring Buffers for Hardware and File I/O Bridges...");
    let input_bridge = AudioBridge::new(16384);
    let output_bridge = AudioBridge::new(16384);

    let (input_producer, input_consumer) = input_bridge.split();
    let (output_producer, mut output_consumer) = output_bridge.split();

    // 4. THREAD A: Initialize Thread C (DSP Worker State Context) on the Heap
    println!("Thread A: Hooking up high-priority allocation-free DSP Worker...");
    let mut dsp_worker = DspWorker::new(sample_rate, input_consumer, output_producer);

    // Setup an atomic flag to orchestrate safe real-time thread shutdowns across the cluster
    let running_flag = Arc::new(AtomicBool::new(true));
    let worker_flag = Arc::clone(&running_flag);
    let writer_flag = Arc::clone(&running_flag);

    // 5. THREAD A: Spawn THREAD C (The Real-Time DSP Processing Loop)
    println!("Thread A: Launching Thread C (DSP Math Worker Environment)...");
    let thread_c_handle = thread::spawn(move || {
        dsp_worker.run_loop(worker_flag, &flat_payload_bits, &pre_generated_pn);
        println!("\n[Thread C] Execution loop paused safely. Disposing worker buffers...");
    });

    // 6. THREAD A: Setup and Spawn THREAD D (The Background Disk Writer Thread)
    let output_wav_filename = "recorded_secret.wav";
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    println!("Thread A: Launching Thread D (Background File Writer Scope)...");
    let thread_d_handle = thread::spawn(move || {
        let mut wav_writer = WavWriter::create(output_wav_filename, spec)
            .expect("Thread D Error: Failed to allocate output WAV file target on disk!");

        while writer_flag.load(Ordering::Acquire) {
            let mut wrote_samples = false;

            while let Ok(modulated_sample) = output_consumer.pop() {
                wrote_samples = true;
                let clamped = modulated_sample.clamp(-1.0, 1.0);
                let amplitude = (clamped * i16::MAX as f32) as i16;
                wav_writer
                    .write_sample(amplitude)
                    .expect("Thread D Error: Disk write stall occurred!");
            }

            if !wrote_samples {
                thread::sleep(Duration::from_millis(5));
            }
        }

        // Post-loop drain: Flush any remaining trailing fragments out to disk after flag drop
        while let Ok(modulated_sample) = output_consumer.pop() {
            let clamped = modulated_sample.clamp(-1.0, 1.0);
            let amplitude = (clamped * i16::MAX as f32) as i16;
            let _ = wav_writer.write_sample(amplitude);
        }

        wav_writer
            .finalize()
            .expect("Thread D Error: Failed to cleanly finalize WAV data blocks on disk!");
        println!("[Thread D] Audio file safely flushed and closed on disk.");
    });

    // 7. THREAD A: Initialize and Spawn THREAD B (High-Priority Hardware I/O Callback)
    println!("Thread A: Launching Thread B (CPAL Live Audio Callback Interface)...");
    let thread_b_hardware = match HardwareInputDevice::new(sample_rate, input_producer) {
        Ok(device) => device,
        Err(e) => {
            eprintln!(
                "Thread A Fatal Error: Failed to lock microphone device handle: {}",
                e
            );
            running_flag.store(false, Ordering::Release);
            let _ = thread_c_handle.join();
            let _ = thread_d_handle.join();
            return;
        }
    };

    // Activate physical input stream capture hardware loops
    if let Err(e) = thread_b_hardware.play() {
        eprintln!(
            "Thread A Fatal Error: Failed to engage stream playback tracks: {}",
            e
        );
        running_flag.store(false, Ordering::Release);
        let _ = thread_c_handle.join();
        let _ = thread_d_handle.join();
        return;
    }

    println!("\n=======================================================");
    println!(">>> DEEPHIDE SYSTEM ONLINE: RECORDING LIVE AUDIO TO DISK <<<");
    println!("Speak into your microphone now. Data is embedding in-place.");
    println!(
        "Audio is actively streaming into: '{}'",
        output_wav_filename
    );
    println!("Press [ENTER] at any time to freeze recording and finalize files.");
    println!("=======================================================");

    // Thread A safely parks right here on the input terminal listener!
    let stdin = io::stdin();
    let mut iterator = stdin.lock().lines();
    let _ = iterator.next();

    // 8. SAFE TEARDOWN PROTOCOLS
    println!("\nThread A: Initiating systematic live system termination sequence...");
    let _ = thread_b_hardware.pause();

    // Drop the atomic flag to gracefully halt Thread C and Thread D loops simultaneously
    running_flag.store(false, Ordering::Release);

    println!("Thread A: Rejoining child thread contexts...");
    let _ = thread_c_handle.join();
    let _ = thread_d_handle.join();

    println!("\n=======================================================");
    println!("SUCCESS: DeepHide Engine offline. Live steganographic track saved.");
    println!("Final File Created: '{}'", output_wav_filename);
    println!("=======================================================");
}
