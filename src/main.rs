mod dsp;
mod modulation;
mod payload_engine;
mod pipeline;
mod utils;

use crate::modulation::embed::{
    DEFAULT_DELTA, embed_chips_in_spectrum, extract_chips_from_spectrum,
};
use crate::modulation::pn_gen::PnGenerator;
use crate::payload_engine::Payload; // Ensure this matches your project's payload visibility
use crate::pipeline::OfflinePipeline;
use rustfft::num_complex::Complex;

fn main() {
    println!("Hello, world!");
    let filename = "example.txt";
    let password = "1234";
    let sample_rate = 44100;
    let frame_size = 1024;

    let tx_payload = payload_engine::prep_payload(filename, password).expect("failed to prepare");
    println!("Payload processed successfully");

    // initial testing
    // let recovery_result = payload_engine::parse_payload(&processed_payload, 256)
    //     .expect("Failed to recover the result");
    // println!("Recovered Filename: {}", recovery_result.filename);
    // println!("Recovered File Bytes: {}", recovery_result.file_bytes.len());

    // if let Ok(text) = String::from_utf8(recovery_result.file_bytes) {
    //     println!("Recovered File Content Preview:\n{}", text);
    // }

    let flat_payload_bits = &tx_payload.encrypted_payload_bits;
    println!(
        "Transmitter: Payload generated. Total serialized bits (including preamble): {}",
        flat_payload_bits.len()
    );

    let shared_key_seed: [u8; 32] = [42; 32];
    let mut tx_pn_gen = PnGenerator::new(shared_key_seed);
    let mut pre_generated_pn = vec![0.0f32; flat_payload_bits.len() * 64];
    tx_pn_gen.fill_sequence(&mut pre_generated_pn);

    let pipeline = OfflinePipeline::new(sample_rate);

    let mut synchronized_safe_bins = vec![0usize; frame_size / 2];
    for i in 0..64 {
        synchronized_safe_bins[i] = 12 + i;
    }
    let safe_count = 64;

    let mut channel_spectrum_matrix =
        vec![vec![Complex::new(1.0f32, 1.0f32); frame_size]; flat_payload_bits.len()];
    let mut baseline_reference_phases = vec![vec![0.0f32; frame_size]; flat_payload_bits.len()];

    for frame_idx in 0..flat_payload_bits.len() {
        for bin in 0..frame_size {
            let (_, phase) = channel_spectrum_matrix[frame_idx][bin].to_polar();
            baseline_reference_phases[frame_idx][bin] = phase;
        }
    }

    let mut current_frame_chips = vec![0.0f32; 64];
    for bit_idx in 0..flat_payload_bits.len() {
        let single_bit = &[flat_payload_bits[bit_idx]];
        let pn_offset = bit_idx * 64;

        // Spreading
        pipeline.spreader.spread_block(
            single_bit,
            &pre_generated_pn[pn_offset..pn_offset + 64],
            &mut current_frame_chips,
        );

        embed_chips_in_spectrum(
            &mut channel_spectrum_matrix[bit_idx],
            &current_frame_chips,
            &synchronized_safe_bins,
            safe_count,
            DEFAULT_DELTA,
        );
    }
    println!("Transmitter: Modulated and injected bits cleanly into frequency channel matrix.");

    // pn recreatoin
    let mut rx_pn_gen = PnGenerator::new(shared_key_seed);
    let mut rx_reference_pn = vec![0.0f32; flat_payload_bits.len() * 64];
    rx_pn_gen.fill_sequence(&mut rx_reference_pn);

    let mut rx_extracted_chips = vec![0.0f32; 64];
    let mut rx_recovered_bits = Vec::with_capacity(flat_payload_bits.len());

    for bit_idx in 0..flat_payload_bits.len() {
        // phase tracing
        extract_chips_from_spectrum(
            &channel_spectrum_matrix[bit_idx],
            &baseline_reference_phases[bit_idx],
            &synchronized_safe_bins,
            safe_count,
            &mut rx_extracted_chips,
        );

        // Correlation
        let pn_offset = bit_idx * 64;
        let reference_window = &rx_reference_pn[pn_offset..pn_offset + 64];
        let recovered_bit = pipeline
            .spreader
            .despread_block(&rx_extracted_chips, reference_window);

        rx_recovered_bits.push(recovered_bit);
    }
    println!("Receiver: Phase extraction and DSSS correlation loops completed.");

    let rx_payload = Payload {
        encrypted_payload_bits: rx_recovered_bits,
        salt: tx_payload.salt,
        key: tx_payload.key,
        encrypted_len: tx_payload.encrypted_len,
    };

    println!("Receiver: Handing reconstructed context back to parse_payload...");
    let recovery_result = payload_engine::parse_payload(&rx_payload, 256)
        .expect("Receiver: Cryptographic authentication or Reed-Solomon restoration failed!");

    println!("\n=== SUCCESS: Full Symmetrical System Integration Verified ===");
    println!("Recovered Filename: {}", recovery_result.filename);
    println!(
        "Recovered File Bytes: {} bytes",
        recovery_result.file_bytes.len()
    );

    if let Ok(text) = String::from_utf8(recovery_result.file_bytes) {
        println!(
            "\nRecovered File Content Output:\n------------------------------\n{}",
            text
        );
    }
}
