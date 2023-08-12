use std::fs::File;
use std::io::{BufReader, Write};
use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose;
use bcrypt::{hash, verify};
use log::*;
use rand::distributions::Alphanumeric;
use rand::Rng;

use commons_error::*;

use crate::dk_chacha::{decrypt_cc20, encrypt_cc20};
use crate::dk_crypto::CypherMode::CC20;

#[derive()]
enum CypherMode {
    #[allow(dead_code)]
   AES, CC20
}

const MODE : CypherMode = CC20; // AES | CC20

pub struct DkEncrypt {
}

/* Public routines */
impl DkEncrypt {


    pub fn encrypt_vec(clear_data: &Vec<u8>, key : &str  ) -> anyhow::Result<Vec<u8>>  {

        match MODE {
            CypherMode::AES => {
                Err(anyhow::anyhow!("AES not supported"))
            }
            CC20 => {
                encrypt_cc20(clear_data, key)
            }
        }

    }

    //
    //
    pub fn decrypt_vec(encrypted_data : &Vec<u8>, key : &str  ) -> anyhow::Result<Vec<u8>> {

        match MODE {
            CypherMode::AES => {
                Err(anyhow::anyhow!("AES not supported"))
            }
            CypherMode::CC20 => {
                decrypt_cc20(encrypted_data, key)
            }
        }

    }

    //
    //
    pub fn encrypt_str(clear_txt : &str, key : &str  ) -> anyhow::Result<String> {
        let clear_data = clear_txt.to_string().into_bytes();
        let encrypted_data = DkEncrypt::encrypt_vec(&clear_data, key)
            .map_err( err_fwd!("Binary help in prison"))?;

        let str = general_purpose::URL_SAFE_NO_PAD.encode(encrypted_data);

        Ok(str)
    }


    //  We want "AES/CBC/PKCS5PADDING"
    //  base64 url key : O27AYTdNPNbG-7olPOUxDNb6GNnVzZpbGRa4qkhJ4BU
    //  crypted text : 5eftIdP8d4MFUU4KVUn-VQ3Tu_SACE47R01xt9KOhVCxGyVVRSn19yWnbXjOmg-cao6SW4itOM4cRUz33ZgQP_Ae5VtTmk-NsXtg5StaYlGX4QCljpO914xJkocNW_0TZCLvqzaNsTZKGzbPGXJlFMWy8JunbKMR1omkze5-w17Yxr2Gg1SpHU57SeqBCpvbkj5rMyF6skxp4LWMQzEBSj121n7VpXkmndtP-y4n7QOeQjTpW2tmXMhqpTyr-B5mhO7PXsMcNoIcWr7FCpGws14m_I8PNRaCN3nfpviXV5l1TbBa1noeE5HH0AFOs8IxqMLRmikA6bY8Av6IipDYnbZ7d2TO6SjGcE40Yvl3Z_e963Y4GLrbpnwj_9_V4_wNmUFROtj9AO5uRPzwEQdlKcGmiqfluTow-jG4ROJTnaggiCkaTEyFpcjhAye8VNahjo1rKBxecWzC1bp6SrH1-g-jFnMT5yrC7rko3fYvuN2LBpIldDziaJ3ahy3rRWIkelYIHigx6Zu__BZXSAkoKioQ6kvldsVDvFi1_NUISk3b9TOs5pNcopVJKhBEiJHoSUonICPj7UzxauyArh-RzNQQoZV19D03hXFNgXYJvPuXJ3upIpgFMaLC59NcAGZj0Q3H3uztAmkvpICr5Uv05FrmdiLKpN0lhKS0ETr2gVwuY_MRNTmI_V5Ud7SY6tutnLQtjrOFPNckPMQ1Yjyq_2b3FrClJ5fvunvfAEDh0RSKOx62GatWWtiuH7HDhkU_0pRC6QfnIL9W0W6YLnvlTKq_HaaVECuhp-PMRN6PQxkg5TOWOtjQ1IyvIosKfgBXhjyp5AhKlYevoOZqRyo0YxycviyCZUAq4-k5KzTaacDPMx_HYcpg0waPVIsE4DPtgLNQjDl2RaEGUKYntu89bYn47lFj3CP1j0umrWwJuJhznr5NtU7oxZ4Rlznq3lEjqNKkHnvUWD3Z8l68XWicvHWaZ9itH6IznD9GMksQYA-YbumI9wh4BIP1u1T-A9pHWRbWjpJP2sNVKMgLeIZhCy5go8uHDPIwNqTZFQLM59DtTrWCEJHQIP4KMabwHNDTBHvVQtn-EOQZP9kF7kMtYKsnmMlx12mS-fdG4qT_ko5zceYctXwiICT-DpWiRhfI2C29zRZqPLj0s3iuMo1xopL1fDX9b6gG2RywFZwZRtjEhiFi-lfpR-P7Jck61qu2V4sBx_OYNa78epKwelp6gwtSgmzOJjnPULmif9AL9HE
    pub fn decrypt_str(encrypted_text : &str, key : &str  ) -> anyhow::Result<String> {
        log_debug!("Decrypt a string");

        let encrypted_data = general_purpose::URL_SAFE_NO_PAD.decode(encrypted_text)
            .map_err( err_fwd!("The text is not base64 encoded") )?;

        // SymmetricCipherError is no std error so we cannot use the err_fwd macro
        let decrypted_data = match DkEncrypt::decrypt_vec(&encrypted_data, key) {
            Ok(v) => { v },
            Err(e) => {
                log_error!("Error {:?}", e);
                return Err(anyhow::anyhow!("Decrypt error"));
            }
        };

        let clear_string = String::from_utf8(decrypted_data)
            .map_err(err_fwd!("Data are not UTF8 compatible"))?;

        Ok(clear_string)
    }


    // TODO to be tested
    pub fn decrypt_file(path : &str,  key : &str ) -> anyhow::Result<Vec<u8>> {
        let file = File::open(path).map_err(err_fwd!("Cannot read the customer file, [{}]", path))?;
        let mut buf_reader = BufReader::new(file);
        let mut buf : Vec<u8> = vec![];

        let _n = buf_reader.read_to_end(&mut buf).map_err(err_fwd!("Didn't read enough"))?;

        // TODO check ??? let _s = buf.to_base64(URL_SAFE);
        // SymmetricCipherError is no std error so we cannot use the err_fwd macro
        let bin_content = match DkEncrypt::decrypt_vec(&buf, &key) {
            Ok(bc) => {bc}
            Err(e) => {
                log_error!("Error {:?}", e);
                return Err(anyhow::anyhow!("Decrypt vec error"));
            }
        };

        Ok(bin_content)
    }

    // Generate a random password of 1024 bytes
    // Then compute the SHA256 on it
    // Returned as base64url encoded string
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

    ///
    /// Hash a password with bcrypt
    ///
    pub fn hash_password(password: &str) -> String {
        match hash(password, 4) {
            Ok(x) => {x}
            Err(e) => {
                log_error!("Impossible to hash the password, [{}]", e);
                "".to_string()
            }
        }
    }

    ///
    ///
    ///
    pub fn verify_password(candidate : &str, hash_password : &str) -> bool {
        match verify(candidate, hash_password) {
            Ok(x) => {x}
            Err(e) => {
                log_warn!("Impossible to verify the password, [{}]", e);
                false
            }
        }
    }

}  // trait DkEncrypt


fn compute_sha(text: &str) -> Vec<u8> {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.write_all(text.as_bytes()).unwrap();
    let result = hasher.finalize();
    (&*result).to_vec()
}

/// Crypto Init Vector
/// It's initialized once and for all.
/// Can be used by calling get_iv()
fn _get_constant_iv() -> [u8; 16] {
    log_info!("Build the IV constant");
    let iv: [u8; 16] = [78, 241, 26, 48, 230, 214, 47, 151, 90, 115, 148, 58, 131, 162, 119, 230, ];
    iv
}

#[cfg(test)]
mod tests {
    use crate::dk_crypto::DkEncrypt;

    #[test]
    fn test_decrypt_token() {
        let token = "p60XDuOC6PKDcADcay4U-cLuEKgvp3eTLmj_unGDquYb-LQCappgwIZ-yc8NL-c1";
        let cek = "qYEV-MKSeQb6lSuXjqeqKH8QH7khmi0kuczzLC6j8eA";

        let clear = DkEncrypt::decrypt_str(token, cek).unwrap();

        println!("{:#?}", clear);
    }

    #[test]
    fn test_encrypt_token() {
        let clear_token = "{\"datetime\"}";
        let cek = "qYEV-MKSeQb6lSuXjqeqKH8QH7khmi0kuczzLC6j8eA";
        let enc_token = DkEncrypt::encrypt_str(clear_token, cek).unwrap();
        println!("Enc Token : {}", &enc_token);
        let clear = DkEncrypt::decrypt_str(&enc_token, cek).unwrap();
        println!("{:#?}", clear);
    }
}



