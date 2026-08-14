#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    // Helper function to dynamically create the mock directory and file structure
    fn setup_mock_file(filename: &str, content: &[u8]) -> PathBuf {
        let mut data_dir = PathBuf::from("data");
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir).expect("Failed to create mock data directory");
        }
        data_dir.push(filename);
        let mut file = File::create(&data_dir).expect("Failed to create mock payload file");
        file.write_all(content).expect("Failed to write mock data");
        data_dir
    }

    #[test]
    fn test_prep_payload_integration_pipeline() {
        // --- 1. SETUP MOCK FILE & CONSTANTS ---
        let filename = "example.txt";
        let password = "super_secure_password_123";
        let original_content =
            b"Hello world! This is a mock integration payload for testing Phase 1.";

        let file_path = setup_mock_file(filename, original_content);

        // --- 2. FORWARD PIPELINE (EXECUTE PRODUCTION CODE) ---
        let prep_result = prep_payload(filename, password);
        assert!(
            prep_result.is_ok(),
            "Forward prep_payload pipeline failed: {:?}",
            prep_result.err()
        );

        let output_payload = prep_result.unwrap();
        let bitstream = output_payload.encrypted_payload_bits;

        // --- 3. REVERSE PIPELINE (THE PARSER & RECOVERY LOGIC) ---

        // Step A: Lock onto and extract the Preamble
        // Assumes your preamble module has a parsing or synchronization function
        // For example: preamble::strip_preamble() or preamble::find_sync_pattern()
        let post_preamble_bits = preamble::strip_preamble(&bitstream)
            .expect("Parser failed: Could not lock onto the high-entropy 80-bit preamble");

        // Step B: Deserialize the Bitstream back into Byte Shards
        let serialized_shards = serializer::deserialize_bits(&post_preamble_bits)
            .expect("Parser failed: Could not deserialize bitstream back into byte vectors");

        // Step C: Deinterleave the shards
        // Mirroring the 3, 3 configuration passed to the forward interleaver
        let interleaved_fec_shards = fec::fec_interleaver::deinterleave(&serialized_shards, 3, 3)
            .expect("Parser failed: Shard deinterleaving operation failed");

        // Step D: Decode Reed-Solomon Forward Error Correction
        // We decode using the original parameters (3 data shards, 3 parity shards)
        let recovered_packets =
            fec::forward_err_correction::decode_reed_solomon_fec(&interleaved_fec_shards, 3, 3)
                .map_err(|e| format!("FEC recovery failed: {}", e))
                .unwrap();

        // Step E: Depacketize / Reconstruct the complete Encrypted Byte Stream
        let encrypted_payload = payload::packetize::depacketize_encrypted_payload(
            &recovered_packets,
        )
        .expect("Parser failed: Failed to collapse packets back into continuous encrypted block");

        // Step F: Decrypt using AES-GCM
        // We reuse the exact derived key or re-derive using output_payload.salt + password
        let recovered_framed_payload =
            crypto::aes_gcm_encryption::aes_gcm_decrypt(&output_payload.key, &encrypted_payload)
                .expect("Parser failed: AES-GCM decryption failed. Key or Salt mismatch.");

        // Step G: Unframe the payload to extract pure contents
        let (parsed_content, parsed_filename) =
            payload::frame::unframe_payload(&recovered_framed_payload)
                .expect("Parser failed: Malformed metadata framing structure");

        // --- 4. VALIDATION & ASSERTIONS ---
        assert_eq!(
            parsed_filename, filename,
            "Recovered filename metadata metadata does not match!"
        );
        assert_eq!(
            parsed_content, original_content,
            "CRITICAL: Recovered byte stream data corruption!"
        );

        // Clean up the mock filesystem artifact after a successful loop
        let _ = fs::remove_file(file_path);
    }
}
