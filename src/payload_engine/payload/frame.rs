use std::io::{self, Error, ErrorKind};

pub fn frame_payload(file_bytes: &[u8], filename: &str) -> io::Result<Vec<u8>> {
    let filename_bytes = filename.as_bytes();
    if filename_bytes.len() > 255 {
        return Err(Error::new(ErrorKind::InvalidInput, "Filename too long"));
    }

    const HEADER_SIZE: usize = 10;
    let total_size = HEADER_SIZE + filename_bytes.len() + file_bytes.len();

    let mut packet = Vec::with_capacity(total_size);

    packet.extend_from_slice(&0x44484944u32.to_be_bytes());
    packet.push(0x01);
    packet.push(filename_bytes.len() as u8);

    let file_len = file_bytes.len() as u32;

    packet.extend_from_slice(&file_len.to_be_bytes());
    packet.extend_from_slice(filename_bytes);
    packet.extend_from_slice(file_bytes);

    Ok(packet)
}

#[derive(Debug, PartialEq)]
pub struct FramedData {
    pub filename: String,
    pub file_bytes: Vec<u8>,
}

pub fn deframe_payload(packet: &[u8]) -> io::Result<FramedData> {
    const HEADER_SIZE: usize = 10;

    if packet.len() < HEADER_SIZE {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Packet too short for header",
        ));
    }

    if &packet[0..4] != 0x44484944u32.to_be_bytes() {
        return Err(Error::new(ErrorKind::InvalidData, "Invalid magic bytes"));
    }

    let filename_len = packet[5] as usize;

    let file_len = u32::from_be_bytes(packet[6..10].try_into().unwrap()) as usize;

    let expected_total_size = HEADER_SIZE + filename_len + file_len;
    if packet.len() < expected_total_size {
        return Err(Error::new(
            ErrorKind::UnexpectedEof,
            "Packet data is truncated",
        ));
    }

    let filename_start = HEADER_SIZE;
    let filename_end = filename_start + filename_len;
    let filename_bytes = &packet[filename_start..filename_end];
    let filename = String::from_utf8(filename_bytes.to_vec()).map_err(|e| {
        Error::new(
            ErrorKind::InvalidData,
            format!("Invalid UTF-8 filename: {}", e),
        )
    })?;

    let file_start = filename_end;
    let file_end = file_start + file_len;
    let file_bytes = packet[file_start..file_end].to_vec();

    Ok(FramedData {
        filename,
        file_bytes,
    })
}
#[cfg(test)]

/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_frame_payload_success() {
        let file_bytes = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let filename = "test.txt";

        let result = frame_payload(&file_bytes, filename);
        assert!(result.is_ok(), "Framing should succeed");

        let packet = result.unwrap();

        // Total size: 10 (header) + 8 (filename) + 4 (payload) = 22 bytes
        assert_eq!(packet.len(), 22);

        // Verify Magic Number (0-3): 0x44484944 ("DHID")
        assert_eq!(&packet[0..4], &[0x44, 0x48, 0x49, 0x44]);

        // Verify Version (4): 0x01
        assert_eq!(packet[4], 0x01);

        // Verify Filename Length (5): 8 bytes
        assert_eq!(packet[5], 8);

        // Verify Payload Length (6-9): 4 bytes -> Big-Endian [0, 0, 0, 4]
        assert_eq!(&packet[6..10], &[0, 0, 0, 4]);

        // Verify Filename payload (10..18)
        assert_eq!(&packet[10..18], "test.txt".as_bytes());

        // Verify File bytes payload (18..22)
        assert_eq!(&packet[18..22], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_filename_at_maximum_length() {
        // Create a filename exactly 255 bytes long
        let max_filename = "a".repeat(255);
        let file_bytes = vec![0x00];

        let result = frame_payload(&file_bytes, &max_filename);
        assert!(result.is_ok(), "255 byte filename should be allowed");

        let packet = result.unwrap();
        assert_eq!(packet[5], 255); // Length field fits in u8
    }

    #[test]
    fn test_filename_too_long_errors() {
        // Create a filename 256 bytes long (exceeds u8 limit)
        let invalid_filename = "a".repeat(256);
        let file_bytes = vec![0x00];

        let result = frame_payload(&file_bytes, &invalid_filename);
        assert!(result.is_err(), "Should return an error for long filenames");

        let error = result.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), "Filename too long");
    }

    #[test]
    fn test_empty_payload_and_filename() {
        let file_bytes = vec![];
        let filename = "";

        let result = frame_payload(&file_bytes, filename);
        assert!(result.is_ok());

        let packet = result.unwrap();
        assert_eq!(packet.len(), 10); // Only the header size
        assert_eq!(packet[5], 0); // Filename len = 0
        assert_eq!(&packet[6..10], &[0, 0, 0, 0]); // Payload len = 0
    }

    #[test]
    fn test_round_trip_framing() {
        let original_file = vec![0x11, 0x22, 0x33, 0x44];
        let original_name = "document.pdf";

        // Frame it
        let framed = frame_payload(&original_file, original_name).unwrap();

        // Deframe it
        let deframed = deframe_payload(&framed).unwrap();

        // Assert they match perfectly
        assert_eq!(deframed.filename, original_name);
        assert_eq!(deframed.file_bytes, original_file);
    }
}
