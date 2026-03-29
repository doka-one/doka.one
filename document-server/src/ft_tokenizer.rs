use std::collections::HashMap;

use anyhow::anyhow;
use chrono::Utc;
use log::debug;
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;
use unicode_segmentation::{Graphemes, UnicodeSegmentation};

use commons_error::*;
use dkcrypto::dk_crypto::CypherMode::CC20;
use dkcrypto::dk_crypto::DkEncrypt;

use crate::ft_tokenizer::WordType::WordToEncrypt;

#[derive(Debug, Clone)]
enum WordType {
    WordToEncrypt(u64),
    PureText(String),
}

fn concat(phrase: &[WordType]) -> String {
    let mut result = String::new();

    for word in phrase {
        match word {
            WordType::WordToEncrypt(_) => {}
            WordType::PureText(text) => {
                result.push_str(&text);
            }
        }
    }
    result
}

#[derive(Debug)]
enum CharType {
    SEPARATOR,
    LEXEME,
    // UNKNOWN,
}

enum PatternStatus {
    NORMAL,
    STARTED,
    DATE,
    NUMBER,
    EMAIL,
    //UNKNOWN,
}

const MIN_WORD_LEN: usize = 4;

pub(crate) struct FTTokenizer<'a> {
    graphemes: Graphemes<'a>,
    _raw_size: usize,
    pattern_status: PatternStatus,
    words: Vec<String>,
}

impl<'a> FTTokenizer<'a> {
    pub fn new(raw_text: &'a str) -> Self {
        FTTokenizer {
            graphemes: raw_text.graphemes(true),
            _raw_size: raw_text.len(),
            pattern_status: PatternStatus::NORMAL,
            words: vec![],
        }
    }

    fn char_type(g: &str) -> CharType {
        match g {
            "/" | "," | "." | "-" | "@" => CharType::LEXEME,
            _ => {
                let w = g.unicode_words().collect::<Vec<&str>>();
                if w.is_empty() {
                    CharType::SEPARATOR
                } else {
                    CharType::LEXEME
                }
            }
        }
    }

    fn clear_word(word: &str) -> String {
        let w = word.unicode_words().collect::<String>();
        w
    }

    fn terminate_word(&mut self, word: &mut Vec<&str>) {
        let s: String = word.concat();
        let w = match self.pattern_status {
            PatternStatus::NORMAL => Self::clear_word(&s),
            _ => s,
        };

        if word.len() >= MIN_WORD_LEN {
            log_debug!("Added word [{}]", &w);
            self.words.push(w);
        }

        self.pattern_status = PatternStatus::NORMAL;
        word.clear();
    }

    fn is_digit(g: &str) -> bool {
        !g.is_empty() && (g.as_bytes()[0].is_ascii_digit() || g == "-")
    }

    // pub fn next_words(&mut self) -> Vec<String> {
    //     self.next_n_words(self._raw_size)
    // }

    pub fn next_n_words(&mut self, n: usize) -> Vec<String> {
        // println!("Parsing...");
        let mut word: Vec<&str> = vec![];

        let mut counter: usize = 0;
        // let mut requested_stop = false;

        loop {
            // if counter >= n {
            //     requested_stop = true;
            // }

            let opt_g = self.graphemes.next();

            let g = match opt_g {
                None => {
                    break;
                }
                Some(g) => g,
            };

            counter += 1;

            // println!("Process grapheme [{}]", g);

            let graphem_type = Self::char_type(g);
            match graphem_type {
                CharType::SEPARATOR => {
                    // println!("Separator found (g)");
                    self.terminate_word(&mut word);
                    if counter >= n {
                        break;
                    }
                }
                CharType::LEXEME => {
                    match self.pattern_status {
                        PatternStatus::NORMAL => {
                            if word.is_empty() && Self::is_digit(g) {
                                self.pattern_status = PatternStatus::STARTED;
                                // println!("STARTED mode");
                            } else if g == "@" {
                                self.pattern_status = PatternStatus::EMAIL;
                                // println!("EMAIL mode");
                            }

                            match g {
                                // "-" is not a terminator
                                "/" | "," | "." => {
                                    // println!("Lexeme Separator found [{}]", g);
                                    self.terminate_word(&mut word);
                                    if counter >= n {
                                        break;
                                    }
                                }
                                _ => {
                                    word.push(g);
                                }
                            }
                        }
                        PatternStatus::STARTED => {
                            // The pattern has started, so we can try to guess which pattern it is.
                            match g {
                                "/" | "-" => {
                                    self.pattern_status = PatternStatus::DATE;
                                    // println!("DATE mode");
                                }
                                "," | "." => {
                                    self.pattern_status = PatternStatus::NUMBER;
                                }
                                _ => {}
                            }
                            word.push(g);
                        }
                        PatternStatus::DATE => {
                            match g {
                                "," | "." => {
                                    // println!("Lexeme Separator found [{}]", g);
                                    self.terminate_word(&mut word);
                                    if counter >= n {
                                        break;
                                    }
                                }
                                _ => {
                                    word.push(g);
                                }
                            }
                        }
                        PatternStatus::NUMBER => {
                            word.push(g);
                        }
                        PatternStatus::EMAIL => {
                            word.push(g);
                        } // PatternStatus::UNKNOWN => {
                          //     word.push(g);
                          // }
                    }
                } // CharType::UNKNOWN => {
                  //     // println!("(g)");
                  // }
            }
        }

        self.terminate_word(&mut word);
        let ret = self.words.clone();
        self.words.clear();

        ret
    }
}

fn parse_vector(tsvector: &str) -> (Vec<WordType>, HashMap<u64, String>) {
    #[derive(Debug, PartialEq)]
    enum Mode {
        Word,       // A lexeme is started, all char is part of the lexeme until  QUOTE+SEMI
        PendingEnd, // A closing QUOTE was encountered, pending for the SEMI
        Clear,      // Mormal mode where we collect the positions, not the lexemes
    }

    let graphemes = tsvector.graphemes(true);
    let mut phrase: Vec<WordType> = vec![];
    let mut lexeme: Vec<String> = vec![];
    let mut mode: Mode = Mode::Clear;

    let mut words_to_encrypt = HashMap::<u64, String>::new();
    let mut word_order = 0;

    for g in graphemes {
        match g {
            ":" => {
                //println!("Char  => SEMICOL");
                match mode {
                    Mode::PendingEnd => {
                        let w: String = lexeme.concat();
                        words_to_encrypt.insert(word_order, w.clone());
                        // The LEXEME will be replaced with the encrypted value
                        // of the word 32 in the words_to_encrypt
                        phrase.push(WordToEncrypt(word_order));
                        word_order += 1;
                        phrase.push(WordType::PureText("'".to_string()));
                        phrase.push(WordType::PureText(":".to_string()));

                        mode = Mode::Clear;
                    }
                    Mode::Word => {
                        lexeme.push(":".to_string());
                    }
                    Mode::Clear => {
                        // We ignore the : in clear mode
                        // TODO
                        // println!("PendingEnd expected, was  {:?}", mode);
                    }
                }
            }
            "'" => {
                match mode {
                    Mode::Word => {
                        // println!("Word --> Pending");
                        mode = Mode::PendingEnd;
                    }
                    Mode::PendingEnd => {
                        // Glups ...Error, we want a ":"
                        // TODO
                        //println!("Error, char : expected, was a quote" );
                    }
                    Mode::Clear => {
                        // Quote opening : Start collecting the lexeme
                        // println!("Clear --> Word");
                        mode = Mode::Word;
                        lexeme.clear();
                        phrase.push(WordType::PureText("'".to_string()));
                    }
                }
            }
            c => {
                // println!("Char  => {:?}", c);
                match mode {
                    Mode::Word => {
                        // Collect the lexeme
                        lexeme.push(c.to_string());
                    }
                    Mode::PendingEnd => {
                        //The only allowed char is " "
                        if c == " " {
                            // TODO warning
                        } else {
                            // println!("Pending --> Word");
                            mode = Mode::Word;
                            lexeme.push("'".to_string()); // Store the previous quote we ignored
                            lexeme.push(c.to_string());
                        }
                    }
                    Mode::Clear => {
                        phrase.push(WordType::PureText(c.to_string()));
                    }
                }
            }
        }
    }
    (phrase, words_to_encrypt)
}

///
/// Deprecated - Use  encrypt_words_rayon instead
/// Unused
///
fn encrypt_words(words_to_encrypt: &HashMap<u64, String>, customer_key: &str) -> anyhow::Result<HashMap<u64, String>> {
    let mut encrypted_words = HashMap::<u64, String>::new();
    for (k, w) in words_to_encrypt {
        let encrypted_word =
            DkEncrypt::new(CC20).encrypt_str(&w, customer_key).map_err(err_fwd!("Cannot encrypt the word: [{}]", w))?;
        encrypted_words.insert(*k, encrypted_word);
    }
    Ok(encrypted_words)
}

///
/// Unused
///
// fn encrypt_words_rayon(words_to_encrypt: &HashMap<u64, String>, customer_key: &str) -> anyhow::Result<HashMap<u64, String>> {
//     let encrypted_words: anyhow::Result<HashMap<u64, String>> = words_to_encrypt.par_iter()
//         .map(|(key, value)| {
//             let encrypted_value = DkEncrypt::new(CC20).encrypt_str(value, &customer_key)
//                 .map_err( err_fwd!("Cannot encrypt the word: [{}]", value))?;
//             Ok((*key, encrypted_value))
//         }).collect();
//     encrypted_words
// }

fn hash_words_rayon(
    words_to_encrypt: &HashMap<u64, String>,
    customer_key: &str,
) -> anyhow::Result<HashMap<u64, String>> {
    let encrypted_words: anyhow::Result<HashMap<u64, String>> = words_to_encrypt
        .par_iter()
        .map(|(key, value)| {
            let encrypted_value = DkEncrypt::hmac_word(value, customer_key);
            Ok((*key, encrypted_value))
        })
        .collect();
    encrypted_words
}

fn replace_words_in_phrase(
    mut phrase: Vec<WordType>,
    encrypted_words: &HashMap<u64, String>,
) -> anyhow::Result<String> {
    for w in &mut phrase {
        match w {
            WordToEncrypt(order) => {
                let r = encrypted_words.get(order).ok_or(anyhow!("Wrong index: {}", order))?;
                *w = WordType::PureText(r.clone());
            }
            WordType::PureText(_) => {}
        }
    }
    Ok(concat(&phrase))
}

///
///
///
pub fn encrypt_tsvector(tsvector: &str, customer_key: &str) -> anyhow::Result<String> {
    let timestamp_start_0 = Utc::now().timestamp_millis();
    let (phrase, words_to_encrypt) = parse_vector(&tsvector);
    let timestamp_end_0 = Utc::now().timestamp_millis();
    println!("parse_vector :: diff [{}] ms", timestamp_end_0 - timestamp_start_0);

    // dbg!(&phrase.len(), &words_to_encrypt);

    let timestamp_start_1 = Utc::now().timestamp_millis();
    let encrypted_words = hash_words_rayon(&words_to_encrypt, &customer_key)?;
    let timestamp_end_1 = Utc::now().timestamp_millis();
    println!("encrypt_words :: diff [{}] ms", timestamp_end_1 - timestamp_start_1);

    //dbg!(&encrypted_words);

    let timestamp_start_2 = Utc::now().timestamp_millis();
    let complete_phrase = replace_words_in_phrase(phrase, &encrypted_words)?;
    let timestamp_end_2 = Utc::now().timestamp_millis();
    println!("replace_words_in_phrase :: diff [{}] ms", timestamp_end_2 - timestamp_start_2);

    Ok(complete_phrase)
}

#[cfg(test)]
mod file_server_test {
    use std::collections::HashMap;

    use chrono::Utc;

    use crate::char_lib::has_not_printable_char;
    use crate::ft_tokenizer::{encrypt_tsvector, FTTokenizer};

    const KEY: &str = "fqYVyce-Nh0HwpPQ7ZGZLog5s7PBLnwFMAW2OMnNPUs";

    #[test]
    fn crypto_perf() -> anyhow::Result<()> {
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

        let mut words_to_encrypt: HashMap<u64, String> = HashMap::new();
        for (index, &phrase) in phrases.iter().enumerate() {
            words_to_encrypt.insert(index as u64, String::from(phrase));
        }

        let timestamp_start_1 = Utc::now().timestamp_millis();
        // let encrypted_words = encrypt_words(&words_to_encrypt, KEY)?;
        let timestamp_end_1 = Utc::now().timestamp_millis();
        println!("encrypt_words :: diff [{}] ms", timestamp_end_1 - timestamp_start_1);

        let timestamp_start_0 = Utc::now().timestamp_millis();
        // let encrypted_words = encrypt_words_rayon(&words_to_encrypt, KEY)?;
        let timestamp_end_0 = Utc::now().timestamp_millis();
        println!("encrypt_words_rayon :: diff [{}] ms", timestamp_end_0 - timestamp_start_0);

        Ok(())
    }

    #[test]
    fn tokenize_garbage() {
        let garbage_1 = "On [ne] sera jamais l'élite de la nation";
        let garbage_1_tokens = vec!["sera", "jamais", "élite", "nation"];

        let garbage_2 = "On [ne] sera jamais l'élite de la nation😈";
        let garbage_2_tokens = vec!["sera", "jamais", "élite", "nation"];

        let garbage_3 = "On [ne] sera, jamais l'élite de la नमस्ते😈";
        let garbage_3_tokens = vec!["sera", "jamais", "élite"];

        let mut tkn = FTTokenizer::new(&garbage_1);
        let garbage_1_words: Vec<String> = tkn.next_n_words(garbage_1.len());
        assert_eq!(garbage_1_tokens, garbage_1_words);

        let mut tkn = FTTokenizer::new(&garbage_2);
        let garbage_2_words: Vec<String> = tkn.next_n_words(garbage_2.len());
        assert_eq!(garbage_2_tokens, garbage_2_words);

        let mut tkn = FTTokenizer::new(&garbage_3);
        let garbage_3_words: Vec<String> = tkn.next_n_words(garbage_3.len());
        assert_eq!(garbage_3_tokens, garbage_3_words);
    }

    #[test]
    fn tokenize_date() {
        let case_1 = "On ne sera, jamais le 12/13/2009";
        let tokens_1 = vec!["sera", "jamais", "12/13/2009"];

        let case_2 = "2009/10/01 n'est pas férié";
        let tokens_2 = vec!["2009/10/01", "férié"];

        let case_3 = "20/10/01 est bizarre";
        let tokens_3 = vec!["20/10/01", "bizarre"];

        let case_4 = "2010-10-01 est normal";
        let tokens_4 = vec!["2010-10-01", "normal"];

        let test_cases: Vec<(&str, Vec<&str>)> =
            vec![(case_1, tokens_1), (case_2, tokens_2), (case_3, tokens_3), (case_4, tokens_4)];

        for case in test_cases {
            let mut tkn = FTTokenizer::new(case.0);
            let words: Vec<String> = tkn.next_n_words(case.0.len());
            assert_eq!(case.1, words);
        }
    }

    #[test]
    fn tokenize_number() {
        let case_1 = "51234567890.25 est une sacrée somme.Mais bon !";
        let tokens_1 = vec!["51234567890.25", "sacrée", "somme", "Mais"];

        let case_2 = "Il me doit -51,01 €";
        let tokens_2 = vec!["doit", "-51,01"];

        let case_3 = "Il me doit 1,235,458,456 €";
        let tokens_3 = vec!["doit", "1,235,458,456"];

        let case_4 = "Il me doit 1.235.458.456 €";
        let tokens_4 = vec!["doit", "1.235.458.456"];

        let case_5 = "C'est bien 5 cts et non pas 5 francs";
        let tokens_5 = vec!["bien", "francs"];

        let case_6 = "C'est le bien-être 5-0 cts et non pas 5.0-1 francs";
        let tokens_6 = vec!["bienêtre", "5.0-1", "francs"];

        let case_7 = "Il me doit +51,01 €";
        let tokens_7 = vec!["doit", "51,01"];

        let test_cases: Vec<(&str, Vec<&str>)> = vec![
            (case_1, tokens_1),
            (case_2, tokens_2),
            (case_3, tokens_3),
            (case_4, tokens_4),
            (case_5, tokens_5),
            (case_6, tokens_6),
            (case_7, tokens_7),
        ];

        for case in test_cases {
            let mut tkn = FTTokenizer::new(case.0);
            let words: Vec<String> = tkn.next_n_words(case.0.len());
            println!("{:?}", &words);
            assert_eq!(case.1, words);
        }
    }

    #[test]
    fn tokenize_mixed() {
        let case_1 = "-5.00 10-12-2010-1 l'élement 241-3";
        let tokens_1 = vec!["-5.00", "10-12-2010-1", "élement", "241-3"];

        let case_2 = "arc-en-ciel -5....00 10-12-2010-1, ";
        let tokens_2 = vec!["arcenciel", "-5....00", "10-12-2010-1"];

        let case_3 = "arc-en-ciel -5....00 10-12-2010-1. ";
        let tokens_3 = vec!["arcenciel", "-5....00", "10-12-2010-1"];

        let case_4 = "[\"-55.2\"][12-05]";
        let tokens_4 = vec!["-55.2", "12-05"];

        let case_5 = "un ╫ c'est mieux qu'un σ";
        let tokens_5 = vec!["mieux"];

        let case_6 = "B10-12-2010-1.ABC";
        let tokens_6 = vec!["B101220101"];

        let case_7 = "B10.12.2010-1 06.10.53.81.30";
        let tokens_7 = vec!["12.2010-1", "06.10.53.81.30"];

        let test_cases: Vec<(&str, Vec<&str>)> = vec![
            (case_1, tokens_1),
            (case_2, tokens_2),
            (case_3, tokens_3),
            (case_4, tokens_4),
            (case_5, tokens_5),
            (case_6, tokens_6),
            (case_7, tokens_7),
        ];

        for case in test_cases {
            let mut tkn = FTTokenizer::new(case.0);
            let words: Vec<String> = tkn.next_n_words(case.0.len());
            println!("{:?}", &words);
            assert_eq!(case.1, words);
        }
    }

    #[test]
    fn tokenize_email() {
        let case_1 = "denis@isd.lu";
        let tokens_1 = vec!["denis@isd.lu"];

        let case_2 = "Ægon @Ægon";
        let tokens_2 = vec!["Ægon", "@Ægon"];

        let case_3 = "denis@isd.lu @Tarzoun";
        let tokens_3 = vec!["denis@isd.lu", "@Tarzoun"];

        let test_cases: Vec<(&str, Vec<&str>)> = vec![(case_1, tokens_1), (case_2, tokens_2), (case_3, tokens_3)];

        for case in test_cases {
            let mut tkn = FTTokenizer::new(case.0);
            let words: Vec<String> = tkn.next_n_words(case.0.len());
            assert_eq!(case.1, words);
        }
    }

    #[test]
    fn tokenize_unicode() {
        let case_1 = "Le montant de ¥en";
        let tokens_1 = vec!["montant"];

        let case_2 = "Ægon le grand";
        let tokens_2 = vec!["Ægon", "grand"];

        let case_3 = "Добрый день,Добрый день,";
        let tokens_3 = vec!["Добрый", "день", "Добрый", "день"];

        let case_4 = "https://doka.eu/get";
        let tokens_4 = vec!["https", "doka"];

        let case_5 = "Català Mìng-dĕ̤ng-ngṳ̄ Нохчийн";
        let tokens_5 = vec!["Català", "Mìngdĕ̤ngngṳ̄", "Нохчийн"];

        let test_cases: Vec<(&str, Vec<&str>)> =
            vec![(case_1, tokens_1), (case_2, tokens_2), (case_3, tokens_3), (case_4, tokens_4), (case_5, tokens_5)];

        for case in test_cases {
            let mut tkn = FTTokenizer::new(case.0);
            let words: Vec<String> = tkn.next_n_words(case.0.len());
            assert_eq!(case.1, words);
        }

        let a = "\u{5B4}\u{FC}";
        println!("Code : {:?}", &a);
    }

    #[test]
    #[ignore]
    fn tokenize_big_planet() -> anyhow::Result<()> {
        let byte_buf: String = std::fs::read_to_string("/mnt/blob/Upload/tokenizer_tests/content.planet.txt")?;
        let mut tkn = FTTokenizer::new(&byte_buf);
        let words: Vec<String> = tkn.next_n_words(5_000);
        println!("PART 1 => {:?}", words);

        let words: Vec<String> = tkn.next_n_words(10);
        println!("PART 2 => {:?}", words);

        let words: Vec<String> = tkn.next_n_words(4);
        println!("PART 3 => {:?}", words);

        let words: Vec<String> = tkn.next_n_words(20);
        println!("PART 4 => {:?}", words);

        let words: Vec<String> = tkn.next_n_words(10);
        println!("PART 5 => {:?}", words);

        Ok(())
    }

    ///
    /// Read the tsvector
    ///
    #[test]
    pub fn tsvector_encrypt() {
        let s = r#"'06/05/22':25,455 '1179592'   :  20,450  'accompani':35::2,782 'a:c c''''ount':182,612 'admiss':3,269,347,433,699
                '06/05/22':25,455 '1179592':20,450 '1740':29,459 '41.7':16,446 '7.00':22,452 '839370784507':430
                'accompani':352,782 'account':182,612 'admiss':3,269,347,433,699,777 'admitt':83,513 'adult':355,785 'ahead':221,651
                'alcohol':114,544 'allow':54,59,108,484,489,538 'also':36,466 'amend':385,815 'amir':415 'anoth':354,784
                'approv':93,168,523,598 'apromot':12,442 'arriv':318,748 'artist':234,250,664,680
                'ateli':4,27,224,391,400,403,408,414,426,434,457,654,821,830,833"#;

        let phrase = encrypt_tsvector(s, "O27AYTdNPNbG-7olPOUxDNb6GNnVzZpbGRa4qkhJ4BU").unwrap();
        // println!("Replaced text => {:?}", &phrase);
        const ANSWER: &str = "'V6E6QNIwsk0RerdDEorVZZwwcKHrPfVH0GxBpHKiGaI':25,455 'B0o7usyhsLqFbkB-IBpmwWjb0szRRi4G4ClJtCev5F4':  20,450  '0IbcJjDRJ_mpwq06rF6K15GLL3iBXrWtkfkWaR-ZgQw':352,782 'nrirRUsuVFQw68RIWJKRYvKfpdXiyKA59MGn8G3BA4k':182,612 '1q4rLmSCsechwllc7Qyw4hL56GO4QrOfHk0j-QhCTgs':3,269,347,433,699
                'V6E6QNIwsk0RerdDEorVZZwwcKHrPfVH0GxBpHKiGaI':25,455 'B0o7usyhsLqFbkB-IBpmwWjb0szRRi4G4ClJtCev5F4':20,450 'hiVkCoU_uGlS-TJnTMHGqUicSEkGUQW8FgBw-EWBatU':29,459 'rYGCosIMtPXIS6HRJdzKW3iAVJ_HPjTpgCVh8swXmcs':16,446 'YUUiFeYTL92CmI0EuoN6IeBGk70kKledU37DkMrHgOM':22,452 '8N--KGaNkjQ9yWBBH5vKv4B8Mh4ZPhFHJNmzVmMZs2E':430
                '0IbcJjDRJ_mpwq06rF6K15GLL3iBXrWtkfkWaR-ZgQw':352,782 'UJvwgsuJ5w-JBhbYVTCjoqejg90onak4PI9xvzQFKCo':182,612 '1q4rLmSCsechwllc7Qyw4hL56GO4QrOfHk0j-QhCTgs':3,269,347,433,699,777 'cWuk0t7UmBkLjf5DNoHfLGmohFVO9_YUMPd5vrbHJa4':83,513 'wKt39_BN995RF37twUaU0sT3_2Hpd5CzqDzC5K5VFBI':355,785 'izMtjfQ5Ewsc-rQ4kDuEKngJgC5LuUSaOflzyeWeH78':221,651
                'qKYCPfaQ1FfBygVSv0Enfa5uJBpvGAFKEUhFgwW0z24':114,544 'SaCrcnB3_GdW1PX0Rv85syPa-iSFuNV-7d29PL6IyrI':54,59,108,484,489,538 'w5f8nGIdFUJLHjQD-7i2j2Hw0RZALGuDw-oO2eJlMrI':36,466 'Xnzsoqk1IxfZHJ6J6ORkeIBPpsZmqeZiI-Ro-fv7N68':385,815 'Ab2uC3AK199UP2XPsbpTHS25-mcX3nhn-mz0wRUPlPw':415 'rc3TygW7HdgZ3b73-RTpQbd2v9gAru1VvdIctaxBDbU':354,784
                'b_mx8BtiCxVDBPXpBzyS9p6dWEHdELdNwoabK6LaId8':93,168,523,598 'EHEmAect8Peh8s-GYnTaN_5RxX8RrFub3nSTXo29Pb8':12,442 'mOR_-o5MBxOUg34OuPEXrm6Degx99skNqcfDLYu13uo':318,748 '9YGM72hArliRJxs1ipF4stQyKRru_r7ObSMUrH0U9y4':234,250,664,680
                'K2GbNko0Ssn0dZXyiIbdrll4bPz-MjMfKgnL4oYI1ig':4,27,224,391,400,403,408,414,426,434,457,654,821,830,833";
        assert_eq!(ANSWER, &phrase);
    }

    #[test]
    pub fn simple_grapheme() {
        let my_str_1 = "denis 😎 papin\n";
        let my_str_2 = "denis😎papin";

        println!("[{}] Has not printable char = {:?}", my_str_1, has_not_printable_char(my_str_1));
        println!("[{}] Has not printable char = {:?}", my_str_2, has_not_printable_char(my_str_2));
    }
}
