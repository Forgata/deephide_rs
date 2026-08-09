/// Takes an encrypted payload and splits it into frames of a given size.
/// Returns a vector of frames, where each frame is a vector of bytes.
/// Each frame has a 4-byte ID followed by the encrypted payload.
/// The ID is a big-endian u32 representing the index of the frame.
///
/// ### Parameters
/// * `encrypted_payload` - The encrypted payload to be split into frames.
/// * `frame_size` - The size of each frame in bytes.
///
/// ### Returns
/// A vector of frames, where each frame is a vector of bytes.

pub fn packetize_encrypted_payload(encrypted_payload: &[u8], frame_size: usize) -> Vec<Vec<u8>> {
    encrypted_payload
        .chunks(frame_size)
        .enumerate()
        .map(|(i, chunk)| {
            let mut frame = Vec::with_capacity(4 + chunk.len());
            let frame_id = (i as u32).to_be_bytes();
            frame.extend_from_slice(&frame_id);
            frame.extend_from_slice(chunk);

            frame
        })
        .collect()
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**

mod tests {
    use super::*;

    #[test]
    fn test_packetize_encrypted_payload() {
        // Create 1034 bytes of data (0u8 is the value, 1034 is the count)
        let data = vec![0u8; 1034];
        let frames = packetize_encrypted_payload(&data, 512);

        // We expect 3 frames:, [512], and [10]
        assert_eq!(frames.len(), 3);

        // Frame 0: ID should be [0, 0, 0, 0] (Big Endian u32)
        assert_eq!(&frames[0][0..4], &[0, 0, 0, 0]);
        assert_eq!(frames[0].len(), 516); // 4 + 512

        // Frame 1: ID should be [0, 0, 0, 1]
        assert_eq!(&frames[1][0..4], &[0, 0, 0, 1]);

        // Frame 2: ID should be [0, 0, 0, 2]
        assert_eq!(&frames[2][0..4], &[0, 0, 0, 2]);
        assert_eq!(frames[2].len(), 14); // 4 + 10 (Remainder)
    }
}
