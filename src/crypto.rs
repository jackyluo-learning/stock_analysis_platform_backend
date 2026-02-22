use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::AeadCore;

pub fn encrypt(data: &[u8], key: &[u8; 32]) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("Invalid key: {}", e))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&nonce, data).map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
    
    // Combine nonce and ciphertext
    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt(data: &[u8], key: &[u8; 32]) -> anyhow::Result<Vec<u8>> {
    if data.len() < 12 {
        return Err(anyhow::anyhow!("Invalid data length"));
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("Invalid key: {}", e))?;
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption_roundtrip() {
        let key = [0u8; 32]; // Test key
        let data = b"Sensitive financial data";
        
        let encrypted = encrypt(data, &key).expect("Encryption failed");
        let decrypted = decrypt(&encrypted, &key).expect("Decryption failed");
        
        assert_eq!(data, decrypted.as_slice());
    }

    #[test]
    fn test_decryption_with_wrong_key_fails() {
        let key = [0u8; 32];
        let wrong_key = [1u8; 32];
        let data = b"Secret";
        
        let encrypted = encrypt(data, &key).expect("Encryption failed");
        let result = decrypt(&encrypted, &wrong_key);
        
        assert!(result.is_err());
    }
}
