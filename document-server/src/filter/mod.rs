use crate::filter::filter_ast::{parse_tokens, ComparisonOperator, FilterCondition, FilterExpressionAST};
use crate::filter::filter_lexer::{lex3, FilterError};
use crate::filter::filter_normalizer::normalize_lexeme;
use crate::parser_log;
use commons_error::*;
use log::error;

pub(crate) mod filter_ast;
pub(crate) mod filter_lexer;
pub(crate) mod filter_normalizer;

pub(crate) fn analyse_expression(expression: &str) -> Result<Box<FilterExpressionAST>, FilterError> {
    parser_log!("Analysing the expression : {:?}", expression; 5);

    match lex3(expression) {
        Ok(mut tokens) => {
            normalize_lexeme(&mut tokens);
            parse_tokens(&mut tokens)
        }
        Err(e) => {
            log_error!("Lexer error : {:?}", e);
            Err(e)
        }
    }
}

pub(crate) fn to_sql_form(filter_expression: &FilterExpressionAST) -> Result<String, FilterError> {
    let mut content: String = String::from("");
    match filter_expression {
        FilterExpressionAST::Condition(FilterCondition { key: _, attribute, operator, value }) => {
            let sql_op = match operator {
                ComparisonOperator::EQ => "=",
                ComparisonOperator::NEQ => "<>",
                ComparisonOperator::GT => ">",
                ComparisonOperator::LT => "<",
                ComparisonOperator::GTE => ">=",
                ComparisonOperator::LTE => "<=",
                ComparisonOperator::LIKE => "LIKE",
            };

            let s = format!("({} {} {})", attribute, sql_op, value);
            content.push_str(&s);
        }
        FilterExpressionAST::Logical { operator, leaves } => {
            content.push_str("(");

            for (i, l) in leaves.iter().enumerate() {
                let r_leaf_content = to_sql_form(l);
                if let Ok(leaf) = r_leaf_content {
                    content.push_str(&leaf);
                }
                if i < leaves.len() - 1 {
                    content.push_str(&format!(" {:?} ", &operator));
                }
            }
            content.push_str(")");
        }
    }
    Ok(content)
}

#[cfg(test)]
mod tests {

    // cargo test --color=always --bin document-server filter  [ -- --show-output]

    use crate::filter::filter_ast::to_canonical_form;
    use crate::filter::analyse_expression;
    use crate::filter::filter_lexer::FilterErrorCode;
    use crate::parser_log;
    use commons_error::*;
    use log::*;
    use std::sync::Once;

    static INIT_LOGGER: Once = Once::new();

    pub(crate) fn init_logger() {
        INIT_LOGGER.call_once(|| {
            if let Err(e) = log4rs::init_file(
                // "/home/denis/Projects/wks-doka-one/doka.one/document-server/log4rs.yaml",
                r#"C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\log4rs.yaml"#,
                Default::default(),
            ) {
                eprintln!("logger init skipped: {:?}", e);
            }
        });
    }

    // Failure case

    #[test]
    pub fn analyse_fail() {
        init_logger();
        let input = "(A LIKE )";
        match analyse_expression(input) {
            Ok(_) => {
                assert_eq!(false, true)
            }
            Err(e) => {
                let e_msg = e.human_error_message();
                log_debug!("Error : {}", &e_msg);
                assert_eq!("The value in the condition is not a valid number at position 9", e_msg);
            }
        }
    }

    #[test]
    pub fn analyse_fail_1() {
        init_logger();
        log_debug!("Start analyse fail 1");
        let input = "()(A == 12)";
        match analyse_expression(input) {
            Ok(ast) => {
                let canonical1 = to_canonical_form(ast.as_ref()).unwrap();
                parser_log!("Result : {}", canonical1; 0);
            }
            Err(e) => {
                let e_msg = e.human_error_message();
                log_debug!("Error : {}", &e_msg);
                assert_eq!("An opening parenthesis was expected at position 1", e_msg);
            }
        }
    }

    #[test]
    pub fn analyse_fail_2() {
        init_logger();
        let input = "A ==() 12";
        match analyse_expression(input) {
            Ok(ast) => {
                let canonical1 = to_canonical_form(ast.as_ref()).unwrap();
                parser_log!("Result : {}", canonical1; 0);
            }
            Err(e) => {
                let e_msg = e.human_error_message();
                log_debug!("Error : {}", &e_msg);
                assert_eq!("The value in the condition is not a valid number at position 5", e_msg);
            }
        }
    }

    #[test]
    pub fn analyse_fail_3() {
        init_logger();
        let input = "A 12";
        match analyse_expression(input) {
            Ok(ast) => {
                let canonical1 = to_canonical_form(ast.as_ref()).unwrap();
                parser_log!("Result : {}", canonical1; 0);
            }
            Err(e) => {
                let e_msg = e.human_error_message();
                log_debug!("Error : {}", &e_msg);
                assert_eq!("Unknown filter operator at position 3", e_msg);
            }
        }
    }

    #[test]
    pub fn analyse_fail_4() {
        init_logger();
        let input = "A == 12)";
        match analyse_expression(input) {
            Ok(ast) => {
                let canonical1 = to_canonical_form(ast.as_ref()).unwrap();
                parser_log!("Result : {}", canonical1; 0);
            }
            Err(e) => {
                let e_msg = e.human_error_message();
                log_debug!("Error : {}", &e_msg);
                assert_eq!("Too many parenthesis at position 8", e_msg);
            }
        }
    }

    // === IT-F0001 : Filter escaping (#", #%, ##) ===

    fn assert_canonical(input: &str, expected: &str) {
        match analyse_expression(input) {
            Ok(ast) => {
                let canonical = to_canonical_form(ast.as_ref()).unwrap();
                assert_eq!(expected, &canonical);
            }
            Err(e) => panic!("Unexpected error for `{}`: {}", input, e.human_error_message()),
        }
    }

    fn assert_error_code(input: &str, expected: FilterErrorCode) {
        match analyse_expression(input) {
            Ok(_) => panic!("Expected error for `{}`, got Ok", input),
            Err(e) => assert_eq!(expected, e.error_code, "input `{}` msg=`{}`", input, e.human_error_message()),
        }
    }

    #[test]
    pub fn it_f0001_001_escape_double_quote() {
        init_logger();
        assert_canonical(
            r#"(category == "super #"extra#" player")"#,
            r#"[category<EQ>super "extra" player]"#,
        );
    }

    #[test]
    pub fn it_f0001_002_escape_percent_outside_like() {
        init_logger();
        assert_canonical(r#"(percent == "less than 10 #%")"#, "[percent<EQ>less than 10 %]");
    }

    #[test]
    pub fn it_f0001_003_escape_hash() {
        init_logger();
        assert_canonical(r#"(comment == "see paragraph ##6")"#, "[comment<EQ>see paragraph #6]");
    }

    #[test]
    pub fn it_f0001_004_multiple_escapes() {
        init_logger();
        assert_canonical(
            r#"(note == "100#% ##1 says #"hi#"")"#,
            r#"[note<EQ>100% #1 says "hi"]"#,
        );
    }

    #[test]
    pub fn it_f0001_005_escapes_with_and() {
        init_logger();
        assert_canonical(
            r#"(category == "super #"extra#" player") AND (percent == "less than 10 #%")"#,
            r#"([category<EQ>super "extra" player]AND[percent<EQ>less than 10 %])"#,
        );
    }

    #[test]
    pub fn it_f0001_006_plain_string_regression() {
        init_logger();
        assert_canonical(r#"(name == "denis")"#, "[name<EQ>denis]");
    }

    #[test]
    pub fn it_f0001_007_hash_then_digit_is_error() {
        init_logger();
        // `#` is at position 28 in `+(comment == "see paragraph #6")`
        match analyse_expression(r#"(comment == "see paragraph #6")"#) {
            Ok(_) => panic!("Expected error"),
            Err(e) => {
                assert_eq!(FilterErrorCode::InvalidEscapeSequence, e.error_code);
                assert_eq!(28, e.char_position);
                assert_eq!(
                    "Invalid escape sequence in string literal at position 28",
                    e.human_error_message()
                );
            }
        }
    }

    #[test]
    pub fn it_f0001_008_hash_then_letter_is_error() {
        init_logger();
        // `#` is at position 15 in `+(comment == "C#a")`
        match analyse_expression(r#"(comment == "C#a")"#) {
            Ok(_) => panic!("Expected error"),
            Err(e) => {
                assert_eq!(FilterErrorCode::InvalidEscapeSequence, e.error_code);
                assert_eq!(15, e.char_position);
            }
        }
    }

    #[test]
    pub fn it_f0001_009_hash_then_space_is_error() {
        init_logger();
        // `#` is at position 19 in `+(comment == "lone # in middle")`
        match analyse_expression(r#"(comment == "lone # in middle")"#) {
            Ok(_) => panic!("Expected error"),
            Err(e) => {
                assert_eq!(FilterErrorCode::InvalidEscapeSequence, e.error_code);
                assert_eq!(19, e.char_position);
            }
        }
    }

    #[test]
    pub fn it_f0001_010_trailing_hash_unclosed() {
        init_logger();
        assert_error_code(r#"(comment == "hello #")"#, FilterErrorCode::UnclosedQuote);
    }

    #[test]
    pub fn it_f0001_011_bare_percent_outside_like_is_error() {
        init_logger();
        // `%` is at position 14 in `+(name == "100%")`
        match analyse_expression(r#"(name == "100%")"#) {
            Ok(_) => panic!("Expected error"),
            Err(e) => {
                assert_eq!(FilterErrorCode::InvalidEscapeSequence, e.error_code);
                assert_eq!(14, e.char_position);
            }
        }
    }

    #[test]
    pub fn it_f0001_012_like_wildcard_regression() {
        init_logger();
        assert_canonical(r#"(name LIKE "den%")"#, "[name<LIKE>den%]");
    }

    #[test]
    pub fn it_f0001_013_literal_percent_inside_like() {
        init_logger();
        assert_canonical(r#"(code LIKE "50#%")"#, "[code<LIKE>50%]");
    }

    #[test]
    pub fn it_f0001_014_backslash_is_not_escape() {
        init_logger();
        // `\` has no escape status: the first `"` after `\` closes the string,
        // and the rest (`hi\"`) is invalid — guards against any regression
        // toward a `\"` escape proposal.
        let input = r#"(comment == "she said \"hi\"" AND name == "x")"#;
        match analyse_expression(input) {
            Ok(ast) => panic!(
                "Expected parsing to fail, got: {}",
                to_canonical_form(ast.as_ref()).unwrap()
            ),
            Err(_) => {}
        }
    }
}
