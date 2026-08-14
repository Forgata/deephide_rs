/// # Inject Preamble
/// Prepends a high-entropy 80-bit sync pattern to the bitstream.
/// This pattern is used to synchronize the receiving tracking station.
pub fn inject_preamble(payload_bits: &[u8]) -> Vec<u8> {
    let preamble: [u8; 80] = [
        1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1,
        0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0,
        1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 1,
    ];

    let mut sync_stream = Vec::with_capacity(preamble.len() + payload_bits.len());

    sync_stream.extend_from_slice(&preamble);
    sync_stream.extend_from_slice(payload_bits);

    sync_stream
}

/// # Strip Preamble
/// Searches for the high-entropy 80-bit sync pattern in the bitstream,
/// and returns everything trailing after that pattern.
pub fn strip_preamble(sync_stream: &[u8]) -> Result<Vec<u8>, String> {
    let preamble: [u8; 80] = [
        1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1,
        0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0,
        1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 1,
    ];

    if sync_stream.len() < preamble.len() {
        return Err("Bitstream is too short to contain a preamble".to_string());
    }

    let match_index = sync_stream
        .windows(preamble.len())
        .position(|window| window == preamble);

    match match_index {
        Some(idx) => {
            let payload_start = idx + preamble.len();
            Ok(sync_stream[payload_start..].to_vec())
        }
        None => Err("Preamble synchronization pattern not found in bitstream".to_string()),
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_inject_preamble_boundaries() {
        // Construct a small mock payload bitstream (3 bits)
        let mock_payload = vec![1, 1, 1];

        let result = inject_preamble(&mock_payload);

        // Expected length: 80 (preamble) + 3 (payload) = 83 items
        assert_eq!(result.len(), 83);

        // Verify the preamble pattern was injected correctly at the front
        assert_eq!(result[0], 1);
        assert_eq!(result[1], 0);
        assert_eq!(result[17], 1);
        assert_eq!(result[79], 1);

        // Verify the original payload bits trailing immediately behind the preamble
        assert_eq!(&result[80..], &[1, 1, 1]);
    }

    #[test]
    fn test_inject_preamble_with_empty_payload() {
        let empty_payload: Vec<u8> = Vec::new();
        let result = inject_preamble(&empty_payload);

        // Output must still contain the full 80-bit preamble sequence
        assert_eq!(result.len(), 80);
    }

    #[test]
    fn test_inject_and_strip_preamble_round_trip() {
        let mock_payload = vec![1, 0, 1, 1, 0, 0, 1, 1];

        let synthesized_stream = inject_preamble(&mock_payload);
        let recovered_payload = strip_preamble(&synthesized_stream).expect("Stripping failed");

        assert_eq!(recovered_payload, mock_payload);
    }

    #[test]
    fn test_strip_preamble_with_leading_noise() {
        let mock_payload = vec![1, 1, 0, 0];
        let mut synthesized_stream = inject_preamble(&mock_payload);

        // Simulate channel noise before the transmission starts
        let mut noisy_stream = vec![0, 0, 1, 1, 0, 1];
        noisy_stream.append(&mut synthesized_stream);

        let recovered_payload =
            strip_preamble(&noisy_stream).expect("Failed to locate preamble amid noise");
        assert_eq!(recovered_payload, mock_payload);
    }

    #[test]
    fn test_strip_preamble_missing_pattern_errors() {
        let bad_stream = vec![1, 0, 1, 0, 1, 0];
        let result = strip_preamble(&bad_stream);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Bitstream is too short to contain a preamble"
        );

        let macro_bad_stream = vec![0; 120];
        let result_macro = strip_preamble(&macro_bad_stream);
        assert!(result_macro.is_err());
        assert_eq!(
            result_macro.unwrap_err(),
            "Preamble synchronization pattern not found in bitstream"
        );
    }
}
