
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

        dbg!(&key);
        dbg!(&encrypted);

        let new_clear = DkEncrypt::decrypt_str(&encrypted, &key).unwrap();
        // dbg!(&new_clear);

        assert_eq!(&clear, &new_clear);
    }

    // Decrypt some text with a specific key
    // Encrypt the result back
    // Compare with the original
    // Disabled : the chacha20 does not produce the same result after an cypher
    // #[test]
    // pub fn a30_decrypt_encrypt() {
    //     if LOG_ENABLE {init_log()};
    //     // let encrypted = "5eftIdP8d4MFUU4KVUn-VQ3Tu_SACE47R01xt9KOhVCxGyVVRSn19yWnbXjOmg-cao6SW4itOM4cRUz33ZgQP_Ae5VtTmk-NsXtg5StaYlGX4QCljpO914xJkocNW_0TZCLvqzaNsTZKGzbPGXJlFMWy8JunbKMR1omkze5-w17Yxr2Gg1SpHU57SeqBCpvbkj5rMyF6skxp4LWMQzEBSj121n7VpXkmndtP-y4n7QOeQjTpW2tmXMhqpTyr-B5mhO7PXsMcNoIcWr7FCpGws14m_I8PNRaCN3nfpviXV5l1TbBa1noeE5HH0AFOs8IxqMLRmikA6bY8Av6IipDYnbZ7d2TO6SjGcE40Yvl3Z_e963Y4GLrbpnwj_9_V4_wNmUFROtj9AO5uRPzwEQdlKcGmiqfluTow-jG4ROJTnaggiCkaTEyFpcjhAye8VNahjo1rKBxecWzC1bp6SrH1-g-jFnMT5yrC7rko3fYvuN2LBpIldDziaJ3ahy3rRWIkelYIHigx6Zu__BZXSAkoKioQ6kvldsVDvFi1_NUISk3b9TOs5pNcopVJKhBEiJHoSUonICPj7UzxauyArh-RzNQQoZV19D03hXFNgXYJvPuXJ3upIpgFMaLC59NcAGZj0Q3H3uztAmkvpICr5Uv05FrmdiLKpN0lhKS0ETr2gVwuY_MRNTmI_V5Ud7SY6tutnLQtjrOFPNckPMQ1Yjyq_2b3FrClJ5fvunvfAEDh0RSKOx62GatWWtiuH7HDhkU_0pRC6QfnIL9W0W6YLnvlTKq_HaaVECuhp-PMRN6PQxkg5TOWOtjQ1IyvIosKfgBXhjyp5AhKlYevoOZqRyo0YxycviyCZUAq4-k5KzTaacDPMx_HYcpg0waPVIsE4DPtgLNQjDl2RaEGUKYntu89bYn47lFj3CP1j0umrWwJuJhznr5NtU7oxZ4Rlznq3lEjqNKkHnvUWD3Z8l68XWicvHWaZ9itH6IznD9GMksQYA-YbumI9wh4BIP1u1T-A9pHWRbWjpJP2sNVKMgLeIZhCy5go8uHDPIwNqTZFQLM59DtTrWCEJHQIP4KMabwHNDTBHvVQtn-EOQZP9kF7kMtYKsnmMlx12mS-fdG4qT_ko5zceYctXwiICT-DpWiRhfI2C29zRZqPLj0s3iuMo1xopL1fDX9b6gG2RywFZwZRtjEhiFi-lfpR-P7Jck61qu2V4sBx_OYNa78epKwelp6gwtSgmzOJjnPULmif9AL9HE";
    //     let encrypted = "2LvOn_uS8NEQ3Ry0R2crclmhIdUQ1cu4WeZhG5O89ZS3rniyAyOPHCwRafBm_CNayHNFfHh71EMwR7f_9ymKxzSXHHQlIQ61EMfHgsrt0dXtwPehsmn03O0Z-HtzxZjEZ5te6Byt6JBcQ9t7iuuKhZlHBRR5sNGCkntqz15UIfm6DFvyyV8LwQJxFKZjzPsI8uouVq4WPGl81zPy9kh-9PkT50fZJnWhc_JjlVwXg2tiWCFIoAmitFPHyqsBNVKGO8iyHER4BgFreRQVKakVBRsspfaAps1p5ijxXBhJCCI_5Z3yFDrSvplTU5edzgV02tHSYG_YxLSXxOCVmNJAyl6-VDM5vv35LRI25ET2mkcaF3TqB1cNO1WoHRjMZ0KoQcw3wU8oHiWaZYKC5kQfHcrHoilGve6hgjdTU8sRwWZCJXpvVInL5ifzmdvg2i10N7JiA-CsZTF93pbEWVnYt4DacHSOfzySh6guY_UluoOOvIJr9370HJWh0p9PoiaRMdCToN_ulTeamnvjhmPBO2CW7bxg2g52H-oCuiAAT9FRW1_aBUMeRBMLCJ5XwCUDOiczHkit3wTSnRGybpuC30Yk8nOdiKnlPqaNJX-TwF8oGMJHxnisTffLRF0bBNJ-VMi_F6Vc4HJ19H5NVi2Ydi4DEDGfWf3A2l-rpkD0x2gJ7Nq2_n3_WD8UIME_fdm8CYqqptuVKtYkBo9JYSxeF8RoAQ6MSkHtJJubr3XHv0GaIF9Qf1jsM-8TAuv-8agJ7AnpaCprWlt5l7kfRryWL1t7o31jRn06w_LfGEghWpkb3Pq57E1F08nXgve70E8O86BARuQTLv6I4_X7oGwJgilMF9TOzug22zdptyzBq8wmW7tTsUDiz1lK_qraGUIdCcCAuVmX8UD2LWAWVSPeFjtqg7jIbxuiHTlp69RWAy2an6uaL_1L4KpB02UPaCnI83pX6vwVRv99nxqnGSu-1iGFqeyLesgM03hPn4n9sFdjJbLCcpSLVbiX2R_oX4V2wj29WHZsdBlYVjSlIMj1j1JfaSCCdQGaQrseKljrKgF1KukDEvCyO4JJhdJE4JeYJNBVgF2tVcGCLV2EuFUzOebOnmTnHzA-931pvt8TPsqOf-IWUZOqx18HizfC10aIe6HVSZDiGiooEYNMmVXzW7twnCdg8BDeAulRob_-98rRdFVq8d_gN5xv6-xBJToP1R7PDwMpVa7kpRG_oA6DOQhdi-rmvTdM7AaZCkMokK4NcjMkJv3NPSZUlveZq7ydlTjsDAOxm69l9Rs";
    //
    //     let clear = DkEncrypt::decrypt_str(encrypted, "363QFiE6wXcDk7izhhQXvwlPJeCYNJG0EKtdHqxMgDg").unwrap();
    //     dbg!(&clear);
    //     let new_encrypted = DkEncrypt::encrypt_str(&clear, "363QFiE6wXcDk7izhhQXvwlPJeCYNJG0EKtdHqxMgDg").unwrap();
    //
    //     // dbg!(&new_encrypted);
    //     assert_eq!(&encrypted, &new_encrypted);
    // }

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
        let encrypted = DkEncrypt::encrypt_str(clear, KEY).unwrap();

        dbg!(&encrypted);
    }


    const KEY: &str = "fqYVyce-Nh0HwpPQ7ZGZLog5s7PBLnwFMAW2OMnNPUs";

    // cargo test --release  --package dkcrypto --test all_tests tests::d10_performance -- --nocapture
    #[test]
    pub fn d10_performance() {
        if LOG_ENABLE {init_log()};

        let phrases = [
            "1. Hello, how are you?",
            "2. I love coding in Rust.",
            "3. The quick brown fox jumps over the lazy dog.",
            "4. Rust is a systems programming language.",
            "5. OpenAI's GPT-3.5 is an amazing language model.",
            "6. I enjoy helping people with their questions.",
            "7. Rustaceans are a friendly community.",
            "8. Programming is fun and challenging.",
            "9. The beach is a great place to relax.",
            "10. I like to read books in my free time.",
            "11. Learning new things is always exciting.",
            "12. Rust is known for its memory safety features.",
            "13. I'm excited to see what the future holds.",
            "14. The mountains are majestic and beautiful.",
            "15. I enjoy playing musical instruments.",
            "16. Coding allows us to create amazing things.",
            "17. Rust's syntax is elegant and expressive.",
            "18. I believe in lifelong learning.",
            "19. The stars are mesmerizing at night.",
            "20. Rust enables high-performance software development.",
        ];

        use chrono::Utc;
        let timestamp_start_0 = Utc::now().timestamp_millis();
        let mut count_0 = 0;
        for _ in 1..=5 {
            let nb = encrypt_test(&phrases);
            count_0 += nb;
        }
        let timestamp_end_0 = Utc::now().timestamp_millis();
        println!("diff [{}] ms", timestamp_end_0 - timestamp_start_0);
        println!("avg [{}] ms", (timestamp_end_0 - timestamp_start_0) / count_0 as i64);

        // Avec Rayon
        let timestamp_start = Utc::now().timestamp_millis();
        let mut count = 0;
        let mut encrypted_words = vec![];
        for _ in 1..=5 {
            encrypted_words = encrypt_rayon_test(&phrases);
            count += encrypted_words.len();
        }
        let timestamp_end = Utc::now().timestamp_millis();
        println!("rayon diff [{}] ms", timestamp_end - timestamp_start);
        println!("rayon avg [{}] ms", (timestamp_end - timestamp_start) / count as i64);

        for w in encrypted_words {
            let clear = DkEncrypt::decrypt_str(&w, KEY).unwrap();
            println!("Clear: {}", &clear);
        }

    }

    fn encrypt_test(phrases: &[&str]) -> i32 {
        let mut count = 0;
        for phrase in phrases.iter() {
            let encrypted = DkEncrypt::encrypt_str(phrase, KEY).unwrap();
            count += 1;
        }
        count
    }


    fn encrypt_rayon_test(phrases: &[&str]) -> Vec<String> {
        use rayon::prelude::*;
        let encrypted_phrases: Vec<String> = phrases.par_iter()
            .map(|phrase| {
                DkEncrypt::encrypt_str(phrase, KEY).unwrap()
            }).collect();
        encrypted_phrases
    }

}