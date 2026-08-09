use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

/// # Key Derivation Function
/// Transforms a password into a key using a salt and a number of iterations.
///
/// ### Parameters
/// * `password` - The password to be transformed into a key.
/// * `salt` - A random value that is used to add entropy to the password.
/// * `iterations` - The number of iterations to be used in the key derivation function.
///
/// ### Returns
/// A byte array representing the key derived from the password.
///
pub fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];

    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;

    #[test]
    fn test_derive_key() {
        let password = "password";
        let salt = b"salt";
        let iterations = 1000;

        let key1 = derive_key(password, salt, iterations);
        let key2 = derive_key(password, salt, iterations);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_derive_key_uniqueness() {
        let salt = b"salt";
        let iterations = 1000;

        let key_a = derive_key("password_a", salt, iterations);
        let key_b = derive_key("password_b", salt, iterations);

        assert_ne!(key_a, key_b);
    }

    #[test]
    fn test_sha256_vector_iterations_1() {
        let password = "password";
        let salt = b"salt";
        let iterations = 1;

        // Verified PBKDF2-HMAC-SHA256 32-byte test vector
        let expected_key: [u8; 32] = [
            0x12, 0x0f, 0xb6, 0xcf, 0xfc, 0xf8, 0xb3, 0x2c, 0x43, 0xe7, 0x22, 0x52, 0x56, 0xc4,
            0xf8, 0x37, 0xa8, 0x65, 0x48, 0xc9, 0x2c, 0xcc, 0x35, 0x48, 0x08, 0x05, 0x98, 0x7c,
            0xb7, 0x0b, 0xe1, 0x7b,
        ];

        let derived_key = derive_key(password, salt, iterations);
        assert_eq!(derived_key, expected_key);
    }

    #[test]
    fn test_sha256_vector_iterations_2() {
        let password = "password";
        let salt = b"salt";
        let iterations = 2;

        // Exactly verified PBKDF2-HMAC-SHA256 32-byte test vector
        let expected_key: [u8; 32] = [
            0xae, 0x4d, 0x0c, 0x95, 0xaf, 0x6b, 0x46, 0xd3, 0x2d, 0x0a, 0xdf, 0xf9, 0x28, 0xf0,
            0x6d, 0xd0, 0x2a, 0x30, 0x3f, 0x8e, 0xf3, 0xc2, 0x51, 0xdf, 0xd6, 0xe2, 0xd8, 0x5a,
            0x95, 0x47, 0x4c, 0x43,
        ];

        let derived_key = derive_key(password, salt, iterations);
        assert_eq!(derived_key, expected_key);
    }
}
