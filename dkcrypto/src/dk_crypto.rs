
use std::iter::repeat;
use std::fs::File;
use std::io::{BufReader};
use std::io::Read;
use std::sync::Once;

use base64::Engine;
use base64::engine::general_purpose;
use bcrypt::{hash, verify};

use crypto;
use crypto::aes::{self};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use crypto::md5::Md5;
use crypto::{ buffer, blockmodes };
use crypto::buffer::{ ReadBuffer, WriteBuffer, BufferResult };
use commons_error::*;

use log::*;
use rand::distributions::Alphanumeric;
use rand::Rng;
use obfustring::obfustring;

pub struct DkEncrypt {

}

/* Public routines */
impl DkEncrypt {

    pub fn encrypt_vec(clear_data: &Vec<u8>, key : &str  ) -> anyhow::Result<Vec<u8>>  {
        let iv = get_iv();
        let vec_key = general_purpose::URL_SAFE_NO_PAD.decode(key)?;
        // let vec_key =  key.from_base64()?;
        let slice_key = &vec_key[..];
        let slice_clear : &[u8] = &clear_data[..];
        let r_encrypted = encrypt(slice_clear, slice_key, &iv)
            .map_err(err_fwd!("Cannot encrypt the data"))?;

        Ok(r_encrypted)
    }

    //
    //
    pub fn decrypt_vec(encrypted_data : &Vec<u8>, key : &str  ) -> anyhow::Result<Vec<u8>> {
        let iv = get_iv();
        let vec_key = general_purpose::URL_SAFE_NO_PAD.decode(key)?;
        // let vec_key =  key.from_base64()?;
        let slice_key = &vec_key[..];
        let slice_encrypted : &[u8] = &encrypted_data[..];
        let r_decrypted = decrypt(slice_encrypted, slice_key, &iv)
            .map_err(err_fwd!("Cannot decrypt the data"))?;

        Ok(r_decrypted)
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


    #[deprecated(note="use decrypt_file instead")]
    pub fn decrypt_customer_file(path : &str, key : &str ) -> anyhow::Result<String> {
        let bytes = Self::decrypt_file(path,key).map_err(err_fwd!("Decrypt file error"))?;
        let ret = String::from_utf8(bytes).map_err( err_fwd!("Data are not utf8 compatible"))?;
        Ok(ret)
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

        let mut sha = Sha256::new();
        sha.input_str(&pass_phrase);
        let mut bytes: Vec<u8> = repeat(0u8).take(sha.output_bytes()).collect();
        sha.result(&mut bytes[..]);

        let key = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        // let key = bytes.to_base64(URL_SAFE);
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


/* Private routines */


// Encrypt a buffer with the given key and iv using
// AES-256/CBC/Pkcs encryption.
fn encrypt(data: &[u8], key: &[u8], iv: &[u8]) -> anyhow::Result<Vec<u8>> {

    // Create an encryptor instance of the best performing
    let mut encryptor = aes::cbc_encryptor(
        aes::KeySize::KeySize256,
        key,
        iv,
        blockmodes::PkcsPadding);

    let mut final_result = Vec::<u8>::new();
    let mut read_buffer = buffer::RefReadBuffer::new(data);
    let mut buffer = [0; 4096];
    let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);

    loop {
        let result = match encryptor.encrypt(&mut read_buffer, &mut write_buffer, true) {
            Ok(r) => {r}
            Err(e) => {
                log_error!("Error {:?}", e);
                return Err(anyhow::anyhow!("Decrypt vec error"));
            }
        };

        final_result.extend(write_buffer.take_read_buffer().take_remaining().iter().map(|&i| i));
        match result {
            BufferResult::BufferUnderflow => break,
            BufferResult::BufferOverflow => { }
        }
    }

    Ok(final_result)
}

//
// Decrypt a byte array of data with the provided key and iv
//
fn decrypt(encrypted_data: &[u8], key: &[u8], iv: &[u8]) -> anyhow::Result<Vec<u8>> {

    let mut decryptor = aes::cbc_decryptor(
        aes::KeySize::KeySize256,
        key,
        iv,
        blockmodes::PkcsPadding
        /*blockmodes::NoPadding */);

    let mut final_result = Vec::<u8>::new();
    let mut read_buffer = buffer::RefReadBuffer::new(encrypted_data);
    let mut buffer = [0; 4096];
    let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);

    loop {

        let result = match decryptor.decrypt(&mut read_buffer, &mut write_buffer, true) {
            Ok(r) => { r},
            Err(e) => {
                log_error!("Error {:?}", e);
                return Err(anyhow::anyhow!("Decrypt error"));
            }
        };


        final_result.extend(write_buffer.take_read_buffer().take_remaining().iter().map(|&i| i));

        match result {
            BufferResult::BufferUnderflow => break,
            BufferResult::BufferOverflow => { }
        }
    }

    Ok(final_result)
}


fn get_salt() -> String {
    // Ensure the constant is not readable in the binary
    obfustring!("vg6E748cXiifSsnErGlXr5KHXN35ANmUoa2VRiebAmllCKCxItIvYZXlqCYGl0BfAzJQ4hIzbrcbISZ07yxA8G9W9x7hbZKVekpX")
}

///
/// The IV is a simple MD5 on the constant SALT
/// It's initialzed once and for all.
/// Can used by calling get_iv()
///
fn get_constant_iv() -> [u8;16] {
    log_info!("Build the IV constant");
    let mut md5 = Md5::new();
    md5.input_str(get_salt().as_str());
    let mut iv :[u8;16] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
    md5.result(&mut iv);
    iv
}


static mut IV_VALUE: [u8;16] = [0; 16];
static INIT_IV: Once = Once::new();

pub(crate) fn get_iv() -> [u8;16] {
    unsafe {
        INIT_IV.call_once(|| {
            IV_VALUE = get_constant_iv();
        });
        IV_VALUE
    }
}

#[cfg(test)]
mod tests {
    use obfustring::obfustring;

    fn test_obfstr() {
        let my_secret = obfustring!("This is my secret string");
        println!("{}", my_secret);
    }
}



