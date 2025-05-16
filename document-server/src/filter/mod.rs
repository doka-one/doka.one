use crate::filter::filter_ast::{
    parse_tokens, ComparisonOperator, FilterCondition, FilterExpressionAST,
};
use crate::filter::filter_lexer::FilterErrorCode::EmptyCondition;
use crate::filter::filter_lexer::{lex3, FilterError, FilterErrorCode, LogicalOperator};
use crate::filter::filter_normalizer::normalize_lexeme;
use crate::parser_log;
use chrono::format::Numeric::Second;
use commons_error::*;
use dkdto::{ClearTextReply, TagElement, TagType};
use log::*;
use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use std::fmt;

pub(crate) mod filter_ast;
pub(crate) mod filter_lexer;
pub(crate) mod filter_normalizer;

pub(crate) fn analyse_expression(
    expression: &str,
) -> Result<Box<FilterExpressionAST>, FilterError> {
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
        FilterExpressionAST::Condition(FilterCondition {
            key,
            attribute,
            operator,
            value,
        }) => {
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
    use crate::filter::{analyse_expression, ComparisonOperator, FilterExpressionAST};
    use crate::parser_log;
    use commons_error::*;
    use log::*;
    use std::sync::Once;

    static INIT_LOGGER: Once = Once::new();

    pub(crate) fn init_logger() {
        INIT_LOGGER.call_once(|| {
            if let Err(e) = log4rs::init_file(
                "/home/denis/Projects/wks-doka-one/doka.one/document-server/log4rs.yaml",
                // r#"C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\log4rs.yaml"#,
                Default::default(),
            ) {
                panic!("{:?}", e);
            }
        });
    }

    // Failure case

    #[test]
    pub fn analyse_fail() {
        init_logger();
        let input = "(A LIKE )";
        match analyse_expression(input) {
            Ok(ast) => {
                assert_eq!(false, true)
            }
            Err(e) => {
                let e_msg = e.human_error_message();
                log_debug!("Error : {}", &e_msg);
                assert_eq!(
                    "The value in the condition is not a valid number at position 9",
                    e_msg
                );
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
                assert_eq!(
                    "The value in the condition is not a valid number at position 5",
                    e_msg
                );
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
}
