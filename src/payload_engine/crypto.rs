mod aes_gcm_encryption;
mod derive_key;

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**

mod tests {
    use super::*;
    use aes_gcm_encryption::{aes_gcm_decrypt, aes_gcm_encrypt};
    use derive_key::derive_key;

    #[test]
    fn test_end_to_end_crypto_pipeline() {
        let correct_password = "super_secret_password";
        let wrong_password = "wrong_password";
        let salt = b"static_salt_for_testing";
        let iterations = 1000;
        let payload = b"Sensitive payload data string";

        // 1. Derive the correct key
        let correct_key = derive_key(correct_password, salt, iterations);

        // 2. Encrypt the payload using the correct key
        let encrypted_packet =
            aes_gcm_encrypt(&correct_key, payload).expect("Encryption should succeed");

        // 3. Decrypt using the CORRECT key -> Should match original payload
        let decrypted_payload = aes_gcm_decrypt(&correct_key, &encrypted_packet)
            .expect("Decryption should succeed with correct key");
        assert_eq!(decrypted_payload, payload);

        // 4. Derive an incorrect key using the wrong password
        let wrong_key = derive_key(wrong_password, salt, iterations);

        // 5. Attempt decryption with WRONG key -> Must fail authentication!
        let decryption_result = aes_gcm_decrypt(&wrong_key, &encrypted_packet);
        assert!(decryption_result.is_err());
        assert_eq!(
            decryption_result.unwrap_err(),
            "Authentication failed: Invalid key or tampered data"
        );
    }
}
