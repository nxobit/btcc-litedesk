use aes::Aes256;
use anyhow::anyhow;
use base64::Engine as _;
use base64::engine::general_purpose;
use cbc::{Decryptor, Encryptor};
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha512;

pub struct PBEWithHmacSha512AndAes256 {
    password: Vec<u8>,
}

impl PBEWithHmacSha512AndAes256 {
    pub const LEGACY_ITERATIONS: u32 = 1_000;
    pub const CURRENT_ITERATIONS: u32 = 20_000;
    const CURRENT_VERSION_PREFIX: &'static str = "v2";

    #[must_use]
    pub fn new(password: &str) -> Self {
        Self {
            password: password.as_bytes().to_vec(),
        }
    }

    #[must_use]
    pub fn is_legacy_ciphertext(encrypted: &str) -> bool {
        !encrypted.trim().starts_with(Self::CURRENT_VERSION_PREFIX)
    }

    pub fn encrypt_str(&self, plaintext: &str) -> String {
        self.encrypt(plaintext.as_bytes())
    }

    pub fn decrypt_to_string(&self, encrypted: &str) -> String {
        let decrypted_bytes = self.decrypt(encrypted);
        String::from_utf8(decrypted_bytes).expect("UTF-8 decode error")
    }

    pub fn decrypt_to_string_result(&self, encrypted: &str) -> Result<String, anyhow::Error> {
        let decrypted_bytes = self.decrypt_result(encrypted)?;
        String::from_utf8(decrypted_bytes)
            .map_err(|err| anyhow!("decrypted plaintext is not UTF-8: {err}"))
    }

    pub fn decrypt_result(&self, encrypted: &str) -> Result<Vec<u8>, anyhow::Error> {
        let (payload, iterations) = Self::split_ciphertext_metadata(encrypted)?;
        let bytes = general_purpose::STANDARD
            .decode(payload)
            .map_err(|err| anyhow!("base64 decode failed: {err}"))?;
        Self::decrypt_with_iterations(&self.password, &bytes, iterations)
    }

    fn encrypt(&self, plaintext: &[u8]) -> String {
        let mut salt = [0u8; 16];
        let mut iv = [0u8; 16];
        let mut rng = rand::rngs::OsRng;
        rng.fill_bytes(&mut salt);
        rng.fill_bytes(&mut iv);

        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha512>(
            &self.password,
            &salt,
            Self::CURRENT_ITERATIONS,
            &mut key,
        );

        let cipher =
            Encryptor::<Aes256>::new_from_slices(&key, &iv).expect("invalid key or iv length");

        let block_size = 16;
        let padded_len = ((plaintext.len() / block_size) + 1) * block_size;
        let mut buffer = vec![0u8; padded_len];
        buffer[..plaintext.len()].copy_from_slice(plaintext);

        let ciphertext = cipher
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
            .expect("encryption failed");

        let mut result = Vec::with_capacity(16 + 16 + ciphertext.len());
        result.extend_from_slice(&salt);
        result.extend_from_slice(&iv);
        result.extend_from_slice(ciphertext);

        format!(
            "{}${}${}",
            Self::CURRENT_VERSION_PREFIX,
            Self::CURRENT_ITERATIONS,
            general_purpose::STANDARD.encode(result)
        )
    }

    fn decrypt(&self, encrypted: &str) -> Vec<u8> {
        self.decrypt_result(encrypted)
            .expect("decryption or unpad failed")
    }

    fn split_ciphertext_metadata(encrypted: &str) -> Result<(&str, u32), anyhow::Error> {
        let trimmed = encrypted.trim();
        if let Some(rest) = trimmed.strip_prefix(Self::CURRENT_VERSION_PREFIX) {
            let rest = rest
                .strip_prefix('$')
                .ok_or_else(|| anyhow!("invalid ciphertext version header"))?;
            let mut parts = rest.splitn(2, '$');
            let iterations = parts
                .next()
                .ok_or_else(|| anyhow!("missing iteration count"))?
                .parse::<u32>()
                .map_err(|err| anyhow!("invalid iteration count: {err}"))?;
            let payload = parts
                .next()
                .ok_or_else(|| anyhow!("missing ciphertext payload"))?;
            return Ok((payload, iterations));
        }
        Ok((trimmed, Self::LEGACY_ITERATIONS))
    }

    fn decrypt_with_iterations(
        password: &[u8],
        bytes: &[u8],
        iterations: u32,
    ) -> Result<Vec<u8>, anyhow::Error> {
        if bytes.len() < 32 {
            anyhow::bail!("ciphertext is too short");
        }
        let (salt, rest) = bytes.split_at(16);
        let (iv, ciphertext) = rest.split_at(16);

        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha512>(password, salt, iterations, &mut key);

        let cipher = Decryptor::<Aes256>::new_from_slices(&key, iv)
            .map_err(|err| anyhow!("create decryptor failed: {err}"))?;
        let mut buffer = ciphertext.to_vec();
        let decrypted = cipher
            .decrypt_padded_mut::<Pkcs7>(&mut buffer)
            .map_err(|err| anyhow!("decrypt/unpad failed: {err}"))?;
        Ok(decrypted.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::PBEWithHmacSha512AndAes256;
    use aes::Aes256;
    use base64::Engine as _;
    use base64::engine::general_purpose;
    use cbc::Encryptor;
    use cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha512;

    #[test]
    fn encrypt_uses_current_versioned_format() {
        let pbe = PBEWithHmacSha512AndAes256::new("abc123");
        let encrypted = pbe.encrypt_str("secret");
        assert!(encrypted.starts_with("v2$20000$"));
        assert_eq!(pbe.decrypt_to_string_result(&encrypted).unwrap(), "secret");
    }

    #[test]
    fn decrypt_supports_legacy_ciphertext() {
        let plaintext = "secret";
        let salt = [7u8; 16];
        let iv = [9u8; 16];
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha512>(
            b"abc123",
            &salt,
            PBEWithHmacSha512AndAes256::LEGACY_ITERATIONS,
            &mut key,
        );
        let cipher = Encryptor::<Aes256>::new_from_slices(&key, &iv).unwrap();
        let block_size = 16;
        let padded_len = ((plaintext.len() / block_size) + 1) * block_size;
        let mut buffer = vec![0u8; padded_len];
        buffer[..plaintext.len()].copy_from_slice(plaintext.as_bytes());
        let ciphertext = cipher
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
            .unwrap();

        let mut payload = Vec::new();
        payload.extend_from_slice(&salt);
        payload.extend_from_slice(&iv);
        payload.extend_from_slice(ciphertext);
        let legacy = general_purpose::STANDARD.encode(payload);

        let pbe = PBEWithHmacSha512AndAes256::new("abc123");
        assert!(PBEWithHmacSha512AndAes256::is_legacy_ciphertext(&legacy));
        assert_eq!(pbe.decrypt_to_string_result(&legacy).unwrap(), "secret");
    }
}
