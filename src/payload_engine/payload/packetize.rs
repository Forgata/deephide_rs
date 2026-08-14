use std::io::{self, Error, ErrorKind};

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
    let chunks: Vec<&[u8]> = encrypted_payload.chunks(frame_size).collect();
    let num_chunks = chunks.len();

    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut frame = Vec::with_capacity(4 + frame_size);
            let frame_id = (i as u32).to_be_bytes();
            frame.extend_from_slice(&frame_id);
            frame.extend_from_slice(chunk);

            if i == num_chunks - 1 && chunk.len() < frame_size {
                frame.extend(vec![0u8; frame_size - chunk.len()]);
            }

            frame
        })
        .collect()
}

/// Reassembles a vector of fixed-size frames back into a single continuous payload vector.
/// Automatically sorts the frames by their 4-byte big-endian ID to guarantee correct ordering.
///
/// ### Parameters
/// * `packets` - The vector of frames containing the 4-byte ID header and payload.
///
/// ### Returns
/// A byte vector representing the combined payload (with padding preserved).
///

pub fn depacketize_encrypted_payload(mut packets: Vec<Vec<u8>>) -> io::Result<Vec<u8>> {
    if packets.is_empty() {
        return Ok(Vec::new());
    }

    packets.sort_by(|a, b| {
        let id_a = u32::from_be_bytes(a[0..4].try_into().unwrap_or([0; 4]));
        let id_b = u32::from_be_bytes(b[0..4].try_into().unwrap_or([0; 4]));
        id_a.cmp(&id_b)
    });

    let total_capacity = packets.iter().map(|p| p.len().saturating_sub(4)).sum();
    let mut combined_payload = Vec::with_capacity(total_capacity);

    for packet in packets {
        if packet.len() < 4 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Packet too short to contain a 4-byte ID header",
            ));
        }

        let payload_chunk = &packet[4..];
        combined_payload.extend_from_slice(payload_chunk);
    }

    Ok(combined_payload)
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**

mod tests {
    use super::*;

    #[test]
    fn test_packetize_encrypted_payload_with_padding() {
        // Create 1034 bytes of data
        let data = vec![0u8; 1034];
        let frame_size = 512;
        let frames = packetize_encrypted_payload(&data, frame_size);

        // We expect 3 frames:
        // Frame 0: 512 bytes of data
        // Frame 1: 512 bytes of data
        // Frame 2: 10 bytes of data + 502 bytes of zero padding = 512 bytes of data
        assert_eq!(frames.len(), 3);

        // Frame 0: ID should be [0, 0, 0, 0] (Big Endian u32), total size = 4 + 512
        assert_eq!(&frames[0][0..4], &[0, 0, 0, 0]);
        assert_eq!(frames[0].len(), 516);

        // Frame 1: ID should be, total size = 4 + 512
        assert_eq!(&frames[1][0..4], &[0, 0, 0, 1]);
        assert_eq!(frames[1].len(), 516);

        // Frame 2: ID should be, total size = 4 + 512 (due to padding)
        assert_eq!(&frames[2][0..4], &[0, 0, 0, 2]);
        assert_eq!(frames[2].len(), 516);

        // Verify that the trailing bytes of the last frame are indeed zero-padded
        // 4 (ID) + 10 (remaining data) = index 14 onwards should be all zeros
        let padding_part = &frames[2][14..];
        assert!(padding_part.iter().all(|&b| b == 0));
        assert_eq!(padding_part.len(), 502);
    }

    #[test]
    fn test_packetize_and_depacketize_round_trip() {
        let original_data = vec![0xAA; 1034];
        let frame_size = 512;

        // Run forward packetization
        let packets = packetize_encrypted_payload(&original_data, frame_size);
        assert_eq!(packets.len(), 3);

        // Run inverse depacketization
        let recovered_data = depacketize_encrypted_payload(packets).unwrap();

        // 3 packets * 512 bytes per frame = 1536 bytes total recovered
        assert_eq!(recovered_data.len(), 1536);

        // Confirm original data is perfectly intact at the front
        assert_eq!(&recovered_data[0..1034], &original_data[..]);

        // Confirm the trailing padding bytes are all 0
        assert!(recovered_data[1034..].iter().all(|&b| b == 0));
    }
}
