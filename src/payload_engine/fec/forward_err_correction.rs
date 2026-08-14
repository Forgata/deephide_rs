use reed_solomon_erasure::galois_8::ReedSolomon;
/// Apply Reed-Solomon error correction to a list of packets.
/// Returns the list of corrected packets.
///
/// ### Parameters
/// * `packets` - A list of packets to be corrected.
/// * `data_shards` - The number of data shards in the packets.
/// * `parity_shards` - The number of parity shards in the packets.
///
/// ### Returns
/// A list of corrected packets.
///
pub fn apply_reed_solomon_fec(
    packets: &[Vec<u8>],
    data_shards: usize,
    parity_shards: usize,
) -> Result<Vec<Vec<u8>>, String> {
    if packets.is_empty() {
        return Ok(Vec::new());
    }

    let rs = ReedSolomon::new(data_shards, parity_shards)
        .map_err(|e| format!("Failed to initialize RS: {:?}", e))?;

    let shard_length = packets[0].len();
    let mut encoded_stream = Vec::new();

    for block in packets.chunks(data_shards) {
        let mut shard_block: Vec<Vec<u8>> =
            vec![vec![0u8; shard_length]; data_shards + parity_shards];

        for (j, packet) in block.iter().enumerate() {
            if packet.len() != shard_length {
                return Err("All packets must have the exact same length".to_string());
            }
            shard_block[j].copy_from_slice(packet);
        }

        rs.encode(&mut shard_block)
            .map_err(|e| format!("FEC Encoding failed: {:?}", e))?;

        encoded_stream.extend(shard_block);
    }

    Ok(encoded_stream)
}

pub fn recover_reed_solomon_fec(
    encoded_shards: &[Option<Vec<u8>>],
    data_shards: usize,
    parity_shards: usize,
) -> Result<Vec<Vec<u8>>, String> {
    if encoded_shards.is_empty() {
        return Ok(Vec::new());
    }

    let total_shards_per_block = data_shards + parity_shards;
    let rs = ReedSolomon::new(data_shards, parity_shards)
        .map_err(|e| format!("Failed to initialize RS decoder: {:?}", e))?;

    let mut recovered_packets = Vec::new();

    for block in encoded_shards.chunks(total_shards_per_block) {
        let mut shard_block: Vec<Option<Vec<u8>>> = vec![None; total_shards_per_block];
        for (i, shard) in block.iter().enumerate() {
            shard_block[i] = shard.clone();
        }

        rs.reconstruct(&mut shard_block)
            .map_err(|e| format!("FEC Reconstruction failed: {:?}", e))?;

        for i in 0..data_shards {
            if let Some(packet) = shard_block[i].take() {
                recovered_packets.push(packet);
            } else {
                return Err("Critical error: Shard was not recovered by Reed-Solomon".to_string());
            }
        }
    }

    Ok(recovered_packets)
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_apply_fec_empty_input() {
        let packets: Vec<Vec<u8>> = Vec::new();
        let result = apply_reed_solomon_fec(&packets, 3, 3).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_apply_fec_packet_grouping_and_lengths() {
        // Create 4 distinct packets, each 8 bytes long
        let packets = vec![vec![11u8; 8], vec![22u8; 8], vec![33u8; 8], vec![44u8; 8]];

        // Layout: 2 data shards, 3 parity shards
        // Block 1 will contain packets 0 and 1, plus 3 parity shards
        // Block 2 will contain packets 2 and 3, plus 3 parity shards
        let result = apply_reed_solomon_fec(&packets, 2, 3).expect("FEC generation failed");

        // Total expected packets = 2 blocks * (2 data + 3 parity) = 10 packets
        assert_eq!(result.len(), 10);

        // Every packet in the output stream must retain the 8-byte length
        for packet in &result {
            assert_eq!(packet.len(), 8);
        }

        // Verify the original systematic data packets are unchanged in their slots
        assert_eq!(result[0], vec![11u8; 8]); // Block 1, Data 1
        assert_eq!(result[1], vec![22u8; 8]); // Block 1, Data 2
        assert_eq!(result[5], vec![33u8; 8]); // Block 2, Data 1
        assert_eq!(result[6], vec![44u8; 8]); // Block 2, Data 2
    }

    #[test]
    fn test_fec_reconstruction_recovery() {
        // Setup a 3 data, 3 parity block
        let packets = vec![vec![0xAA; 16], vec![0xBB; 16], vec![0xCC; 16]];

        let encoded_stream = apply_reed_solomon_fec(&packets, 3, 3).expect("Encoding failed");
        assert_eq!(encoded_stream.len(), 6);

        // Simulate losing 3 random shards by copying the stream and clearing them out
        // We will "drop" the first data shard, the third data shard, and the second parity shard
        let mut received_block = encoded_stream.clone();
        received_block[0] = Vec::new(); // Erased Data 1
        received_block[2] = Vec::new(); // Erased Data 3
        received_block[4] = Vec::new(); // Erased Parity 2

        // Initialize the pure Rust decoder with the same parameters
        let rs = ReedSolomon::new(3, 3).unwrap();

        // Attempt reconstruction in-place
        let mut reconstructed: Vec<Option<Vec<u8>>> = received_block
            .into_iter()
            .map(|shard| if shard.is_empty() { None } else { Some(shard) })
            .collect();
        let reconstruction_result = rs.reconstruct(&mut reconstructed);
        assert!(reconstruction_result.is_ok());

        // Verify that the missing data shards were accurately rebuilt
        assert_eq!(reconstructed[0].as_ref().unwrap(), &vec![0xAA; 16]);
        assert_eq!(reconstructed[1].as_ref().unwrap(), &vec![0xBB; 16]); // Left untouched
        assert_eq!(reconstructed[2].as_ref().unwrap(), &vec![0xCC; 16]);
    }

    #[test]
    fn test_invalid_mismatched_packet_lengths() {
        // Shard lengths must be identical
        let mismatched_packets = vec![
            vec![1u8; 10],
            vec![2u8; 12], // Mismatched length
            vec![3u8; 10],
        ];

        let result = apply_reed_solomon_fec(&mismatched_packets, 3, 3);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "All packets must have the exact same length"
        );
    }

    #[test]
    fn test_fec_encode_and_decode_round_trip_with_losses() {
        // 1. Setup 3 matching data packets
        let original_packets = vec![vec![0xAA; 16], vec![0xBB; 16], vec![0xCC; 16]];
        let data_shards = 3;
        let parity_shards = 3;

        // 2. Encode them using your original function
        let encoded_stream =
            apply_reed_solomon_fec(&original_packets, data_shards, parity_shards).unwrap();
        assert_eq!(encoded_stream.len(), 6); // 3 data + 3 parity

        // 3. Simulate network loss by wrapping them in Option and dropping 2 shards
        let mut received_stream: Vec<Option<Vec<u8>>> =
            encoded_stream.into_iter().map(Some).collect();

        received_stream[0] = None; // Drop first data shard
        received_stream[4] = None; // Drop second parity shard

        // 4. Run recovery reverse function
        let recovered =
            recover_reed_solomon_fec(&received_stream, data_shards, parity_shards).unwrap();

        // 5. Assert the parity shards were stripped and the missing data shard was perfectly restored
        assert_eq!(recovered.len(), 3);
        assert_eq!(recovered[0], vec![0xAA; 16]);
        assert_eq!(recovered[1], vec![0xBB; 16]);
        assert_eq!(recovered[2], vec![0xCC; 16]);
    }
}
