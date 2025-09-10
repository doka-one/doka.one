use std::fs::File;
use std::io::Read;
use std::io::{BufReader, Write};

use base64::engine::general_purpose;
use base64::Engine;
use bcrypt::{hash, verify};
use log::*;
use rand::distributions::Alphanumeric;
use rand::Rng;
use ring::hmac::Key;
use sha2::*;

use commons_error::*;

use crate::dk_aes::{decrypt_aes128, encrypt_aes128};
use crate::dk_chacha::{decrypt_cc20, encrypt_cc20};

#[derive()]
pub enum CypherMode {
    AES,
    CC20,
}

pub struct DkEncrypt {
    pub mode: CypherMode,
}

impl DkEncrypt {
    pub fn new(mode: CypherMode) -> DkEncrypt {
        DkEncrypt { mode }
    }

    /// Encrypts a binary data using the specified key
    /// The data is encrypted using AES128 in GCM mode or XChaCha20-Poly1305
    /// The IV is randomly generated and prepended to the encrypted data (AES128 only)
    /// The encrypted data is returned as a vector of bytes.
    /// # Arguments
    /// * `clear_data` - A vector of bytes that holds the data to be encrypted.
    /// * `key` - A string slice that holds the key used for encryption.
    /// # Returns
    /// * `Ok(Vec<u8>)` - The encrypted data if encryption is successful.
    /// * `Err(anyhow::Error)` - An error if encryption fails.
    pub fn encrypt_vec(&self, clear_data: &Vec<u8>, key: &str) -> anyhow::Result<Vec<u8>> {
        match self.mode {
            CypherMode::AES => {
                // Randomly generate an IV of 12 bytes
                let iv: Vec<u8> = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(12)
                    .collect();
                let encrypted = encrypt_aes128(clear_data, key, &iv)?;
                // Append the iv in the beginning of the encrypted data
                let mut encrypted_data = Vec::with_capacity(iv.len() + encrypted.len());
                encrypted_data.extend_from_slice(&iv);
                encrypted_data.extend_from_slice(&encrypted);

                Ok(encrypted_data)
            }
            CypherMode::CC20 => encrypt_cc20(clear_data, key),
        }
    }

    /// Decrypts a binary data using the specified key
    /// The data is decrypted using AES128 in GCM mode or XChaCha20-Poly1305
    /// The IV is extracted from the beginning of the encrypted data (AES128 only)
    /// The decrypted data is returned as a vector of bytes.
    /// # Arguments
    /// * `encrypted_data` - A vector of bytes that holds the encrypted data.
    /// * `key` - A string slice that holds the key used for decryption.
    /// # Returns
    /// * `Ok(Vec<u8>)` - The decrypted data if decryption is successful.
    /// * `Err(anyhow::Error)` - An error if decryption fails.
    pub fn decrypt_vec(&self, encrypted_data: &Vec<u8>, key: &str) -> anyhow::Result<Vec<u8>> {
        match self.mode {
            CypherMode::AES => {
                // Extract the IV from the encrypted data
                let iv = &encrypted_data[0..12];
                let encrypted_data = &encrypted_data[12..];
                decrypt_aes128(encrypted_data, key, iv)
            }
            CypherMode::CC20 => decrypt_cc20(encrypted_data, key),
        }
    }

    /// Encrypts a clear text string using the specified key.
    ///
    /// # Arguments
    ///
    /// * `clear_txt` - A string slice that holds the clear text to be encrypted.
    /// * `key` - A string slice that holds the key used for encryption.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The encrypted string if encryption is successful.
    /// * `Err(anyhow::Error)` - An error if encryption fails.
    pub fn encrypt_str(&self, clear_txt: &str, key: &str) -> anyhow::Result<String> {
        let clear_data = clear_txt.to_string().into_bytes();
        let encrypted_data = self
            .encrypt_vec(&clear_data, key)
            .map_err(err_fwd!("Cannot encrypt the binary data"))?;

        let str = general_purpose::URL_SAFE_NO_PAD.encode(encrypted_data);

        Ok(str)
    }

    /// Decrypts a base64 encoded string using the specified key.
    ///
    /// # Arguments
    ///
    /// * `encrypted_text` - A base64 encoded string that needs to be decrypted.
    /// * `key` - The key used for decryption
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The decrypted string if decryption is successful.
    /// * `Err(anyhow::Error)` - An error if decryption fails.
    pub fn decrypt_str(&self, encrypted_text: &str, key: &str) -> anyhow::Result<String> {
        log_debug!("Decrypt a string");

        let encrypted_data = general_purpose::URL_SAFE_NO_PAD
            .decode(encrypted_text)
            .map_err(err_fwd!("The text is not base64 encoded"))?;

        let decrypted_data = match self.decrypt_vec(&encrypted_data, key) {
            Ok(v) => v,
            Err(e) => {
                log_error!("Error {:?}", e);
                return Err(anyhow::anyhow!("Decrypt error"));
            }
        };

        let clear_string =
            String::from_utf8(decrypted_data).map_err(err_fwd!("Data are not UTF8 compatible"))?;

        Ok(clear_string)
    }

    // /// TODO to be tested
    // pub fn decrypt_file(&self, path: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    //     let file =
    //         File::open(path).map_err(err_fwd!("Cannot read the customer file, [{}]", path))?;
    //     let mut buf_reader = BufReader::new(file);
    //     let mut buf: Vec<u8> = vec![];
    //
    //     let _n = buf_reader
    //         .read_to_end(&mut buf)
    //         .map_err(err_fwd!("Didn't read enough"))?;
    //
    //     // TODO check ??? let _s = buf.to_base64(URL_SAFE);
    //     // SymmetricCipherError is no std error so we cannot use the err_fwd macro
    //     let bin_content = match self.decrypt_vec(&buf, &key) {
    //         Ok(bc) => bc,
    //         Err(e) => {
    //             log_error!("Error {:?}", e);
    //             return Err(anyhow::anyhow!("Decrypt vec error"));
    //         }
    //     };
    //
    //     Ok(bin_content)
    // }

    /// Generate a random password of 1024 bytes
    /// Then compute the SHA256 on it
    /// Returned as base64url encoded string
    pub fn generate_random_key() -> String {
        let pass_phrase: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(1024)
            .map(char::from)
            .collect();

        let bytes = compute_sha(&pass_phrase);
        let key = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        key
    }

    /// Hash a password with bcrypt
    pub fn hash_password(password: &str) -> String {
        match hash(password, 4) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Impossible to hash the password, [{}]", e);
                "".to_string()
            }
        }
    }

    ///
    ///
    ///
    pub fn verify_password(candidate: &str, hash_password: &str) -> bool {
        match verify(candidate, hash_password) {
            Ok(x) => x,
            Err(e) => {
                log_warn!("Impossible to verify the password, [{}]", e);
                false
            }
        }
    }

    ///
    /// Hash a word with SHA256
    ///
    pub fn hash_word(word: &str) -> String {
        let v = compute_sha(word);
        general_purpose::URL_SAFE_NO_PAD.encode(&v)
    }

    pub fn hmac_word(word: &str, key: &str) -> String {
        let v = compute_hmac(word, key);
        general_purpose::URL_SAFE_NO_PAD.encode(&v)
    }
} // trait DkEncrypt

fn compute_sha(text: &str) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    hasher.write_all(text.as_bytes()).unwrap();
    let result = hasher.finalize();
    (&*result).to_vec()
}

fn compute_hmac(text: &str, key: &str) -> Vec<u8> {
    use ring::hmac;
    // let rng = rand::SystemRandom::new();
    // let key = hmac::Key::generate(hmac::HMAC_SHA256, &rng).unwrap();
    let key = Key::new(hmac::HMAC_SHA256, &key.as_bytes());
    let tag = hmac::sign(&key, text.as_bytes());
    tag.as_ref().to_vec()
}

/// Crypto Init Vector
/// It's initialized once and for all.
/// Can be used by calling get_iv()
fn _get_constant_iv() -> [u8; 16] {
    log_info!("Build the IV constant");
    let iv: [u8; 16] = [
        78, 241, 26, 48, 230, 214, 47, 151, 90, 115, 148, 58, 131, 162, 119, 230,
    ];
    iv
}

#[cfg(test)]
mod tests {
    use crate::dk_crypto::CypherMode::CC20;
    use crate::dk_crypto::DkEncrypt;

    #[test]
    fn test_decrypt_token() {
        let token = "p60XDuOC6PKDcADcay4U-cLuEKgvp3eTLmj_unGDquYb-LQCappgwIZ-yc8NL-c1";
        let cek = "qYEV-MKSeQb6lSuXjqeqKH8QH7khmi0kuczzLC6j8eA";

        let clear = DkEncrypt::new(CC20).decrypt_str(token, cek).unwrap();

        println!("{:#?}", clear);
    }

    #[test]
    fn test_encrypt_token() {
        let clear_token = "{\"datetime\"}";
        let cek = "qYEV-MKSeQb6lSuXjqeqKH8QH7khmi0kuczzLC6j8eA";
        let enc_token = DkEncrypt::new(CC20).encrypt_str(clear_token, cek).unwrap();
        println!("Enc Token : {}", &enc_token);
        let clear = DkEncrypt::new(CC20).decrypt_str(&enc_token, cek).unwrap();
        println!("{:#?}", clear);
    }

    #[test]
    fn test_compute_hmac() {
        let clear_lex = "suprem";
        let cek = "qYEV-MKSeQb6lSuXjqeqKH8QH7khmi0kuczzLC6j8eA";
        let hmac_lex = DkEncrypt::hmac_word(clear_lex, cek);
        println!("Hmac lex : {}", &hmac_lex);
        assert_eq!("PkBE7p8xYsvmepI_wcEGtO672kG1p8jq9rT_hrL1mBI", &hmac_lex);
    }
}
