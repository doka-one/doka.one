use std::error::Error;

use aes_gcm::aead::Nonce;
use aes_gcm::{AeadCore, AeadInPlace, Aes128Gcm, Key, KeyInit};
use anyhow::{ensure, Context, Result};
use rand::RngCore;

fn get_once_from_str(none_12: &str) -> Result<Nonce<Aes128Gcm>> {
    let mut nonce_bytes = [0u8; 12];
    let bytes = none_12.as_bytes();
    ensure!(bytes.len() == 12, "Nonce must be 12 bytes long");
    nonce_bytes.copy_from_slice(bytes);
    Ok(Nonce::<Aes128Gcm>::clone_from_slice(&nonce_bytes))
}

fn get_once_from_bytes(none_12: &[u8]) -> Result<Nonce<Aes128Gcm>> {
    let mut nonce_bytes = [0u8; 12];
    ensure!(none_12.len() == 12, "Nonce must be 12 bytes long");
    nonce_bytes.copy_from_slice(none_12);
    Ok(Nonce::<Aes128Gcm>::clone_from_slice(&nonce_bytes))
}

/// Get a SecretKey that will be used to encrypt/decrypt the data
///
/// # Arguments
/// - `hash128` - The password used to encrypt/decrypt the data
/// - `salt` - The salt used to strengthen the encryption
fn get_key_from_password(hash128: &str) -> Result<Key<Aes128Gcm>> {
    // AES-128 requires a 16-byte key
    let key_bytes = hash128.as_bytes();
    // Create the AES-GCM key
    let key = Key::<Aes128Gcm>::from_slice(key_bytes);
    Ok(Key::<Aes128Gcm>::clone_from_slice(key))
}

fn encrypt_data(
    key: &Key<Aes128Gcm>,
    nonce: &Nonce<Aes128Gcm>,
    plaintext: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes128Gcm::new(key);
    let mut buffer: Vec<u8> = Vec::with_capacity(plaintext.len() + 16); // 16 bytes overhead for auth tag
    buffer.extend_from_slice(plaintext);
    cipher.encrypt_in_place(nonce, b"", &mut buffer).unwrap(); // TODO
    Ok(buffer)
}

fn decrypt_data(
    key: &Key<Aes128Gcm>,
    nonce: &Nonce<Aes128Gcm>,
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = Aes128Gcm::new(key);
    let mut buffer: Vec<u8> = Vec::from(ciphertext);
    cipher.decrypt_in_place(nonce, b"", &mut buffer).unwrap(); // TODO
    Ok(buffer)
}

/// Encrypts the plaintext with the given password and returns the ciphertext.
/// The nonce include with the password.
///
/// ## Arguments
/// - `plaintext`: The plaintext to encrypt
/// - `password`: The password to use for the encryption (16 bytes) + nonce (12 bytes)
/// - `nonce`: The salt to use for the encryption
///
/// ## Returns
/// The ciphertext + Tag (16 bytes)
pub fn encrypt_aes128(
    plaintext: impl AsRef<[u8]>,
    password: impl AsRef<str>,
    nonce: impl AsRef<[u8]>,
) -> Result<Vec<u8>> {
    let nonce = match get_once_from_bytes(nonce.as_ref()) {
        Ok(n) => n,
        Err(e) => return Err(e),
    };

    let key = get_key_from_password(&password.as_ref())?;
    encrypt_data(&key, &nonce, plaintext.as_ref())
}

/// Decrypts the ciphertext with the given password and returns the plaintext.
///
/// ## Arguments
/// - `ciphertext`: The ciphertext to decrypt + Tag (16 bytes)
/// - `password`: The password to use for the decryption (16 bytes) + nonce (12 bytes)
/// - `nonce`: The salt to use for the decryption
///
/// ## Returns
/// The plaintext as bytes
pub fn decrypt_aes128(
    ciphertext: impl AsRef<[u8]>,
    password: impl AsRef<str>,
    nonce: impl AsRef<[u8]>,
) -> Result<Vec<u8>> {
    let nonce = match get_once_from_bytes(nonce.as_ref()) {
        Ok(n) => n,
        Err(e) => return Err(e),
    };

    let key = get_key_from_password(&password.as_ref())?;
    decrypt_data(&key, &nonce, ciphertext.as_ref())
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose;
    use base64::Engine;

    use crate::dk_aes::{decrypt_aes128, encrypt_aes128};

    #[test]
    fn test_encrypt_aes128() {
        let orignal_text = "Un text utf-8 et plus ❤❤  ⡌⠁⠧⠑ ⠼⠁⠒  ⡍⠜⠇⠑⠹⠰⠎ ⡣⠕⠌";
        let bytes = orignal_text.as_bytes();
        let password_128 = "0123456789ABCDEF"; // 16 bytes for the key
        let nonce = "0123456789ABC".as_bytes(); // and 12 bytes for the nonce

        let r = encrypt_aes128(bytes, password_128, nonce).unwrap();
        let str = general_purpose::STANDARD.encode(&r);
        println!("{}", &str);
        let bb = decrypt_aes128(r, password_128, nonce).unwrap();
        let text = String::from_utf8_lossy(&bb);
        println!("{:#?}", &text);
        assert_eq!(&text, orignal_text);
    }
}
