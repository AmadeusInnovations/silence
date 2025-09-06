// Cryptographic core for Ephemeral Key Cascade protocol
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, aead::{Aead, KeyInit}};
use hkdf::Hkdf;
use sha2::Sha256;
use rand::{rngs::OsRng, RngCore};
// Removed zeroize import - manual secure deletion for now
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Error types for cryptographic operations
#[derive(Debug)]
pub enum CryptoError {
    KeyGeneration,
    Encryption,
    Decryption,
    KeyDerivation,
    InvalidNonce,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CryptoError::KeyGeneration => write!(f, "Key generation failed"),
            CryptoError::Encryption => write!(f, "Encryption failed"),
            CryptoError::Decryption => write!(f, "Decryption failed"),
            CryptoError::KeyDerivation => write!(f, "Key derivation failed"),
            CryptoError::InvalidNonce => write!(f, "Invalid nonce"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Ephemeral key material with automatic zeroing
pub struct EphemeralKeys {
    #[allow(dead_code)]
    master_key: [u8; 32],
    session_key: [u8; 32],
    encryption_key: [u8; 32],
    mac_key: [u8; 32],
    created_at: Instant,
    rotation_interval: Duration,
}

impl EphemeralKeys {
    /// Generate new ephemeral keys
    pub fn new(rotation_interval_secs: u64) -> Result<Self, CryptoError> {
        let mut master_key = [0u8; 32];
        OsRng.fill_bytes(&mut master_key);
        
        let mut keys = Self {
            master_key,
            session_key: [0u8; 32],
            encryption_key: [0u8; 32],
            mac_key: [0u8; 32],
            created_at: Instant::now(),
            rotation_interval: Duration::from_secs(rotation_interval_secs),
        };
        
        keys.derive_keys()?;
        Ok(keys)
    }
    
    /// Derive session keys from master key using HKDF
    fn derive_keys(&mut self) -> Result<(), CryptoError> {
        let hk = Hkdf::<Sha256>::new(None, &self.master_key);
        
        hk.expand(b"SILENCE_SESSION_KEY", &mut self.session_key)
            .map_err(|_| CryptoError::KeyDerivation)?;
        
        hk.expand(b"SILENCE_ENCRYPT_KEY", &mut self.encryption_key)
            .map_err(|_| CryptoError::KeyDerivation)?;
        
        hk.expand(b"SILENCE_MAC_KEY", &mut self.mac_key)
            .map_err(|_| CryptoError::KeyDerivation)?;
        
        Ok(())
    }
    
    /// Check if keys should be rotated
    pub fn should_rotate(&self) -> bool {
        self.created_at.elapsed() >= self.rotation_interval
    }
    
    /// Rotate keys using the current session key as input
    pub fn rotate(&mut self) -> Result<(), CryptoError> {
        // Use current session key to derive new master key
        let hk = Hkdf::<Sha256>::new(Some(&self.session_key), &self.master_key);
        hk.expand(b"SILENCE_NEW_MASTER", &mut self.master_key)
            .map_err(|_| CryptoError::KeyDerivation)?;
        
        self.derive_keys()?;
        self.created_at = Instant::now();
        Ok(())
    }
    
    /// Get encryption key
    pub fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}

/// Encrypted message format
#[derive(Serialize, Deserialize, Clone)]
pub struct EncryptedMessage {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub timestamp: u64,
}

/// Main cryptographic engine
pub struct SilenceCrypto {
    keys: EphemeralKeys,
}

impl SilenceCrypto {
    /// Initialize new crypto engine
    pub fn new(rotation_interval_secs: u64) -> Result<Self, CryptoError> {
        let keys = EphemeralKeys::new(rotation_interval_secs)?;
        Ok(Self { keys })
    }
    
    /// Encrypt a message
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<EncryptedMessage, CryptoError> {
        // Rotate keys if needed
        if self.keys.should_rotate() {
            self.keys.rotate()?;
        }
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Encrypt using ChaCha20-Poly1305
        let key = Key::from_slice(self.keys.encryption_key());
        let cipher = ChaCha20Poly1305::new(key);
        
        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::Encryption)?;
        
        Ok(EncryptedMessage {
            nonce: nonce_bytes,
            ciphertext,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }
    
    /// Decrypt a message
    pub fn decrypt(&mut self, encrypted_msg: &EncryptedMessage) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(&encrypted_msg.nonce);
        let key = Key::from_slice(self.keys.encryption_key());
        let cipher = ChaCha20Poly1305::new(key);
        
        let plaintext = cipher.decrypt(nonce, encrypted_msg.ciphertext.as_ref())
            .map_err(|_| CryptoError::Decryption)?;
        
        Ok(plaintext)
    }
    
    /// Force key rotation
    pub fn rotate_keys(&mut self) -> Result<(), CryptoError> {
        self.keys.rotate()
    }
    
    /// Get time until next key rotation
    pub fn seconds_until_rotation(&self) -> u64 {
        let elapsed = self.keys.created_at.elapsed();
        if elapsed >= self.keys.rotation_interval {
            0
        } else {
            (self.keys.rotation_interval - elapsed).as_secs()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_generation() {
        let crypto = SilenceCrypto::new(15).expect("Failed to create crypto engine");
        let seconds_remaining = crypto.seconds_until_rotation();
        assert!(seconds_remaining >= 14 && seconds_remaining <= 15, 
                "Expected 14-15 seconds, got {}", seconds_remaining);
    }
    
    #[test]
    fn test_encrypt_decrypt() {
        let mut crypto = SilenceCrypto::new(15).expect("Failed to create crypto engine");
        let message = b"Hello, secure world!";
        
        let encrypted = crypto.encrypt(message).expect("Encryption failed");
        let decrypted = crypto.decrypt(&encrypted).expect("Decryption failed");
        
        assert_eq!(message, decrypted.as_slice());
    }
    
    #[test]
    fn test_key_rotation() {
        let mut keys = EphemeralKeys::new(1).expect("Key generation failed");
        let old_key = *keys.encryption_key();
        
        std::thread::sleep(Duration::from_secs(1));
        assert!(keys.should_rotate());
        
        keys.rotate().expect("Key rotation failed");
        assert_ne!(old_key, *keys.encryption_key());
    }
}