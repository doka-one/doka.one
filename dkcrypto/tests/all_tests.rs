#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::exit;
    use std::sync::Once;

    use log::*;

    use commons_error::*;
    use dkcrypto::dk_crypto::CypherMode::CC20;
    use dkcrypto::dk_crypto::*;

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

    const LOG_ENABLE: bool = false;

    #[test]
    pub fn a10_hash_my_password() {
        if LOG_ENABLE {
            init_log()
        };
        let s = DkEncrypt::hash_password("my super password");
        log_info!("s = {:?}", &s);
        assert_eq!(true, s.len() > 2, "Wrong hash");
    }

    //
    //
    //
    #[test]
    pub fn a20_encrypt_decrypt() {
        if LOG_ENABLE {
            init_log()
        };

        let clear = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n   <customers>\n      <customer name=\"doka.live\">\n         <cipheredPassword>KgnCdO4pwroiQQfkve7lwtAOClu4N4MgKmpYvsOF5Xodq1EZT_vdeQ_Y_XSj53w8</cipheredPassword>\n         <enabled>true</enabled>\n         <masterKeyHash>gjzLNyQzpZtYOv_Z5XXAJhVdlrJ1X0nZQsHOGCiYmWU</masterKeyHash>\n      </customer>\n      <customer name=\"[[P2_MESSAGE]]\">\n         <cipheredPassword>NCEOHZq19Ta5VK7XkGZPTSP-lOBwkKzCn0DRPl0SKDJ3lsIRxsPUFBq6wNWW-Uiw</cipheredPassword>\n         <enabled>true</enabled>\n         <masterKeyHash>gjzLNyQzpZtYOv_Z5XXAJhVdlrJ1X0nZQsHOGCiYmWU</masterKeyHash>\n      </customer>\n      <customer name=\"SYSTEM\">\n         <cipheredPassword>DSnE3j0m9IqHzdNkAbFNw1So_CawWiUHxfHfJmdDIzjsBoRAXWwDWWITZH1pYXdQ</cipheredPassword>\n         <enabled>true</enabled>\n         <masterKeyHash>gjzLNyQzpZtYOv_Z5XXAJhVdlrJ1X0nZQsHOGCiYmWU</masterKeyHash>\n      </customer>\n   </customers>\n</config>\n";

        let key = DkEncrypt::generate_random_key();
        let encrypted = DkEncrypt::new(CC20).encrypt_str(clear, &key).unwrap();

        dbg!(&key);
        dbg!(&encrypted);

        let new_clear = DkEncrypt::new(CC20).decrypt_str(&encrypted, &key).unwrap();
        // dbg!(&new_clear);

        assert_eq!(&clear, &new_clear);
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
        if LOG_ENABLE {
            init_log()
        };
        let clear = r#"{"expiration_date":"2022-11-01T12:00Z"}"#;
        let encrypted = DkEncrypt::new(CC20).encrypt_str(clear, KEY).unwrap();

        dbg!(&encrypted);
    }

    const KEY: &str = "fqYVyce-Nh0HwpPQ7ZGZLog5s7PBLnwFMAW2OMnNPUs";

    // cargo test --release  --package dkcrypto --test all_tests tests::d10_performance -- --nocapture
    #[test]
    pub fn d10_performance() {
        if LOG_ENABLE {
            init_log()
        };

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
        println!(
            "avg [{}] ms",
            (timestamp_end_0 - timestamp_start_0) / count_0 as i64
        );

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
        println!(
            "rayon avg [{}] ms",
            (timestamp_end - timestamp_start) / count as i64
        );

        for w in encrypted_words {
            let clear = DkEncrypt::new(CC20).decrypt_str(&w, KEY).unwrap();
            println!("Clear: {}", &clear);
        }
    }

    fn encrypt_test(phrases: &[&str]) -> i32 {
        let mut count = 0;
        for phrase in phrases.iter() {
            let encrypted = DkEncrypt::new(CC20).encrypt_str(phrase, KEY).unwrap();
            count += 1;
        }
        count
    }

    fn encrypt_rayon_test(phrases: &[&str]) -> Vec<String> {
        use rayon::prelude::*;
        let encrypted_phrases: Vec<String> = phrases
            .par_iter()
            .map(|phrase| DkEncrypt::new(CC20).encrypt_str(phrase, KEY).unwrap())
            .collect();
        encrypted_phrases
    }
}
