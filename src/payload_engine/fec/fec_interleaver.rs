/// # Interleave Shards
/// Reorders shards so that consecutive physical errors are distributed
/// across different logical RS blocks.
///
/// This spreads error-correction data out over time, ensuring a burst of
/// network noise doesn't wipe out an entire logical FEC block.

/// Interleaves whole blocks (shards) instead of ripping apart single bytes.
pub fn interleave(shards: &[Vec<u8>], data_shards: usize, parity_shards: usize) -> Vec<Vec<u8>> {
    if shards.is_empty() {
        return Vec::new();
    }

    let total_shards_per_block = data_shards + parity_shards;
    let num_blocks = (shards.len() + total_shards_per_block - 1) / total_shards_per_block;

    let mut interleaved: Vec<Vec<u8>> = Vec::with_capacity(shards.len());

    for col in 0..total_shards_per_block {
        for row in 0..num_blocks {
            let source_idx = row * total_shards_per_block + col;
            if source_idx < shards.len() {
                interleaved.push(shards[source_idx].clone());
            }
        }
    }
    interleaved
}

/// Reverses the interleaving process, restoring shards to their original block order.
pub fn deinterleave(
    interleaved: &[Vec<u8>],
    data_shards: usize,
    parity_shards: usize,
) -> Vec<Vec<u8>> {
    if interleaved.is_empty() {
        return Vec::new();
    }

    let total_shards_per_block = data_shards + parity_shards;

    let num_blocks = (interleaved.len() + total_shards_per_block - 1) / total_shards_per_block;

    let mut original: Vec<Vec<u8>> = vec![Vec::new(); interleaved.len()];

    let mut current_idx = 0;

    for col in 0..total_shards_per_block {
        for row in 0..num_blocks {
            let target_idx = row * total_shards_per_block + col;

            if target_idx < interleaved.len() && current_idx < interleaved.len() {
                original[target_idx] = interleaved[current_idx].clone();
                current_idx += 1;
            }
        }
    }
    original
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_interleave_successful_matrix_rearrangement() {
        // Create 6 unique whole shards (mocking 256-byte packets)
        // Data shards:, [102, 102], [103, 103]
        // Parity shards:, [202, 202], [203, 203]
        let shards = vec![
            vec![101, 101], // Block 0, Col 0 (Data 1)
            vec![102, 102], // Block 0, Col 1 (Data 2)
            vec![103, 103], // Block 0, Col 2 (Data 3)
            vec![201, 201], // Block 0, Col 3 (Parity 1)
            vec![202, 202], // Block 0, Col 4 (Parity 2)
            vec![203, 203], // Block 0, Col 5 (Parity 3)
        ];

        // 3 Data + 3 Parity = 6 Total Shards per block.
        // 6 elements total means exactly 1 block of data.
        let result = interleave(&shards, 3, 3);

        assert_eq!(result.len(), 6);

        // With 1 single block, the column-by-row traversal output order
        // must exactly mirror the original incoming shard order.
        assert_eq!(result[0], vec![101, 101]);
        assert_eq!(result[3], vec![201, 201]);
        assert_eq!(result[5], vec![203, 203]);
    }

    #[test]
    fn test_interleave_multi_block_shuffling() {
        // Simulate 2 sequential blocks of transmission data
        // Block 0 has indices 0-3. Block 1 has indices 4-7.
        // Parameters: data_shards = 2, parity_shards = 2 (Total = 4 per block)
        let shards = vec![
            vec![1],
            vec![2],
            vec![3],
            vec![4], // Block 0
            vec![5],
            vec![6],
            vec![7],
            vec![8], // Block 1
        ];

        let result = interleave(&shards, 2, 2);

        assert_eq!(result.len(), 8);

        // Expected Column-Major Reconstruction Order:
        // Col 0: Row 0 (Idx 0 -> [1]), Row 1 (Idx 4 ->)
        // Col 1: Row 0 (Idx 1 -> [2]), Row 1 (Idx 5 ->)
        // Col 2: Row 0 (Idx 2 -> [3]), Row 1 (Idx 6 ->)
        // Col 3: Row 0 (Idx 3 -> [4]), Row 1 (Idx 7 ->)
        let expected = vec![
            vec![1],
            vec![5],
            vec![2],
            vec![6],
            vec![3],
            vec![7],
            vec![4],
            vec![8],
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_interleave_incomplete_final_block() {
        // Total shards per block = 3 (2 data + 1 parity)
        // Block 0: indices 0,1,2 (Full)
        // Block 1: indices 3,4   (Incomplete, missing last parity shard)
        let shards = vec![
            vec![10],
            vec![20],
            vec![30], // Block 0
            vec![40],
            vec![50], // Block 1 (Partial)
        ];

        let result = interleave(&shards, 2, 1);

        assert_eq!(result.len(), 5);

        // Col 0: Row 0 (10), Row 1 (40)
        // Col 1: Row 0 (20), Row 1 (50)
        // Col 2: Row 0 (30), Row 1 (Out of bounds - Skipped cleanly)
        let expected = vec![vec![10], vec![40], vec![20], vec![50], vec![30]];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_interleave_empty_input() {
        let shards: Vec<Vec<u8>> = Vec::new();
        let result = interleave(&shards, 3, 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deinterleave_restores_multi_block() {
        // This is the output we expected from the multi-block interleave test
        let interleaved_shards = vec![
            vec![10],
            vec![50], // Col 0
            vec![20],
            vec![60], // Col 1
            vec![30],
            vec![70], // Col 2
            vec![40],
            vec![80], // Col 3
        ];

        // Parameters match the original test setup: 2 data, 2 parity
        let restored = deinterleave(&interleaved_shards, 2, 2);

        let expected_original = vec![
            vec![10],
            vec![20],
            vec![30],
            vec![40], // Block 0
            vec![50],
            vec![60],
            vec![70],
            vec![80], // Block 1
        ];

        assert_eq!(restored, expected_original);
    }

    #[test]
    fn test_deinterleave_incomplete_final_block() {
        // Ragged matrix output from our incomplete final block test
        let interleaved_shards = vec![
            vec![10],
            vec![40], // Col 0
            vec![20],
            vec![50], // Col 1
            vec![30], // Col 2
        ];

        let restored = deinterleave(&interleaved_shards, 2, 1);

        let expected_original = vec![
            vec![10],
            vec![20],
            vec![30], // Block 0
            vec![40],
            vec![50], // Block 1 (Partial)
        ];

        assert_eq!(restored, expected_original);
    }

    #[test]
    fn test_interleave_deinterleave_round_trip() {
        // Generate an asymmetric sample set of whole mock packets
        let original_shards = vec![
            vec![1, 1, 1],
            vec![2, 2, 2],
            vec![3, 3, 3],
            vec![4, 4, 4],
            vec![5, 5, 5],
            vec![6, 6, 6],
            vec![7, 7, 7],
            vec![8, 8, 8],
            vec![9, 9, 9], // 9 total elements, 4 per block layout = ragged edge
        ];

        // 1. Shuffle them
        let mixed = interleave(&original_shards, 2, 2);

        // 2. Un-shuffle them
        let restored = deinterleave(&mixed, 2, 2);

        // 3. Confirm exact parity matching
        assert_eq!(restored, original_shards);
    }

    #[test]
    fn test_deinterleave_empty_input() {
        let shards: Vec<Vec<u8>> = Vec::new();
        let result = deinterleave(&shards, 3, 3);
        assert!(result.is_empty());
    }
}
