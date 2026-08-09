use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use rand::Rng;

pub fn aes_gcm_encrypt(key: &[u8], payload: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err("Invalid key length".to_string());
    }

    let mut nonce = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce);

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "Invalid key length")?;
    let cipher_text = cipher
        .encrypt(&nonce.into(), payload)
        .map_err(|e| format!("Encryption failed {}", e))?;

    let mut result = Vec::with_capacity(nonce.len() + cipher_text.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&cipher_text);

    Ok(result)
}

pub fn aes_gcm_decrypt(key: &[u8], encrypted_payload: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted_payload.len() < 28 {
        return Err("Encrypted packet to short".to_string());
    }

    let (nonce_bytes, cipher_text) = encrypted_payload.split_at(12);
    let nonce = Nonce::try_from(nonce_bytes).map_err(|_| "Invalid nonce length".to_string())?;

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "Invlaid key length".to_string())?;
    cipher
        .decrypt(&nonce, cipher_text)
        .map_err(|_| "Authentication failed: Invalid key or tampered data".to_string())
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_encryption_success() {
        let key = [0u8; 32]; // Valid 32-byte key
        let payload = b"Hello, RustCrypto!";

        let result = aes_gcm_encrypt(&key, payload);
        assert!(result.is_ok());

        let encrypted_data = result.unwrap();

        // Expected length: 12 (nonce) + payload length + 16 (auth tag)
        let expected_len = 12 + payload.len() + 16;
        assert_eq!(encrypted_data.len(), expected_len);
    }

    #[test]
    fn test_invalid_key_length() {
        let short_key = [0u8; 16]; // Invalid length
        let payload = b"Secret data";

        let result = aes_gcm_encrypt(&short_key, payload);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid key length");
    }

    #[test]
    fn test_nonce_randomness_produces_unique_ciphertexts() {
        let key = [1u8; 32];
        let payload = b"Same exact payload";

        // Two sequential runs with the exact same key and payload
        let run_1 = aes_gcm_encrypt(&key, payload).unwrap();
        let run_2 = aes_gcm_encrypt(&key, payload).unwrap();

        // They must NOT be identical because the nonces must be completely random
        assert_ne!(run_1, run_2);

        // Ensure even the extracted nonces (first 12 bytes) are distinct
        assert_ne!(&run_1[..12], &run_2[..12]);
    }
}
