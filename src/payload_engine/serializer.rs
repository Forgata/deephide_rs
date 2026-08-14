/// # Serialize Bits
/// Converts an array of interleaved shards into a raw bitstream (0s and 1s)
/// suitable for physical modulation.
///
pub fn serialize_bits(interleaved_shards: &[Vec<u8>]) -> Vec<u8> {
    if interleaved_shards.is_empty() {
        return Vec::new();
    }

    let total_bytes: usize = interleaved_shards.iter().map(|shard| shard.len()).sum();
    let mut bitstream = Vec::with_capacity(total_bytes * 8);

    for shard in interleaved_shards {
        for &byte in shard {
            for shift in 0..8 {
                let bit = (byte >> shift) & 1;
                bitstream.push(bit);
            }
        }
    }

    bitstream
}

/// # Deserialize Bits
/// Reconstructs raw bitstream elements (0s and 1s) back into standard bytes,
/// grouping them into separate shard vectors of the specified chunk size.
pub fn deserialize_bits(bitstream: &[u8], shard_size: usize) -> Result<Vec<Vec<u8>>, String> {
    if bitstream.is_empty() {
        return Ok(Vec::new());
    }

    if bitstream.len() % 8 != 0 {
        return Err("Bitstream length must be a multiple of 8 to form complete bytes".to_string());
    }

    let total_bytes = bitstream.len() / 8;
    let mut flat_bytes = Vec::with_capacity(total_bytes);

    for byte_bits in bitstream.chunks_exact(8) {
        let mut byte = 0u8;
        for (shift, &bit) in byte_bits.iter().enumerate() {
            if bit > 1 {
                return Err("Invalid bitstream element: must be 0 or 1".to_string());
            }
            byte |= (bit & 1) << shift;
        }
        flat_bytes.push(byte);
    }

    let shards = flat_bytes
        .chunks_exact(shard_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    Ok(shards)
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_serialize_bits_lsb_order() {
        let interleaved_shards = vec![vec![1u8], vec![2u8]];
        let bitstream = serialize_bits(&interleaved_shards);

        assert_eq!(bitstream.len(), 16);

        let expected_bits = vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        assert_eq!(bitstream, expected_bits);
    }

    #[test]
    fn test_serialize_bits_empty_input() {
        let empty_shards: Vec<Vec<u8>> = Vec::new();
        assert!(serialize_bits(&empty_shards).is_empty());
    }

    #[test]
    fn test_bit_serialization_round_trip() {
        let original_shards = vec![
            vec![10, 20, 30, 40],
            vec![50, 60, 70, 80],
            vec![90, 100, 110, 120],
        ];

        let bitstream = serialize_bits(&original_shards);
        assert_eq!(bitstream.len(), 96);

        let reconstructed_shards = deserialize_bits(&bitstream, 4).expect("Deserialization failed");
        assert_eq!(reconstructed_shards, original_shards);
    }

    #[test]
    fn test_deserialize_handles_remainder_bytes_safely() {
        // Create 2 shards, each 2 bytes long (4 bytes total = 32 bits)
        let shards = vec![vec![11, 22], vec![33, 44]];
        let mut bitstream = serialize_bits(&shards);

        // Add 1 extra noise byte to the bitstream (8 bits of 0s)
        // Total bytes becomes 5. 5 bytes % 2 (shard_size) = 1 remainder byte left over.
        bitstream.extend(vec![0u8; 8]);

        let result = deserialize_bits(&bitstream, 2);
        assert!(result.is_ok(), "Should not fail due to remainder bytes");

        let reconstructed = result.unwrap();
        // It must cleanly discard the 5th remainder byte and only yield the 2 complete shards
        assert_eq!(reconstructed.len(), 2);
        assert_eq!(reconstructed, shards);
    }

    #[test]
    fn test_deserialize_invalid_inputs() {
        // Test non-multiple of 8 bits (Should still trigger the initial length check error)
        let broken_bits = vec![1, 0, 1];
        assert!(deserialize_bits(&broken_bits, 1).is_err());

        // Test illegal bit states (e.g. a '2' on the wire)
        let corrupt_bits = vec![0, 1, 2, 0, 0, 0, 0, 1];
        assert!(deserialize_bits(&corrupt_bits, 1).is_err());

        // Test non-multiple of 8 bits
        let broken_bits = vec![1, 0, 1];
        assert!(deserialize_bits(&broken_bits, 1).is_err());

        // Test illegal bit states (e.g. a '2' on the wire)
        let corrupt_bits = vec![0, 1, 2, 0, 0, 0, 0, 1];
        assert!(deserialize_bits(&corrupt_bits, 1).is_err());
    }
}
