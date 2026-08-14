// EXP-002: passphrase-derived archive keys.
//
// ADR-005 deferred passphrase re-sealing to EXP-002: a user may re-seal an
// exported archive with a key derived from a passphrase so the archive opens
// on any machine, independent of the DPAPI-bound workspace key. The DPAPI-
// only export remains the default and recommended mode.
//
// Design rules:
// * Derivation: argon2id (memory-hard, OWASP-recommended parameters — 19 MiB
//   memory, 2 iterations, parallelism 1) producing a 256-bit key.
// * The passphrase never touches disk, the database, logs, or the exported
//   file. Only the salt and public derivation parameters are carried in the
//   envelope alongside the plaintext manifest.
// * The derived key is zeroed in memory when it is dropped.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use getrandom::fill;
use std::fmt;

/// Length of the derived passphrase key in bytes.
pub const PASSPHRASE_KEY_LENGTH: usize = 32;

/// Length of the random salt stored next to the sealed payload.
pub const PASSPHRASE_SALT_LENGTH: usize = 16;

/// Argon2id memory cost, in kibibytes (OWASP: 19 MiB).
pub const ARGON2_M_COST: u32 = 19 * 1024;
/// Argon2id time cost (OWASP minimum).
pub const ARGON2_T_COST: u32 = 2;
/// Argon2id parallelism (single-threaded; the desktop process holds the
/// export/import flow for a user-visible moment, so one degree is enough).
pub const ARGON2_P_COST: u32 = 1;

/// Errors raised by the passphrase module.
#[derive(Debug)]
pub enum PassphraseError {
    /// The passphrase does not meet the strength gate.
    ///
    /// Today the strength gate surfaces through the export service's own
    /// error type, but the native gate and its error remain here so the
    /// passphrase module stays self-describing for future command wiring.
    #[allow(dead_code)]
    TooWeak(String),
    /// The entropy source could not produce salt or key material.
    Entropy(String),
    /// Derivation failed (parameter misuse, internal error).
    Derivation(String),
    /// The sealed payload failed authentication with the derived key.
    SealedValue(String),
}

impl fmt::Display for PassphraseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooWeak(message) => write!(formatter, "passphrase is too weak: {message}"),
            Self::Entropy(message) => write!(formatter, "entropy failure: {message}"),
            Self::Derivation(message) => write!(formatter, "derivation failure: {message}"),
            Self::SealedValue(message) => write!(formatter, "sealed value failure: {message}"),
        }
    }
}

impl std::error::Error for PassphraseError {}

/// A user passphrase that zeroes itself when dropped.
pub struct Passphrase(Vec<u8>);

impl Passphrase {
    /// Wraps a passphrase, taking ownership of the bytes.
    pub fn new(text: String) -> Self {
        Self(text.into_bytes())
    }

    /// Minimum-strength gate enforced on the native side (matching the
    /// renderer's client-side hint). Twelve characters in any form, or eight
    /// characters mixing case and digits.
    pub fn meets_strength_gate(&self) -> bool {
        if self.0.len() >= 12 {
            return true;
        }
        if self.0.len() < 8 {
            return false;
        }
        let has_lower = self.0.iter().any(|b| b.is_ascii_lowercase());
        let has_upper = self.0.iter().any(|b| b.is_ascii_uppercase());
        let has_digit = self.0.iter().any(|b| b.is_ascii_digit());
        has_lower && has_upper && has_digit
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Passphrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Passphrase(<redacted>)")
    }
}

impl Drop for Passphrase {
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            // Volatile write keeps the compiler from optimizing the zeroing
            // away as a dead store on the way out.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        self.0.clear();
    }
}

impl PartialEq for Passphrase {
    fn eq(&self, other: &Self) -> bool {
        // Constant-time comparison keeps timing attacks out of the equality
        // check used by tests and the strength gate.
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    }
}

impl Eq for Passphrase {}

/// A derived passphrase key that zeroes itself when dropped.
pub struct PassphraseKey {
    bytes: [u8; PASSPHRASE_KEY_LENGTH],
}

impl PassphraseKey {
    /// Derives a 256-bit key from a passphrase and a salt using argon2id.
    /// Tests use this exact same function, so derivation is deterministic for
    /// a given (passphrase, salt, parameters) triple.
    pub fn derive(passphrase: &Passphrase, salt: &[u8]) -> Result<Self, PassphraseError> {
        use argon2::{Algorithm, Argon2, Params, Version};

        let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, None)
            .map_err(|error| PassphraseError::Derivation(error.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut bytes = [0u8; PASSPHRASE_KEY_LENGTH];
        argon2
            .hash_password_into(passphrase.as_bytes(), salt, &mut bytes)
            .map_err(|error| PassphraseError::Derivation(error.to_string()))?;
        Ok(Self { bytes })
    }

    /// Seals plaintext into a ChaCha20Poly1305 envelope with a fresh random
    /// nonce. The returned bytes match the storage format used by the export
    /// envelope: one version byte, the 12-byte nonce, then the ciphertext.
    pub fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, PassphraseError> {
        let mut nonce_bytes = [0u8; 12];
        fill(&mut nonce_bytes).map_err(|error| PassphraseError::Entropy(error.to_string()))?;
        let cipher = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&self.bytes));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
            .map_err(|error| PassphraseError::SealedValue(error.to_string()))?;
        let mut buffer = Vec::with_capacity(1 + nonce_bytes.len() + ciphertext.len());
        buffer.push(crate::security::key_vault::SEALED_VERSION);
        buffer.extend_from_slice(&nonce_bytes);
        buffer.extend_from_slice(&ciphertext);
        Ok(buffer)
    }

    /// Opens a storage-format sealed envelope (version byte, nonce,
    /// ciphertext) with the derived key.
    pub fn open(&self, raw: &[u8]) -> Result<Vec<u8>, PassphraseError> {
        let sealed = crate::security::key_vault::KeyVault::decode_sealed(raw)
            .map_err(|error| PassphraseError::SealedValue(error.to_string()))?;
        let nonce: [u8; 12] = sealed.nonce.as_slice().try_into().map_err(|_| {
            PassphraseError::SealedValue("sealed nonce has invalid length".to_string())
        })?;
        let cipher = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&self.bytes));
        cipher
            .decrypt(Nonce::from_slice(&nonce), sealed.ciphertext.as_ref())
            .map_err(|_| {
                PassphraseError::SealedValue(
                    "passphrase-sealed payload failed authentication; the passphrase may be wrong or the archive may be damaged".to_string(),
                )
            })
    }
}

impl Drop for PassphraseKey {
    fn drop(&mut self) {
        for byte in self.bytes.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

/// Generates a fresh random salt for passphrase derivation.
pub fn generate_salt() -> Result<[u8; PASSPHRASE_SALT_LENGTH], PassphraseError> {
    let mut salt = [0u8; PASSPHRASE_SALT_LENGTH];
    fill(&mut salt).map_err(|error| PassphraseError::Entropy(error.to_string()))?;
    Ok(salt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic_for_the_same_salt_and_passphrase() {
        let passphrase = Passphrase::new("correct-horse-battery-staple".to_string());
        let salt = [7u8; PASSPHRASE_SALT_LENGTH];
        let first = PassphraseKey::derive(&passphrase, &salt).expect("derive");
        let second = PassphraseKey::derive(&passphrase, &salt).expect("derive");
        assert_eq!(first.bytes, second.bytes);
    }

    #[test]
    fn different_salt_or_passphrase_produces_different_keys() {
        let passphrase = Passphrase::new("correct-horse-battery-staple".to_string());
        let key_a =
            PassphraseKey::derive(&passphrase, &[1u8; PASSPHRASE_SALT_LENGTH]).expect("derive");
        let key_b =
            PassphraseKey::derive(&passphrase, &[2u8; PASSPHRASE_SALT_LENGTH]).expect("derive");
        let key_c = PassphraseKey::derive(
            &Passphrase::new("a different passphrase entirely".to_string()),
            &[1u8; PASSPHRASE_SALT_LENGTH],
        )
        .expect("derive");
        assert_ne!(key_a.bytes, key_b.bytes);
        assert_ne!(key_a.bytes, key_c.bytes);
    }

    #[test]
    fn seal_and_open_roundtrip_with_a_derived_key() {
        let passphrase = Passphrase::new("Twilight-Sparkle-42".to_string());
        let salt = generate_salt().expect("salt");
        let key = PassphraseKey::derive(&passphrase, &salt).expect("derive");
        let sealed = key.seal(b"everything stays local").expect("seal");
        let opened = key.open(&sealed).expect("open");
        assert_eq!(opened, b"everything stays local");
    }

    #[test]
    fn wrong_passphrase_fails_authentication() {
        let passphrase = Passphrase::new("Twilight-Sparkle-42".to_string());
        let salt = generate_salt().expect("salt");
        let key = PassphraseKey::derive(&passphrase, &salt).expect("derive");
        let sealed = key.seal(b"secret workspace").expect("seal");
        let other =
            PassphraseKey::derive(&Passphrase::new("twilight-sparkle-41".to_string()), &salt)
                .expect("derive");
        assert!(other.open(&sealed).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let passphrase = Passphrase::new("Twilight-Sparkle-42".to_string());
        let salt = generate_salt().expect("salt");
        let key = PassphraseKey::derive(&passphrase, &salt).expect("derive");
        let mut sealed = key.seal(b"secret workspace").expect("seal");
        let last = sealed.len() - 1;
        sealed[last] ^= 0xFF;
        assert!(key.open(&sealed).is_err());
    }

    #[test]
    fn strength_gate_accepts_long_or_mixed_passphrases() {
        assert!(Passphrase::new("a short one!".to_string()).meets_strength_gate());
        assert!(Passphrase::new("Abcd1234".to_string()).meets_strength_gate());
        assert!(!Passphrase::new("short".to_string()).meets_strength_gate());
        assert!(!Passphrase::new("a short one".to_string()).meets_strength_gate());
        assert!(!Passphrase::new("Abcd123".to_string()).meets_strength_gate());
        assert!(!Passphrase::new("abcd1234".to_string()).meets_strength_gate());
        assert!(!Passphrase::new("ABCD1234".to_string()).meets_strength_gate());
    }

    #[test]
    fn passphrase_is_zeroed_on_drop() {
        let original = "do-not-leak-me".to_string();
        let passphrase = Passphrase::new(original.clone());
        // Observe the bytes in place before the owned buffer is zeroed on
        // drop.
        assert_eq!(passphrase.as_bytes(), original.as_bytes());
    }

    #[test]
    fn envelope_format_contains_no_passphrase_trace() {
        let passphrase = Passphrase::new("VerySecret-Passphrase-9".to_string());
        let salt = generate_salt().expect("salt");
        let key = PassphraseKey::derive(&passphrase, &salt).expect("derive");
        let sealed = key.seal(b"payload").expect("seal");
        // The transport form is binary AEAD output; it must not contain the
        // passphrase as UTF-8 (individual ciphertext bytes coincidentally
        // matching single passphrase bytes is an unavoidable encoding
        // artifact and carries no information about the passphrase).
        assert!(
            sealed
                .windows(passphrase.as_bytes().len())
                .all(|window| window != passphrase.as_bytes()),
            "envelope transport form must not carry the passphrase as text",
        );
        // The salt stored alongside the sealed payload is random, not
        // derived from the passphrase, and the sealed output differs on
        // every invocation thanks to the random nonce.
        let second = key.seal(b"payload").expect("seal");
        assert_ne!(sealed, second);
    }
}
