use rand::Rng;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub mod crypto;
pub mod fec;
pub mod payload;
pub mod preamble;
pub mod serializer;

/// load_file reads a file from the data directory and returns its contents as a byte vector.
fn load_file<P: AsRef<Path>>(filename: P) -> io::Result<Vec<u8>> {
    let mut file_path = PathBuf::from("data");
    file_path.push(filename);
    fs::read(file_path)
}

#[derive(Debug)]
pub struct Payload {
    pub encrypted_payload_bits: Vec<u8>,
    pub salt: [u8; 16],
    pub key: [u8; 32],
    encrypted_len: usize,
}

/// # Prep Payload
/// Preps a payload for encryption and returns the encrypted payload as a byte vector.
/// The payload is framed, salted, and encrypted using AES-GCM.
///
/// The encrypted payload is then split into frames of a given size and fed into the FEC algorithm.
/// The FEC algorithm then returns a vector of shards, which are then interleaved and serialized into a bitstream.
/// The bitstream is then serialized into a byte vector, which is ready for physical transmission.
/// The bitstream is then injected with a high-entropy 80-bit preamble, and the resulting bitstream is returned.
///
/// ### Parameters
/// * `filename` - The name of the file to be encrypted.
/// * `password` - The password used to encrypt the file.
///
/// ### Returns
/// A byte vector representing the encrypted payload.
pub fn prep_payload(filename: &str, password: &str) -> Result<Payload, String> {
    let payload_bytes =
        load_file(filename).map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;
    println!("Payload bytes: {:?}", payload_bytes.len());

    let framed_payload = payload::frame::frame_payload(&payload_bytes, filename)
        .map_err(|e| format!("Framing error: {}", e))?;

    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);

    let key = crypto::derive_key::derive_key(password, &salt, 60_000);
    let encrypted_payload = crypto::aes_gcm_encryption::aes_gcm_encrypt(&key, &framed_payload)?;

    let encrypted_len = encrypted_payload.len();

    let packets = payload::packetize::packetize_encrypted_payload(&encrypted_payload, 256);
    println!("Packets: {:?}", packets.len());

    let fec_shards = fec::forward_err_correction::apply_reed_solomon_fec(&packets, 3, 3)
        .map_err(|e| format!("FEC generation failed: {}", e))?;

    // let flattened_shards: Vec<u8> = fec_shards.into_iter().flatten().collect();

    let interleaved_shards = fec::fec_interleaver::interleave(&fec_shards, 3, 3);

    let encrypted_payload_bits = serializer::serialize_bits(&interleaved_shards);
    let final_bit_stream = preamble::inject_preamble(&encrypted_payload_bits);

    Ok(Payload {
        encrypted_payload_bits: final_bit_stream,
        salt,
        key,
        encrypted_len,
    })
}

/// # Parse Payload
///
/// Processes a generated Payload struct through the inverse pipeline to reconstruct
/// the original uncorrupted file data and metadata.
///
/// ### Parameters
/// * `payload` - The Payload struct containing the synchronized bitstream, salt, and key.
/// * `frame_size` - The data size configuration used during packetization (e.g., 256).
///
/// ### Returns
/// A result containing the extracted FramedData (filename and raw file bytes).
pub fn parse_payload(
    payload: &Payload,
    frame_size: usize,
) -> Result<payload::frame::FramedData, String> {
    let encrypted_payload_bits = preamble::strip_preamble(&payload.encrypted_payload_bits)
        .map_err(|e| format!("Preamble error: {}", e))?;

    let shard_size = 4 + frame_size;
    let interleaved_shards = serializer::deserialize_bits(&encrypted_payload_bits, shard_size)
        .map_err(|e| format!("Deserialization error: {}", e))?;

    let fec_shards = fec::fec_interleaver::deinterleave(&interleaved_shards, 3, 3);

    let optional_shards: Vec<Option<Vec<u8>>> = fec_shards.into_iter().map(Some).collect();

    let packets = fec::forward_err_correction::recover_reed_solomon_fec(&optional_shards, 3, 3)
        .map_err(|e| format!("FEC recovery failed: {}", e))?;

    let padded_encrypted_payload = payload::packetize::depacketize_encrypted_payload(packets)
        .map_err(|e| format!("Depacketization error: {}", e))?;

    if padded_encrypted_payload.len() < payload.encrypted_len {
        return Err("Recovered payload is shorter than expected encrypted length".to_string());
    }
    let true_encrypted_payload = &padded_encrypted_payload[..payload.encrypted_len];

    let framed_payload =
        crypto::aes_gcm_encryption::aes_gcm_decrypt(&payload.key, true_encrypted_payload)?;

    let final_file_data = payload::frame::deframe_payload(&framed_payload)
        .map_err(|e| format!("Deframing error: {}", e))?;

    Ok(final_file_data)
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    // Helper function to dynamically create dummy files in the 'data' directory for testing
    fn setup_test_file(filename: &str, content: &[u8]) -> PathBuf {
        let mut data_dir = PathBuf::from("data");
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir).unwrap();
        }
        data_dir.push(filename);
        let mut file = File::create(&data_dir).unwrap();
        file.write_all(content).unwrap();
        data_dir
    }

    #[test]
    fn test_pipeline_perfect_round_trip() {
        let test_filename = "test_input_perfect.txt";
        let test_content = b"The quick brown fox jumps over the lazy dog near the stream.";
        let password = "super_secure_password_123";

        // 1. Create file context
        let file_path = setup_test_file(test_filename, test_content);

        // 2. Execute forward pipeline
        let prep_result = prep_payload(test_filename, password);
        assert!(prep_result.is_ok(), "Forward pipeline generation failed");
        let payload = prep_result.unwrap();

        // 3. Execute reverse pipeline
        let parse_result = parse_payload(&payload, 256);
        assert!(parse_result.is_ok(), "Reverse pipeline processing failed");
        let recovered = parse_result.unwrap();

        // 4. Assert exact string data validation matches original context
        assert_eq!(recovered.filename, test_filename);
        assert_eq!(recovered.file_bytes, test_content);

        // Clean up filesystem context
        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn test_pipeline_fec_failure_exceeding_limits() {
        let test_filename = "test_fec_failure.txt";
        let test_content = vec![0x42; 500];
        let password = "fec_failure_pass";

        let file_path = setup_test_file(test_filename, &test_content);
        let payload = prep_payload(test_filename, password).unwrap();

        let encrypted_payload_bits =
            preamble::strip_preamble(&payload.encrypted_payload_bits).unwrap();
        let interleaved_shards =
            serializer::deserialize_bits(&encrypted_payload_bits, 4 + 256).unwrap();
        let fec_shards = fec::fec_interleaver::deinterleave(&interleaved_shards, 3, 3);

        // Inject 4 drops inside a single block of 6 shards (Limit is 3)
        let mut optional_shards: Vec<Option<Vec<u8>>> = fec_shards.into_iter().map(Some).collect();
        if optional_shards.len() >= 6 {
            optional_shards[0] = None;
            optional_shards[1] = None;
            optional_shards[2] = None;
            optional_shards[3] = None; // 4th drop!
        }

        // Ensure that the engine safely surfaces a Result::Err instead of crashing or panicking
        let packets = fec::forward_err_correction::recover_reed_solomon_fec(&optional_shards, 3, 3);
        assert!(
            packets.is_err(),
            "FEC should fail when 4 shards are missing"
        );

        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn test_pipeline_missing_file_errors() {
        // Test that passing a non-existent file path safely bubbles an explicit IO error string
        let result = prep_payload("completely_imaginary_file.mov", "password");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read file"));
    }
}
