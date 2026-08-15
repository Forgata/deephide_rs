/// Expands a packed byte slice (8 bits per element) into a flat bitstream vector
/// where each element is a literal byte representation of a single bit (0 or 1).
/// This prepares your Phase 1 crypto payload for the Phase 2 DSSS spreader.
pub fn bytes_to_bits(bytes: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    for &byte in bytes {
        for i in (0..8).rev() {
            let bit = (byte >> i) & 1;
            bits.push(bit);
        }
    }
    bits
}

/// Condenses a flat bitstream slice (literal 0s and 1s) back into a packed byte slice.
/// Writes directly into a caller-provided mutable target buffer to maintain zero runtime allocations.
/// This processes the output of your extraction engine back into packed bytes for Phase 1 decryption.
pub fn bits_to_bytes(bits: &[u8], bytes_out: &mut [u8]) {
    assert_eq!(
        bits.len() / 8,
        bytes_out.len(),
        "Target byte buffer size must exactly match the bitstream capacity!"
    );

    bytes_out.fill(0);

    for (bit_idx, &bit) in bits.iter().enumerate() {
        let byte_idx = bit_idx / 8;
        let bit_position = 7 - (bit_idx % 8);

        if bit == 1 {
            bytes_out[byte_idx] |= 1 << bit_position;
        }
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_bit_byte_symmetrical_routing() {
        // 1. Setup a complex mock packed byte payload matrix
        let original_bytes: [u8; 4] = [0xAA, 0xFF, 0x00, 0x42]; // Binary: 10101010, 11111111, 00000000, 01000010

        // 2. Unpack bytes to a flat bit stream
        let flat_bits = bytes_to_bits(&original_bytes);
        assert_eq!(
            flat_bits.len(),
            32,
            "Bitstream size should be 4 bytes * 8 bits = 32!"
        );

        // Assert specific sample bits to confirm order preservation
        assert_eq!(flat_bits[0], 1); // 0xAA first bit
        assert_eq!(flat_bits[1], 0); // 0xAA second bit
        assert_eq!(flat_bits[8], 1); // 0xFF first bit
        assert_eq!(flat_bits[16], 0); // 0x00 first bit

        // 3. Condense flat bits back to a packed byte target buffer
        let mut reconstructed_bytes = vec![0u8; 4];
        bits_to_bytes(&flat_bits, &mut reconstructed_bytes);

        // 4. Verify full structural reconstruction loopback match
        assert_eq!(
            reconstructed_bytes, original_bytes,
            "Data routing utility corrupted bit alignments across packing boundaries!"
        );
    }
}
