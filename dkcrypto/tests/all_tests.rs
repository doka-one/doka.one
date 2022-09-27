
#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::exit;
    use std::sync::Once;
    use dkcrypto::dk_crypto::*;
    use log::*;
    use commons_error::*;


    static INIT: Once = Once::new();

    fn init_log() {

        INIT.call_once(|| {

            // TODO Use the future commons-config
            let log_config: String = "E:/doka-configs/dev/ppm/config/log4rs.yaml".to_string();
            let log_config_path = Path::new(&log_config);

            match log4rs::init_file(&log_config_path, Default::default()) {
                Err(e) => {
                    eprintln!("{:?} {:?}", &log_config_path, e);
                    exit(-59);
                }
                Ok(_) => {}
            }
        });
    }

    // Get "DOKA_UT_ENV"  and find the test files in [E:/doka-configs/dev]/dkcrypto/data

    const LOG_ENABLE : bool = false;

    #[test]
    pub fn a10_hash_my_password() {
        if LOG_ENABLE {init_log()};
        let s = DkEncrypt::hash_password("my super password");
        log_info!("s = {:?}", &s);
        assert_eq!(true, s.len() > 2, "Wrong hash");
    }

    //
    //
    //
    #[test]
    pub fn a20_encrypt_decrypt() {
        if LOG_ENABLE {init_log()};

        let clear = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n   <customers>\n      <customer name=\"doka.live\">\n         <cipheredPassword>KgnCdO4pwroiQQfkve7lwtAOClu4N4MgKmpYvsOF5Xodq1EZT_vdeQ_Y_XSj53w8</cipheredPassword>\n         <enabled>true</enabled>\n         <masterKeyHash>gjzLNyQzpZtYOv_Z5XXAJhVdlrJ1X0nZQsHOGCiYmWU</masterKeyHash>\n      </customer>\n      <customer name=\"[[P2_MESSAGE]]\">\n         <cipheredPassword>NCEOHZq19Ta5VK7XkGZPTSP-lOBwkKzCn0DRPl0SKDJ3lsIRxsPUFBq6wNWW-Uiw</cipheredPassword>\n         <enabled>true</enabled>\n         <masterKeyHash>gjzLNyQzpZtYOv_Z5XXAJhVdlrJ1X0nZQsHOGCiYmWU</masterKeyHash>\n      </customer>\n      <customer name=\"SYSTEM\">\n         <cipheredPassword>DSnE3j0m9IqHzdNkAbFNw1So_CawWiUHxfHfJmdDIzjsBoRAXWwDWWITZH1pYXdQ</cipheredPassword>\n         <enabled>true</enabled>\n         <masterKeyHash>gjzLNyQzpZtYOv_Z5XXAJhVdlrJ1X0nZQsHOGCiYmWU</masterKeyHash>\n      </customer>\n   </customers>\n</config>\n";

        let key = DkEncrypt::generate_random_key();
        let encrypted = DkEncrypt::encrypt_str(clear, &key).unwrap();

        // dbg!(&encrypted);

        let new_clear = DkEncrypt::decrypt_str(&encrypted, &key).unwrap();
        // dbg!(&new_clear);

        assert_eq!(&clear, &new_clear);
    }

    // Decrypt some text with a specific key
    // Encrypt the result back
    // Compare with the original
    #[test]
    pub fn a30_decrypt_encrypt() {
        if LOG_ENABLE {init_log()};
        let encrypted = "5eftIdP8d4MFUU4KVUn-VQ3Tu_SACE47R01xt9KOhVCxGyVVRSn19yWnbXjOmg-cao6SW4itOM4cRUz33ZgQP_Ae5VtTmk-NsXtg5StaYlGX4QCljpO914xJkocNW_0TZCLvqzaNsTZKGzbPGXJlFMWy8JunbKMR1omkze5-w17Yxr2Gg1SpHU57SeqBCpvbkj5rMyF6skxp4LWMQzEBSj121n7VpXkmndtP-y4n7QOeQjTpW2tmXMhqpTyr-B5mhO7PXsMcNoIcWr7FCpGws14m_I8PNRaCN3nfpviXV5l1TbBa1noeE5HH0AFOs8IxqMLRmikA6bY8Av6IipDYnbZ7d2TO6SjGcE40Yvl3Z_e963Y4GLrbpnwj_9_V4_wNmUFROtj9AO5uRPzwEQdlKcGmiqfluTow-jG4ROJTnaggiCkaTEyFpcjhAye8VNahjo1rKBxecWzC1bp6SrH1-g-jFnMT5yrC7rko3fYvuN2LBpIldDziaJ3ahy3rRWIkelYIHigx6Zu__BZXSAkoKioQ6kvldsVDvFi1_NUISk3b9TOs5pNcopVJKhBEiJHoSUonICPj7UzxauyArh-RzNQQoZV19D03hXFNgXYJvPuXJ3upIpgFMaLC59NcAGZj0Q3H3uztAmkvpICr5Uv05FrmdiLKpN0lhKS0ETr2gVwuY_MRNTmI_V5Ud7SY6tutnLQtjrOFPNckPMQ1Yjyq_2b3FrClJ5fvunvfAEDh0RSKOx62GatWWtiuH7HDhkU_0pRC6QfnIL9W0W6YLnvlTKq_HaaVECuhp-PMRN6PQxkg5TOWOtjQ1IyvIosKfgBXhjyp5AhKlYevoOZqRyo0YxycviyCZUAq4-k5KzTaacDPMx_HYcpg0waPVIsE4DPtgLNQjDl2RaEGUKYntu89bYn47lFj3CP1j0umrWwJuJhznr5NtU7oxZ4Rlznq3lEjqNKkHnvUWD3Z8l68XWicvHWaZ9itH6IznD9GMksQYA-YbumI9wh4BIP1u1T-A9pHWRbWjpJP2sNVKMgLeIZhCy5go8uHDPIwNqTZFQLM59DtTrWCEJHQIP4KMabwHNDTBHvVQtn-EOQZP9kF7kMtYKsnmMlx12mS-fdG4qT_ko5zceYctXwiICT-DpWiRhfI2C29zRZqPLj0s3iuMo1xopL1fDX9b6gG2RywFZwZRtjEhiFi-lfpR-P7Jck61qu2V4sBx_OYNa78epKwelp6gwtSgmzOJjnPULmif9AL9HE";
        let clear = DkEncrypt::decrypt_str(encrypted, "O27AYTdNPNbG-7olPOUxDNb6GNnVzZpbGRa4qkhJ4BU").unwrap();
        // dbg!(&clear);
        let new_encrypted = DkEncrypt::encrypt_str(&clear, "O27AYTdNPNbG-7olPOUxDNb6GNnVzZpbGRa4qkhJ4BU").unwrap();

        // dbg!(&new_encrypted);
        assert_eq!(&encrypted, &new_encrypted);
    }

    #[test]
    fn b10_hash_password() {
        let hash = DkEncrypt::hash_password("my_super_password");
        dbg!(&hash);
        let check = DkEncrypt::verify_password("my_super_password", &hash);
        assert_eq!(true, check);
    }

    // fqYVyce-Nh0HwpPQ7ZGZLog5s7PBLnwFMAW2OMnNPUs
    #[test]
    pub fn c10_security_token() {
        if LOG_ENABLE {init_log()};

        let clear = r#"{"expiration_date":"2022-11-01T12:00Z"}"#;

        let key = "fqYVyce-Nh0HwpPQ7ZGZLog5s7PBLnwFMAW2OMnNPUs".to_string();
        let encrypted = DkEncrypt::encrypt_str(clear, &key).unwrap();

        dbg!(&encrypted);

        //let new_clear = DkEncrypt::decrypt_str(&encrypted, &key).unwrap();
        //dbg!(&new_clear);

        //assert_eq!(&clear, &new_clear);
    }

}