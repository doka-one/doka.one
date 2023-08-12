pub type LanguageCode = (&'static str, &'static str, &'static str);

pub const ARABIC: LanguageCode = ("arabic", "ar", "ara");
pub const DANISH: LanguageCode = ("danish", "da", "dan");
pub const DUTCH: LanguageCode = ("dutch", "nl", "nld");
pub const ENGLISH: LanguageCode = ("english", "en", "eng");
pub const FINNISH: LanguageCode = ("finnish", "fi", "fin");
pub const FRENCH: LanguageCode = ("french", "fr", "fra");
pub const GERMAN: LanguageCode = ("german", "de", "deu");
pub const GREEK: LanguageCode = ("greek", "el", "ell");
pub const HUNGARIAN: LanguageCode = ("hungarian", "hu", "hun");
pub const INDONESIAN: LanguageCode = ("indonesian", "id", "ind");
pub const IRISH: LanguageCode = ("irish", "ga", "gle");
pub const ITALIAN: LanguageCode = ("italian", "it", "ita");
pub const LITHUANIAN: LanguageCode = ("lithuanian", "lt", "lit");
pub const NEPALI: LanguageCode = ("nepali", "ne", "nep");
pub const NORWEGIAN: LanguageCode = ("norwegian", "nb", "nob");
pub const PORTUGUESE: LanguageCode = ("portuguese", "pt", "por");
pub const ROMANIAN: LanguageCode = ("romanian", "ro", "ron");
pub const RUSSIAN: LanguageCode = ("russian", "ru", "rus");
pub const SPANISH: LanguageCode = ("spanish", "es", "spa");
pub const SWEDISH: LanguageCode = ("swedish", "sv", "swe");
pub const TAMIL: LanguageCode = ("tamil", "ta", "tam");
pub const TURKISH: LanguageCode = ("turkish", "tr", "tur");

pub static LANGUAGES: [LanguageCode; 22] = [ARABIC, DANISH, DUTCH, ENGLISH, FINNISH, FRENCH, GERMAN, GREEK,
                                        HUNGARIAN, INDONESIAN, IRISH, ITALIAN, LITHUANIAN, NEPALI, NORWEGIAN, PORTUGUESE,
                                        ROMANIAN, RUSSIAN, SPANISH, SWEDISH, TAMIL, TURKISH];

// pub(crate) fn lang_code_2_from( lang_name: &'_ str) -> &'_ str {
//     let mut found_lg = ENGLISH;
//     for lg in LANGUAGES {
//         if lg.0 == lang_name {
//             found_lg = lg;
//         }
//     }
//     found_lg.1
// }

// pub(crate) fn lang_code_3_from( lang_name: &'_ str) -> &'_ str {
//     let mut found_lg = ENGLISH;
//     for lg in LANGUAGES {
//         if lg.0 == lang_name {
//             found_lg = lg;
//         }
//     }
//     found_lg.2  // Code 3
// }

pub(crate) fn lang_name_from_code_2( lang_code_2: &'_ str) -> &'_ str {
    search_from_code_2(lang_code_2).0
}

///
/// (private) Find the language Code from the code-2 iso
///
fn search_from_code_2( lang_code_2: &'_ str) -> LanguageCode {
    let mut found_lg = ENGLISH;
    for lg in LANGUAGES {
        if lg.1 == lang_code_2 {
            found_lg = lg;
            break;
        }
    }
    found_lg
}

///
/// From the lang code returned by Tika, we find a lang code that is relevant for PGSQL
/// We also map some languages with substitution languages (ex . créole => français)
///
pub(crate) fn map_code(lang_code_2: &'_ str) -> &'_ str {
    match lang_code_2 {
        // Créole haïtien
        "ht" => {
            FRENCH.1
        }
        _ => {
            search_from_code_2(lang_code_2).1
        }
    }
}

#[cfg(test)]
mod test {
    // use lingua::{Language, LanguageDetectorBuilder};
    // use lingua::Language::{English, French, German, Spanish};
    use crate::language::{lang_name_from_code_2, map_code};

// #[test]
    // fn test_search_code2() {
    //     let code = lang_code_2_from("french");
    //     assert_eq!("fr", code);
    // }

    #[test]
    fn test_search_name() {
        let name = lang_name_from_code_2("el");
        assert_eq!("greek", name);
    }

    #[test]
    fn test_map_code() {
        let code = map_code("el");
        assert_eq!("el", code);

        let code = map_code("ht");
        assert_eq!("fr", code);

        let code = map_code("it");
        assert_eq!("it", code);

        let code = map_code("sw");
        assert_eq!("en", code);
    }

    // #[test]
    // fn language_detection() {
    //     let languages = vec![English, French, German, Spanish];
    //     let detector = LanguageDetectorBuilder::from_languages(&languages).build();
    //     let found_language: Option<Language> = detector.detect_language_of("languages are awesome");
    //
    //     dbg!(found_language);
    // }
    //
    //
    // #[test]
    // fn detect_language() {
    //     let languages = vec![English, French, German, Spanish];
    //     let detector = LanguageDetectorBuilder::from_languages(&languages).build();
    //     let confidence_values: Vec<(Language, f64)> = detector.compute_language_confidence_values(
    //         "languages are awesome"
    //     );
    //     dbg!(confidence_values);
    // }
}