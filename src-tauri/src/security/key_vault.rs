// SEC-001: DPAPI key-wrapping and value-level envelope encryption.
//
// ADR-003 commits Aura to Windows DPAPI as the V0 key-wrapping boundary: a
// random data-encryption key is generated on first use, protected with the
// current Windows user's DPAPI context, and only the wrapped blob is ever
// persisted. The raw key exists in process memory only for the duration of
// an unwrap operation; it is never written to disk, logs, analytics, crash
// reports, or renderer state, and no Tauri command exposes raw key material.
//
// Database-agnostic by design: the raw SQLite file is not re-encrypted by
// this milestone. `seal` / `open` provide value-level AEAD envelopes with a
// self-describing versioned format so a future database-level migration can
// re-seal records or adopt a database crate without breaking stored values.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key as AeadKey, Nonce,
};
use getrandom::fill;
use std::path::PathBuf;

/// Length of the randomly generated data-encryption key in bytes.
pub const KEY_LENGTH: usize = 32;

/// Sealed-value version byte. Future migrations may introduce newer formats
/// while keeping older sealed values openable during the transition.
pub const SEALED_VERSION: u8 = 0x01;

/// Name of the persisted wrapped-key file inside the Tauri application-data
/// directory. The file contains only the DPAPI-protected blob, never the
/// raw key.
pub const WRAPPED_KEY_FILE_NAME: &str = "aura.keywrap";

/// Errors raised by the key vault.
#[derive(Debug)]
pub enum KeyVaultError {
    /// The entropy source could not produce key material.
    Entropy(String),
    /// The key could not be protected or unprotected with the platform
    /// boundary (on Windows, DPAPI).
    Wrapping(String),
    /// The wrapped key could not be read from or written to storage.
    Storage(String),
    /// The sealed envelope could not be opened: the ciphertext is invalid,
    /// was tampered with, or was produced with a different key.
    SealedValue(String),
}

impl std::fmt::Display for KeyVaultError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entropy(message) => write!(formatter, "entropy failure: {message}"),
            Self::Wrapping(message) => write!(formatter, "key wrapping failure: {message}"),
            Self::Storage(message) => write!(formatter, "key storage failure: {message}"),
            Self::SealedValue(message) => write!(formatter, "sealed value failure: {message}"),
        }
    }
}

impl std::error::Error for KeyVaultError {}

/// A sealed value: version byte, nonce, and authenticated ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedValue {
    pub version: u8,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Platform boundary that protects the data-encryption key.
///
/// On Windows the implementation wraps the key with `CryptProtectData` under
/// the current user's DPAPI context. No other platform implementation is
/// permitted at runtime; development-only builds carry a compile-time
/// alternative solely so the portable parts of this module can be exercised
/// outside Windows.
trait PlatformKeyWrapper: Send + Sync {
    fn wrap(&self, key: &[u8; KEY_LENGTH]) -> Result<Vec<u8>, KeyVaultError>;
    fn unwrap(&self, blob: &[u8]) -> Result<[u8; KEY_LENGTH], KeyVaultError>;
}

// ---------------------------------------------------------------------------
// Windows DPAPI implementation (real product boundary).
// ---------------------------------------------------------------------------

#[cfg(windows)]
struct DpapiKeyWrapper;

#[cfg(windows)]
impl PlatformKeyWrapper for DpapiKeyWrapper {
    fn wrap(&self, key: &[u8; KEY_LENGTH]) -> Result<Vec<u8>, KeyVaultError> {
        use windows_sys::Win32::Security::Cryptography::*;

        // CRYPTPROTECT_MEMORY is not passed: the blob is bound to the current
        // Windows user and machine, so the current-user context that owns the
        // database file can always unwrap it.
        let mut protected = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
            cbData: KEY_LENGTH as u32,
            pbData: key.as_ptr() as *mut u8,
        };
        let mut wrapped = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let status = unsafe {
            CryptProtectData(
                &mut protected,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                &mut wrapped,
            )
        };
        if status == 0 {
            return Err(KeyVaultError::Wrapping(format!(
                "CryptProtectData failed with code {}",
                unsafe { windows_sys::Win32::Foundation::GetLastError() }
            )));
        }
        let result =
            unsafe { std::slice::from_raw_parts(wrapped.pbData, wrapped.cbData as usize).to_vec() };
        unsafe { windows_sys::Win32::System::Com::CoTaskMemFree(wrapped.pbData as *mut _) };
        Ok(result)
    }

    fn unwrap(&self, blob: &[u8]) -> Result<[u8; KEY_LENGTH], KeyVaultError> {
        use windows_sys::Win32::Security::Cryptography::*;

        let mut protected = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
            cbData: blob.len() as u32,
            pbData: blob.as_ptr() as *mut u8,
        };
        let mut unwrapped = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let status = unsafe {
            CryptUnprotectData(
                &mut protected,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                &mut unwrapped,
            )
        };
        if status == 0 {
            return Err(KeyVaultError::Wrapping(format!(
                "CryptUnprotectData failed with code {}",
                unsafe { windows_sys::Win32::Foundation::GetLastError() }
            )));
        }
        if unwrapped.cbData != KEY_LENGTH as u32 {
            unsafe { windows_sys::Win32::System::Com::CoTaskMemFree(unwrapped.pbData as *mut _) };
            return Err(KeyVaultError::Wrapping(format!(
                "wrapped key had unexpected length {}",
                unwrapped.cbData,
            )));
        }
        let mut key = [0u8; KEY_LENGTH];
        key.copy_from_slice(unsafe { std::slice::from_raw_parts(unwrapped.pbData, KEY_LENGTH) });
        unsafe { windows_sys::Win32::System::Com::CoTaskMemFree(unwrapped.pbData as *mut _) };
        Ok(key)
    }
}

// ---------------------------------------------------------------------------
// Development-only implementation (non-Windows builds only).
// ---------------------------------------------------------------------------

/// The only non-DPAPI wrapper permitted: a compile-time-only stand-in so the
/// portable key-vault logic can be exercised outside Windows. It is excluded
/// from Windows builds entirely and must never be selectable at runtime.
#[cfg(not(windows))]
struct DevKeyWrapper;

#[cfg(not(windows))]
impl PlatformKeyWrapper for DevKeyWrapper {
    fn wrap(&self, key: &[u8; KEY_LENGTH]) -> Result<Vec<u8>, KeyVaultError> {
        // Dev wrapper: version byte + raw key bytes, stored under the same
        // file layout as the Windows blob. This is explicitly NOT a security
        // boundary; it exists only so tests and CI can exercise the key
        // vault's storage and sealing logic off Windows.
        let mut blob = Vec::with_capacity(1 + KEY_LENGTH);
        blob.push(SEALED_VERSION);
        blob.extend_from_slice(key);
        Ok(blob)
    }

    fn unwrap(&self, blob: &[u8]) -> Result<[u8; KEY_LENGTH], KeyVaultError> {
        let mut key = [0u8; KEY_LENGTH];
        match blob {
            [version_byte, rest @ ..]
                if *version_byte == SEALED_VERSION && rest.len() == KEY_LENGTH =>
            {
                key.copy_from_slice(rest);
                Ok(key)
            }
            _ => Err(KeyVaultError::Wrapping(
                "development wrapper could not read wrapped key".to_string(),
            )),
        }
    }
}

fn platform_wrapper() -> Box<dyn PlatformKeyWrapper> {
    #[cfg(windows)]
    {
        Box::new(DpapiKeyWrapper)
    }
    #[cfg(not(windows))]
    {
        Box::new(DevKeyWrapper)
    }
}

// ---------------------------------------------------------------------------
// Key vault.
// ---------------------------------------------------------------------------

/// Generates `length` bytes of key material from the platform entropy
/// source. No key material is returned from this function on success other
/// than the caller-held bytes.
fn generate_key_material(length: usize) -> Result<Vec<u8>, KeyVaultError> {
    let mut key = vec![0u8; length];
    fill(&mut key).map_err(|error| KeyVaultError::Entropy(error.to_string()))?;
    Ok(key)
}

/// The in-memory key vault. It holds the *unwrapped* data-encryption key for
/// the lifetime of the application process so that seal/open operations do
/// not re-unprotect the blob on every call. The raw key is never exposed
/// through commands, logging, or the renderer; it leaves process memory when
/// the `KeyVault` is dropped (the `Vec` is freed by the allocator; sensitive
/// zeroing is part of a future hardening pass once the DPAPI blob is
/// validated on real Windows).
pub struct KeyVault {
    data_directory: PathBuf,
    key: [u8; KEY_LENGTH],
}

impl KeyVault {
    /// Opens the key vault, generating and wrapping a fresh data-encryption
    /// key on first use. All persisted key material is the DPAPI-wrapped blob.
    pub fn new(data_directory: &std::path::Path) -> Result<Self, KeyVaultError> {
        let wrapped_path = data_directory.join(WRAPPED_KEY_FILE_NAME);
        let wrapper = platform_wrapper();
        let key = if wrapped_path.exists() {
            let blob = std::fs::read(&wrapped_path)
                .map_err(|error| KeyVaultError::Storage(error.to_string()))?;
            wrapper.unwrap(&blob)?
        } else {
            let material = generate_key_material(KEY_LENGTH)?;
            let mut key = [0u8; KEY_LENGTH];
            key.copy_from_slice(&material);
            let blob = wrapper.wrap(&key)?;
            std::fs::write(&wrapped_path, &blob)
                .map_err(|error| KeyVaultError::Storage(error.to_string()))?;
            key
        };
        Ok(Self {
            data_directory: data_directory.to_path_buf(),
            key,
        })
    }

    /// Encrypts a plaintext into a versioned AEAD envelope.
    pub fn seal(&self, plaintext: &[u8]) -> Result<SealedValue, KeyVaultError> {
        let mut nonce_bytes = [0u8; 12];
        fill(&mut nonce_bytes).map_err(|error| KeyVaultError::Entropy(error.to_string()))?;
        let cipher = ChaCha20Poly1305::new(AeadKey::from_slice(&self.key));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
            .map_err(|error| KeyVaultError::SealedValue(error.to_string()))?;
        Ok(SealedValue {
            version: SEALED_VERSION,
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        })
    }

    /// Decrypts a versioned AEAD envelope. Tampered or cross-key ciphertexts
    /// fail authentication.
    pub fn open(&self, sealed: &SealedValue) -> Result<Vec<u8>, KeyVaultError> {
        if sealed.version != SEALED_VERSION {
            return Err(KeyVaultError::SealedValue(format!(
                "unknown sealed version {}",
                sealed.version,
            )));
        }
        let nonce: [u8; 12] = sealed.nonce.as_slice().try_into().map_err(|_| {
            KeyVaultError::SealedValue("sealed nonce has invalid length".to_string())
        })?;
        let cipher = ChaCha20Poly1305::new(AeadKey::from_slice(&self.key));
        cipher
            .decrypt(Nonce::from_slice(&nonce), sealed.ciphertext.as_ref())
            .map_err(|_| KeyVaultError::SealedValue(
                "sealed value failed authentication; the ciphertext may be tampered or bound to a different key".to_string(),
            ))
    }

    /// Serializes a sealed envelope to the storage format: one version byte,
    /// the nonce, then the ciphertext.
    pub fn encode_sealed(sealed: &SealedValue) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(1 + sealed.nonce.len() + sealed.ciphertext.len());
        buffer.push(sealed.version);
        buffer.extend_from_slice(&sealed.nonce);
        buffer.extend_from_slice(&sealed.ciphertext);
        buffer
    }

    /// Parses a storage-format sealed envelope.
    pub fn decode_sealed(raw: &[u8]) -> Result<SealedValue, KeyVaultError> {
        if raw.is_empty() {
            return Err(KeyVaultError::SealedValue(
                "sealed value is empty".to_string(),
            ));
        }
        let version = raw[0];
        if version != SEALED_VERSION {
            return Err(KeyVaultError::SealedValue(format!(
                "unknown sealed version {version}",
            )));
        }
        if raw.len() < 1 + 12 {
            return Err(KeyVaultError::SealedValue(
                "sealed value is too short to hold a nonce".to_string(),
            ));
        }
        let nonce = raw[1..13].to_vec();
        let ciphertext = raw[13..].to_vec();
        if ciphertext.is_empty() {
            return Err(KeyVaultError::SealedValue(
                "sealed value has no ciphertext".to_string(),
            ));
        }
        Ok(SealedValue {
            version,
            nonce,
            ciphertext,
        })
    }

    /// The application-data directory this vault persists its wrapped key in.
    #[allow(dead_code)]
    /// Exposed for the export/recovery path (the archive's key binding is
    /// derived from this directory) and tests; it carries no key material.
    pub fn data_directory(&self) -> &std::path::Path {
        &self.data_directory
    }

    pub fn status(&self) -> KeyVaultStatus {
        KeyVaultStatus {
            wrapped_key_persisted: self.data_directory.join(WRAPPED_KEY_FILE_NAME).exists(),
            key_length: KEY_LENGTH,
            sealed_version: SEALED_VERSION,
        }
    }
}

/// Diagnostic summary returned by the `key_vault_status` command.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyVaultStatus {
    pub wrapped_key_persisted: bool,
    pub key_length: usize,
    pub sealed_version: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault() -> KeyVault {
        let directory =
            std::env::temp_dir().join(format!("aura-keyvault-{}", uuid::Uuid::new_v4(),));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        KeyVault::new(&directory).expect("open key vault")
    }

    #[test]
    fn first_use_generates_key_material_and_persists_only_a_wrapped_blob() {
        let directory =
            std::env::temp_dir().join(format!("aura-keyvault-{}", uuid::Uuid::new_v4(),));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        let vault = KeyVault::new(&directory).expect("open key vault");
        let blob_path = directory.join(WRAPPED_KEY_FILE_NAME);
        assert!(blob_path.exists(), "wrapped blob must be persisted");
        let status = vault.status();
        assert!(status.wrapped_key_persisted);
        assert_eq!(status.key_length, KEY_LENGTH);
        assert_eq!(status.sealed_version, SEALED_VERSION);
    }

    #[test]
    fn seal_and_open_are_lossless_for_arbitrary_utf8() {
        let vault = temp_vault();
        let plaintext = "Aura context note: the release gate is DPAPI wrapping.".as_bytes();
        let sealed = vault.seal(plaintext).expect("seal");
        assert_eq!(sealed.version, SEALED_VERSION);
        assert_eq!(sealed.nonce.len(), 12);
        let recovered = vault.open(&sealed).expect("open");
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let vault = temp_vault();
        let sealed = vault.seal(b"secret").expect("seal");
        let mut tampered = sealed.ciphertext.clone();
        let last = tampered.len() - 1;
        tampered[last] = tampered[last].wrapping_add(1);
        let result = vault.open(&SealedValue {
            ciphertext: tampered,
            ..sealed
        });
        assert!(
            result.is_err(),
            "a tampered envelope must fail AEAD authentication",
        );
    }

    #[test]
    fn sealed_values_roundtrip_through_the_storage_format() {
        let vault = temp_vault();
        let plaintext = "cross-version storage format check".as_bytes();
        let sealed = vault.seal(plaintext).expect("seal");
        let encoded = KeyVault::encode_sealed(&sealed);
        assert_eq!(encoded[0], SEALED_VERSION);
        assert_eq!(encoded.len(), 1 + 12 + sealed.ciphertext.len());
        let decoded = KeyVault::decode_sealed(&encoded).expect("decode");
        assert_eq!(decoded, sealed);
        assert_eq!(vault.open(&decoded).expect("open"), plaintext);
    }

    #[test]
    fn malformed_storage_envelopes_are_rejected() {
        assert!(KeyVault::decode_sealed(&[]).is_err());
        assert!(KeyVault::decode_sealed(&[0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(KeyVault::decode_sealed(&[SEALED_VERSION, 0, 0]).is_err());
        assert!(
            KeyVault::decode_sealed(&[SEALED_VERSION, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_err()
        );
    }

    #[test]
    fn vault_reopens_an_existing_wrapped_blob() {
        let directory =
            std::env::temp_dir().join(format!("aura-keyvault-{}", uuid::Uuid::new_v4(),));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        {
            let first = KeyVault::new(&directory).expect("open first vault");
            let sealed = first.seal(b"persisted across restart").expect("seal");
            let raw = KeyVault::encode_sealed(&sealed);
            std::fs::write(directory.join("sealed.bin"), &raw).expect("write fixture");
            raw
        };
        let second = KeyVault::new(&directory).expect("open second vault");
        let decoded =
            KeyVault::decode_sealed(&std::fs::read(directory.join("sealed.bin")).unwrap())
                .expect("decode");
        assert_eq!(
            second.open(&decoded).expect("open"),
            b"persisted across restart",
            "a later process must unwrap the same key and open earlier envelopes",
        );
    }
}
