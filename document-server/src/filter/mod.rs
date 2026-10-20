use crate::filter::filter_ast::{parse_tokens, ComparisonOperator, FilterCondition, FilterExpressionAST, FilterValue};
use crate::filter::filter_lexer::{lex3, FilterError, PatternPart};
use crate::filter::filter_normalizer::normalize_lexeme;
use crate::parser_log;
use commons_error::*;
use log::error;

pub(crate) mod filter_ast;
pub(crate) mod filter_lexer;
pub(crate) mod filter_normalizer;
pub(crate) mod type_resolver;

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

            let value_sql = to_sql_literal(value, operator);

            let s = format!("({} {} {})", attribute, sql_op, value_sql);
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

/// Render a [`FilterValue`] as a SQL operand (operator + value-literal),
/// applying the escaping rules of F0002.
///
/// Rules applied:
/// - **Rule 1 — `'` doubling.** Every `'` inside a string value becomes `''`.
/// - **Rule 2 — non-`LIKE` operators.** For `=`, `<>`, `>`, `>=`, `<`, `<=`,
///   the characters `%`, `_`, `\`, `"`, `#` have no SQL meaning inside a
///   single-quoted literal and pass through unchanged.
/// - **Rule 3 — `LIKE` emission.** `AnySequence` is rendered as the bare
///   `%` wildcard. Inside `Literal` parts, `%`, `_`, `\` are prefixed with
///   `\`; if any such escape was emitted, the fragment is suffixed with
///   ` ESCAPE '\'`.
/// - **Rule 4 — backslash in non-`LIKE` strings.** Under
///   `standard_conforming_strings = on`, `\` is a plain character and
///   passes through unchanged.
///
/// Numbers and booleans are emitted as unquoted SQL atoms.
///
/// The returned string is ready to be substituted as-is into a SQL
/// fragment of the form `<column> <op> <here>`.
pub(crate) fn to_sql_literal(value: &FilterValue, operator: &ComparisonOperator) -> String {
    if let (ComparisonOperator::LIKE, FilterValue::ValuePattern(parts)) = (operator, value) {
        let mut body = String::new();
        let mut needs_escape = false;
        for part in parts {
            match part {
                PatternPart::AnySequence => body.push('%'),
                PatternPart::Literal(lit) => {
                    for ch in lit.chars() {
                        match ch {
                            '\'' => body.push_str("''"),
                            '%' | '_' | '\\' => {
                                body.push('\\');
                                body.push(ch);
                                needs_escape = true;
                            }
                            _ => body.push(ch),
                        }
                    }
                }
            }
        }
        return if needs_escape {
            format!("'{}' ESCAPE '\\'", body)
        } else {
            format!("'{}'", body)
        };
    }

    match value {
        FilterValue::ValueString(s) => format!("'{}'", s.replace('\'', "''")),
        FilterValue::ValueInt(i) => i.to_string(),
        FilterValue::ValueBool(b) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
        FilterValue::ValueDate(d) => format!("DATE '{}'", d),
        FilterValue::ValuePattern(parts) => {
            // Defensive: the parser only emits `ValuePattern` paired with `LIKE`.
            let mut joined = String::new();
            for part in parts {
                match part {
                    PatternPart::AnySequence => joined.push('%'),
                    PatternPart::Literal(lit) => joined.push_str(lit),
                }
            }
            format!("'{}'", joined.replace('\'', "''"))
        }
    }
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
        // Bare `%` is the AnySequence wildcard, rendered `\%\` in the canonical form.
        // AST: ValuePattern([Literal("den"), AnySequence])
        assert_canonical(r#"(name LIKE "den%")"#, r"[name<LIKE>den\%\]");
    }

    #[test]
    pub fn it_f0001_013_literal_percent_inside_like() {
        init_logger();
        // Escaped `#%` is consumed as a literal percent inside the Literal segment.
        // AST: ValuePattern([Literal("50%")])
        // Distinct from `LIKE "50%"` (which would render as `50\%\`).
        assert_canonical(r#"(code LIKE "50#%")"#, "[code<LIKE>50%]");
    }

    #[test]
    pub fn it_f0001_013b_like_wildcard_surrounding_literals() {
        init_logger();
        // Leading wildcard, embedded escaped percent, trailing wildcard.
        // AST: ValuePattern([AnySequence, Literal("foo%bar"), AnySequence])
        assert_canonical(r#"(name LIKE "%foo#%bar%")"#, r"[name<LIKE>\%\foo%bar\%\]");
    }

    // === IT-F0002 : SQL escaping (DFS → PostgreSQL) ===

    fn assert_sql(input: &str, expected: &str) {
        match analyse_expression(input) {
            Ok(ast) => {
                let sql = super::to_sql_form(ast.as_ref()).unwrap();
                assert_eq!(expected, &sql, "input `{}`", input);
            }
            Err(e) => panic!("Unexpected error for `{}`: {}", input, e.human_error_message()),
        }
    }

    #[test]
    pub fn it_f0002_001_quote_doubling_eq() {
        init_logger();
        assert_sql(r#"(name == "d'arc")"#, "(name = 'd''arc')");
    }

    #[test]
    pub fn it_f0002_002_special_chars_pass_through() {
        init_logger();
        // `#"` is the DFS escape for `"`; after parsing the value is L'"équipe"
        assert_sql(r#"(team == "L'#"équipe#"")"#, r#"(team = 'L''"équipe"')"#);
    }

    #[test]
    pub fn it_f0002_002b_backslash_pass_through_non_like() {
        init_logger();
        // Rule 4: `\` is a plain character inside non-LIKE literals.
        assert_sql(r#"(path == "C:\tmp")"#, r"(path = 'C:\tmp')");
    }

    #[test]
    pub fn it_f0002_003_like_pure_wildcard_no_escape() {
        init_logger();
        assert_sql(r#"(name LIKE "den%")"#, "(name LIKE 'den%')");
    }

    #[test]
    pub fn it_f0002_004_like_literal_percent_with_escape() {
        init_logger();
        assert_sql(r#"(code LIKE "50#%")"#, r"(code LIKE '50\%' ESCAPE '\')");
    }

    #[test]
    pub fn it_f0002_005_like_quote_and_trailing_wildcard() {
        init_logger();
        assert_sql(r#"(name LIKE "L'équipe de moi%")"#, r"(name LIKE 'L''équipe de moi%')");
    }

    #[test]
    pub fn it_f0002_006_like_literal_underscore_with_escape() {
        init_logger();
        assert_sql(r###"(code LIKE "##_x_%")"###, r"(code LIKE '#\_x\_%' ESCAPE '\')");
    }

    #[test]
    pub fn it_f0002_007_like_literal_backslash_with_escape() {
        init_logger();
        assert_sql(r#"(path LIKE "C:\path%")"#, r"(path LIKE 'C:\\path%' ESCAPE '\')");
    }

    #[test]
    pub fn it_f0002_008_neq_with_quote_doubling() {
        init_logger();
        assert_sql(r#"(name != "d'arc")"#, "(name <> 'd''arc')");
    }

    #[test]
    pub fn it_f0002_009_non_like_mixed_quote_and_percent() {
        init_logger();
        assert_sql(r#"(note == "ain't 100#%")"#, "(note = 'ain''t 100%')");
    }

    #[test]
    pub fn it_f0002_010_like_quote_percent_and_escape() {
        init_logger();
        assert_sql(r#"(note LIKE "L'eq 50#%")"#, r"(note LIKE 'L''eq 50\%' ESCAPE '\')");
    }

    #[test]
    pub fn it_f0002_011_plain_value_regression() {
        init_logger();
        assert_sql(r#"(name == "denis")"#, "(name = 'denis')");
    }

    #[test]
    pub fn it_f0002_012_empty_string_literal() {
        init_logger();
        assert_sql(r#"(note == "")"#, "(note = '')");
    }

    #[test]
    pub fn it_f0002_013_composite_and_with_escaping_each_side() {
        init_logger();
        assert_sql(
            r#"(name == "d'arc") AND (code LIKE "50#%")"#,
            r"((name = 'd''arc') AND (code LIKE '50\%' ESCAPE '\'))",
        );
    }

    // === UT-F0005 : Date filter — `to_sql_literal` defensive arm ===

    #[test]
    pub fn ut_f0005_012_to_sql_literal_value_date() {
        use crate::filter::filter_ast::{ComparisonOperator, FilterValue};
        use chrono::NaiveDate;
        let v = FilterValue::ValueDate(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
        let s = super::to_sql_literal(&v, &ComparisonOperator::EQ);
        assert_eq!("DATE '2025-12-31'", s);
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
