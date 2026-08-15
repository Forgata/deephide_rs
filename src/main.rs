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

use std::io::{self, BufRead};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

fn main() {
    let filename = "example.txt";
    let password = "1234";

    // use println!("Default input config: {:?}", config);
    let sample_rate = 48000; // my PC's default sample rate

    // 2. THREAD A
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

    // pre-calcs
    println!("Thread A: Generating deterministic DSSS PN key streams...");
    let shared_secret_seed: [u8; 32] = [42; 32];
    let mut pn_gen = PnGenerator::new(shared_secret_seed);
    let mut pre_generated_pn = vec![0.0f32; flat_payload_bits.len() * 64];
    pn_gen.fill_sequence(&mut pre_generated_pn);

    // Instantiate and Split the Lock-Free SPSC Bridge Bridge
    println!("Thread A: Pre-allocating the atomic 8192-sample SPSC Ring Buffer...");
    let audio_bridge = AudioBridge::new(8192);
    let (producer, consumer) = audio_bridge.split();

    // Thread C Heap
    println!("Thread A: Initializing high-priority allocation-free DSP Worker...");
    let mut dsp_worker = DspWorker::new(sample_rate, consumer);

    // atomic flag to orchestrate safe real-time thread teardowns
    let running_flag = Arc::new(AtomicBool::new(true));
    let worker_flag = Arc::clone(&running_flag);

    // Spawn THREAD C (The Real-Time DSP Processing Loop)
    println!("Thread A: Launching Thread C (DSP Math Worker Environment)...");
    let thread_c_handle = thread::spawn(move || {
        // Thread C owns the consumer side and runs completely allocation-free inside its loop
        dsp_worker.run_loop(worker_flag, &flat_payload_bits, &pre_generated_pn);
        println!("\n[Thread C] Execution loop paused safely. Disposing worker buffers...");
    });

    // Spawn THREAD B (High-Priority Hardware I/O Callback)
    println!("Thread A: Launching Thread B (CPAL Live Audio Callback Interface)...");
    let thread_b_hardware = match HardwareInputDevice::new(sample_rate, producer) {
        Ok(device) => device,
        Err(e) => {
            eprintln!(
                "Thread A Fatal Error: Failed to lock microphone device handle: {}",
                e
            );
            // Emergency flag clear to prevent Thread C from becoming an orphan loop
            running_flag.store(false, Ordering::Release);
            let _ = thread_c_handle.join();
            return;
        }
    };

    // input stream capture loops
    if let Err(e) = thread_b_hardware.play() {
        eprintln!(
            "Thread A Fatal Error: Failed to engage stream playback tracks: {}",
            e
        );
        running_flag.store(false, Ordering::Release);
        let _ = thread_c_handle.join();
        return;
    }

    println!(">>> DEEPHIDE LIVE TRANSMITTING STREAM <<<");
    println!("Speak into your default microphone device. Data is modulating in-place.");
    println!("Press [ENTER] at any time to pause transmission and terminate threads.");
    println!("\n-");

    // Thread A parks safely here, waiting on user interface input while threads execute lock-free
    let stdin = io::stdin();
    let mut iterator = stdin.lock().lines();
    let _ = iterator.next();

    println!("\nThread A: Initiating systematic live system termination sequence...");

    // Suspend physical input listening
    println!("Thread A: Suspending Thread B (Audio Hardware Streams)...");
    let _ = thread_b_hardware.pause();

    // thread c's signal
    println!("Thread A: Signalling Thread C to suspend in-place math execution...");
    running_flag.store(false, Ordering::Release);

    // joining c to main
    println!("Thread A: Joining Thread C back to parent execution scope...");
    match thread_c_handle.join() {
        Ok(_) => println!("Thread A: Thread C safely joined. All states fully preserved."),
        Err(_) => eprintln!(
            "Thread A Error: Thread C tracking panics or structural leaks encountered during shutdown!"
        ),
    }

    println!("\n-");
    println!("DeepHide Offline");
    println!("-");
}
