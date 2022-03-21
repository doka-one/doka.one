
pub fn valid_password( pass : &str ) -> bool {

    let special_chars : Vec<char> = vec!['€', '$', '%', ';', ',', '.', ':', '_', '-', '/', '&', '!', '?', '#', '*', '+' ];

    if pass.chars().count() < 8 {
        return false;
    }

    let mut valid : bool = true;
    let mut nb_upper : u16 = 0u16;
    let mut nb_digit : u16 = 0u16;
    let mut nb_symbol : u16 = 0u16;
    for c in pass.chars() {

        match is_latin_base_char(c) {
            CharType::DIGIT => {
                nb_digit += 1;
                continue;
            }
            CharType::UPPER => {
                nb_upper += 1;
                continue;
            }
            CharType::LOWER => {
                continue;
            }
            CharType::WRONG => {
            }
        }

        if special_chars.contains(&c) {
            nb_symbol += 1;
            continue;
        }

        valid = false;
        break;
    }

    if nb_upper == 0u16  || nb_digit == 0u16 || nb_symbol == 0u16 {
        return false;
    }

    valid
}

enum CharType {
    DIGIT,
    UPPER,
    LOWER,
    WRONG,
}


fn is_latin_base_char(c : char) -> CharType {

    let b = c as u32;
    // 0 to 9
    if b >= 48  && b <= 57 {
        return CharType::DIGIT;
    }

    // A to Z
    if b >= 65  && b <= 90 {
        return CharType::UPPER;
    }

    // a to z
    if b >= 97  && b <= 122 {
        return CharType::LOWER;
    }

    CharType::WRONG
}


#[cfg(test)]
mod tests {
    use crate::dk_password::valid_password;

    #[test]
    fn many_special_chars() {
        let pass1 = "$%$$&AA99";
        let test1 = valid_password(pass1);
        assert_eq!(true, test1);
    }

    #[test]
    fn many_special_chars_2() {
        let pass1 = "$%$$&AA99-*+";
        let test1 = valid_password(pass1);
        assert_eq!(true, test1);
    }

    #[test]
    fn forbidden_char() {
        let pass1 = "a%AA$123<4567";
        let test1 = valid_password(pass1);
        assert_eq!(false, test1);

        let pass1 = "a%AA$1234567";
        let test1 = valid_password(pass1);
        assert_eq!(true, test1);
    }

    #[test]
    fn at_leat_one_digit() {
        let pass1 = "A%AABBBCC";
        let test1 = valid_password(pass1);
        assert_eq!(false, test1);

        let pass1 = "A%AABBBCC1";
        let test1 = valid_password(pass1);
        assert_eq!(true, test1);
    }

    #[test]
    fn at_leat_one_upper() {
        let pass1 = "a%aaabbcc1";
        let test1 = valid_password(pass1);
        assert_eq!(false, test1);

        let pass1 = "a%aaAbbcC1";
        let test1 = valid_password(pass1);
        assert_eq!(true, test1);
    }

    #[test]
    fn at_leat_one_symbol() {
        let pass1 = "aaaAbbcC1";
        let test1 = valid_password(pass1);
        assert_eq!(false, test1);
    }

    #[test]
    fn password_too_short() {
        let pass1 = "%23456A";
        let test1 = valid_password(pass1);
        assert_eq!(false, test1);

        let pass1 = "%23456AB";
        let test1 = valid_password(pass1);
        assert_eq!(true, test1);
    }

}