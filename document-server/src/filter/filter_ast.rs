use std::cell::RefCell;
use std::fmt;

use crate::filter::filter_lexer::FilterErrorCode::{
    AttributeExpected, ClosingExpected, LogicalOperatorExpected, OpeningExpected, OperatorExpected, ValueExpected,
};
use crate::filter::filter_lexer::{lex3, FilterError, LogicalOperator, Token};
use crate::filter::filter_normalizer::normalize_lexeme;
use crate::parser_log;
use commons_error::*;
use log::*;
use rs_uuid::uuid8;

#[cfg(test)]
const COND_OPEN: &str = "[";
#[cfg(test)]
const COND_CLOSE: &str = "]";
#[cfg(test)]
const LOGICAL_OPEN: &str = "(";
#[cfg(test)]
const LOGICAL_CLOSE: &str = ")";

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ComparisonOperator {
    EQ,
    NEQ,
    GT,
    GTE,
    LT,
    LTE,
    LIKE,
}

#[derive(Debug, Clone)]
pub(crate) enum FilterValue {
    ValueInt(i32),
    ValueString(String),
    ValueBool(bool),
}

impl fmt::Display for FilterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterValue::ValueInt(i) => {
                write!(f, "{}", i)
            }
            FilterValue::ValueString(s) => {
                write!(f, "{}", s.as_str())
            }
            FilterValue::ValueBool(b) => {
                write!(f, "{}", if *b { "TRUE" } else { "FALSE" })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FilterCondition {
    pub(crate) key: String, // a unique key to identify the leaves
    pub(crate) attribute: String,
    pub(crate) operator: ComparisonOperator,
    pub(crate) value: FilterValue,
}

#[derive(Debug)]
pub enum FilterExpressionAST {
    Condition(FilterCondition),
    Logical { operator: LogicalOperator, leaves: Vec<Box<FilterExpressionAST>> },
}

/**
REF_TAG : DOKA_SEARCH_SQL
 */
#[cfg(test)]
pub(crate) fn to_canonical_form(filter_expression: &FilterExpressionAST) -> Result<String, FilterError> {
    let mut content: String = String::from("");
    match filter_expression {
        FilterExpressionAST::Condition(FilterCondition { key, attribute, operator, value }) => {
            let s = format!("{}{}<{:?}>{}{}", COND_OPEN, attribute, operator, value, COND_CLOSE);
            content.push_str(&s);
        }
        FilterExpressionAST::Logical { operator, leaves } => {
            content.push_str(LOGICAL_OPEN);

            for (i, l) in leaves.iter().enumerate() {
                let r_leaf_content = to_canonical_form(l);
                if let Ok(leaf) = r_leaf_content {
                    content.push_str(&leaf);
                }
                if i < leaves.len() - 1 {
                    content.push_str(&format!("{:?}", &operator));
                }
            }
            content.push_str(LOGICAL_CLOSE);
        }
    }
    Ok(content)
}

/// Parse a list of tokens to create the FilterExpression (AST)
/// The list of Tokens must be N3-normalized first.
pub(crate) fn parse_tokens(tokens: &[Token]) -> Result<Box<FilterExpressionAST>, FilterError> {
    let index = RefCell::new(0usize);
    parse_tokens_with_index(&tokens, &index)
}

fn parse_tokens_with_index(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpressionAST>, FilterError> {
    // Read the fist token
    // we start at 0
    let t = tokens.get(*index.borrow());

    if let Some(token) = t {
        match token {
            Token::LogicalOpen(pt) => {
                // The expression starts with a bracket, it's a logical
                log_debug!("found a logical at index {}", *index.borrow());
                let logical_expression = parse_logical(tokens, &index)?;
                log_debug!("logical expression was [{:?}], now index is [{}]", &logical_expression, *index.borrow());
                Ok(logical_expression)
            }
            Token::ConditionOpen(pt) => {
                log_debug!("found a condition at index {}", *index.borrow());
                let c = parse_condition(&tokens, &index)?;
                log_debug!("condition expression was [{:?}], now index is [{}]", &c, *index.borrow());
                Ok(c)
            }
            _ => {
                log_error!("Logical opening expected");
                Err(FilterError { char_position: *index.borrow(), error_code: OpeningExpected })
            }
        }
    } else {
        log_error!("Logical opening expected");
        Err(FilterError { char_position: *index.borrow(), error_code: OpeningExpected })
    }
}

/// At this point we know the tokens starting at <index>
/// are of the form : LO EXPRESSION LOP EXPRESSION LC
fn parse_logical(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpressionAST>, FilterError> {
    log_debug!("parse_logical at [{}]", *index.borrow());

    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow());

    log_debug!("next token is [{:?}]", &t);

    if let Some(token) = t {
        match token {
            Token::ConditionOpen(pt) | Token::LogicalOpen(pt) => {
                // Read the Left member of the Logical Expression
                log_debug!("found a new expression at index {}", *index.borrow());
                let left = parse_tokens_with_index(&tokens, &index)?;
                log_debug!("logical expression_left was [{:?}], now index is [{}]", &left, *index.borrow());

                // here we must found the LOP
                *index.borrow_mut() += 1;
                let op_fop = tokens.get(*index.borrow());

                let operator = if let Some(t_op) = op_fop {
                    match t_op {
                        Token::BinaryLogicalOperator(op) => op,
                        _ => {
                            warn!("Must be an operator");
                            return Err(FilterError {
                                char_position: *index.borrow(),
                                error_code: LogicalOperatorExpected,
                            });
                        }
                    }
                } else {
                    warn!("Must be an operator");
                    return Err(FilterError { char_position: *index.borrow(), error_code: LogicalOperatorExpected });
                }
                .clone();

                log_debug!("Found the logical operator [{:?}], index is [{}]", &operator, *index.borrow());

                // and then the right expression

                *index.borrow_mut() += 1;
                log_debug!("looking for the right expression at index {}", *index.borrow());
                let right = parse_tokens_with_index(&tokens, &index)?;
                log_debug!("logical expression_right was [{:?}], now index is [{}]", &left, *index.borrow());

                // then the logical closing
                *index.borrow_mut() += 1;
                let t = tokens.get(*index.borrow());

                log_debug!("Looking for the logical close at index {}, token=[{:?}]", *index.borrow(), &t);

                if let Some(Token::LogicalClose(_)) = t {
                    Ok(Box::new(FilterExpressionAST::Logical {
                        // FIXME : should keep the position
                        operator: operator.token,  //left,
                        leaves: vec![left, right], //right,
                    }))
                } else {
                    warn!("Expected logical closing");
                    Err(FilterError { char_position: *index.borrow(), error_code: ClosingExpected })
                }
            }
            _ => Err(FilterError { char_position: *index.borrow(), error_code: OpeningExpected }),
        }
    } else {
        log_error!("Logical opening expected");
        Err(FilterError { char_position: *index.borrow(), error_code: OpeningExpected })
    }
}

/// At this point we know the tokens starting at <index>
/// are of the form : C_OPEN ATTRIBUTE  FOP  VALUE C_CLOSE
fn parse_condition(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpressionAST>, FilterError> {
    // Here we know that the form is C_OPEN ATTRIBUTE  FOP  VALUE C_CLOSE
    //
    log_debug!("parse_condition at [{}]", *index.borrow());

    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow());

    log_debug!("next condition token is [{:?}]", &t);

    if let Some(token) = t {
        match token {
            Token::Attribute(attr) => {
                let attribute = attr.clone();
                *index.borrow_mut() += 1;
                let op_fop = tokens.get(*index.borrow());

                let operator = if let Some(t_op) = op_fop {
                    match t_op {
                        Token::Operator(op) => op,
                        _ => {
                            warn!("Must be an comparison operator"); // TODO NORM
                            return Err(FilterError { char_position: *index.borrow(), error_code: OperatorExpected });
                        }
                    }
                } else {
                    warn!("Must be a comparison operator"); // TODO NORM
                    return Err(FilterError { char_position: *index.borrow(), error_code: OperatorExpected });
                }
                .clone();

                log_debug!("comparison operator [{:?}] at [{}]", &operator, *index.borrow());

                *index.borrow_mut() += 1;
                let op_value = tokens.get(*index.borrow());

                let value = if let Some(t_value) = op_value {
                    match t_value {
                        // FIXEME : should keep the position
                        Token::ValueInt(op) => FilterValue::ValueInt(op.clone().token),
                        Token::ValueString(op) => FilterValue::ValueString(op.clone().token),
                        Token::ValueBool(op) => FilterValue::ValueBool(op.clone().token),
                        _ => {
                            warn!("Must be a token value"); // TODO NORM
                            return Err(FilterError { char_position: *index.borrow(), error_code: ValueExpected });
                        }
                    }
                } else {
                    warn!("Must be a value"); // TODO NORM
                    return Err(FilterError { char_position: *index.borrow(), error_code: ValueExpected });
                };

                *index.borrow_mut() += 1;
                let op_value = tokens.get(*index.borrow());

                log_debug!("CLOSE parse_condition at [{}], token=[{:?}]", *index.borrow(), &op_value);
                let key = uuid8();
                Ok(Box::new(FilterExpressionAST::Condition(FilterCondition {
                    key,
                    attribute: attribute.token,
                    operator: operator.token,
                    value,
                })))
            }
            t => {
                warn!("Mysterious Token [{:?}]", t); // TODO NORM
                Err(FilterError { char_position: *index.borrow(), error_code: AttributeExpected })
            }
        }
    } else {
        Err(FilterError { char_position: *index.borrow(), error_code: AttributeExpected })
    }
}

#[cfg(test)]
mod tests {

    //cargo test --color=always --bin document-server filter_ast::tests   -- --show-output

    use crate::filter::filter_ast::Token::{
        Attribute, BinaryLogicalOperator, ConditionClose, ConditionOpen, LogicalClose, LogicalOpen, Operator, ValueInt,
        ValueString,
    };
    use crate::filter::filter_ast::{
        parse_tokens, parse_tokens_with_index, to_canonical_form, ComparisonOperator, FilterError, Token,
    };
    use crate::filter::filter_lexer::{lex3, FilterErrorCode, PositionalToken, TokenSlice};
    use crate::filter::filter_normalizer::normalize_lexeme;
    use crate::filter::tests::init_logger;

    use crate::filter::filter_ast::ComparisonOperator::{EQ, GT, GTE, LIKE, LT};
    use crate::filter::filter_lexer::LogicalOperator::{AND, OR};
    use crate::filter::{analyse_expression, to_sql_form};
    use commons_error::*;
    use log::*;
    use std::cell::RefCell;

    #[test]
    pub fn test_logs() {
        init_logger();
        log_info!("**** test_logs");
    }

    #[test]
    pub fn global_analyser_1() {
        init_logger();
        log_info!("**************************************");
        log_info!("**** global_analyser_1");
        log_info!("**************************************");
        // let input = " age < 40 AND (( limit == 5 OR birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\") ";
        let input1 = " (country == \"FR\"  AND  (science >= 40) OR (lost_in_hell == \"TRUE\") )";
        let input2 = "((country==\"FR\" AND (science>=40)) OR (lost_in_hell==\"TRUE\") )";
        let input3 = "country == \"FR\"  AND  (science => 40) OR (lost_in_hell == \"TRUE\")";
        let input4 = "country == \"FR\"  AND  science >= 40 OR lost_in_hell == \"TRUE\"";

        log_debug!("Analyse...");
        let tree1 = analyse_expression(input1).unwrap();
        let tree2 = analyse_expression(input2).unwrap();
        let tree3 = analyse_expression(input3).unwrap();
        let tree4 = analyse_expression(input4).unwrap();

        let canonical1 = to_canonical_form(tree1.as_ref()).unwrap();
        let canonical2 = to_canonical_form(tree2.as_ref()).unwrap();
        let canonical3 = to_canonical_form(tree3.as_ref()).unwrap();
        let canonical4 = to_canonical_form(tree4.as_ref()).unwrap();

        log_debug!("canonical...{canonical1}");

        let expected = "(([country<EQ>FR]AND[science<GTE>40])OR[lost_in_hell<EQ>TRUE])";
        assert_eq!(expected, &canonical1);
        assert_eq!(expected, &canonical2);
        assert_eq!(expected, &canonical3);
        assert_eq!(expected, &canonical4);
    }

    #[test]
    pub fn global_test_1() {
        init_logger();
        let input = "(age < 40) OR (denis < 5 AND age > 21) AND (detail == 6)";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);

        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR(([denis<LT>5]AND[age<GT>21])AND[detail<EQ>6]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_1_1() {
        init_logger();
        let input = "(age < 40) OR (question == TRUE)";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);

        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR[question<EQ>TRUE])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_2() {
        init_logger();
        let input = "(A < 40) OR (B > 21) AND (C == 6)";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);

        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([A<LT>40]OR([B<GT>21]AND[C<EQ>6]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_2_2() {
        init_logger();
        let input = "(A < 40) AND (B > 21) AND (C == 6)";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);

        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([A<LT>40]AND[B<GT>21])AND[C<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_3() {
        init_logger();
        let input = "((age < 40) OR (age > 21)) AND (detail == 6)";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);
        log_debug!("norm {:?}", &tokens);
        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]OR[age<GT>21])AND[detail<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_4() {
        init_logger();
        let input = "(A < 40) OR (B > 21) OR (C == 6)";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);

        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([A<LT>40]OR[B<GT>21])OR[C<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_5() {
        init_logger();
        let input = "(age < 40 OR (   age > 21 AND detail == \"bonjour\"  )   )";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);
        log_debug!("Norm {}", &TokenSlice(&tokens));
        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR([age<GT>21]AND[detail<EQ>bonjour]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_6() {
        init_logger();
        let input = "age < 40 OR  birthdate >= \"2001-01-01\"  OR  age > 21 AND detail == \"bonjour\"  ";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);
        log_debug!("norm {:?}", &tokens);
        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]OR[birthdate<GTE>2001-01-01])OR([age<GT>21]AND[detail<EQ>bonjour]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_7() {
        init_logger();
        let input = "age < 40 AND ( birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\"";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);
        log_debug!("norm {:?}", &tokens);
        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]AND[birthdate<GTE>2001-01-01])OR([age<GT>21]AND[detail<EQ>bonjour]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_8() {
        init_logger();
        let input =
            " age < 40 AND (( limit == 5 OR birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\") ";
        log_debug!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        log_debug!("Normalizing...");
        normalize_lexeme(&mut tokens);
        log_debug!("norm {:?}", &tokens);
        log_debug!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected =
            "([age<LT>40]AND(([limit<EQ>5]OR[birthdate<GTE>2001-01-01])OR([age<GT>21]AND[detail<EQ>bonjour])))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn parse_token_test() {
        init_logger();
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),                          // {
            LogicalOpen(PositionalToken::new((), 0)),                          // {{
            ConditionOpen(PositionalToken::new((), 0)),                        // {{(
            Attribute(PositionalToken::new(String::from("attribut1"), 0)),     // {{( attribut1
            Operator(PositionalToken::new(ComparisonOperator::GT, 0)),         // {{( attribut1 GT
            ValueInt(PositionalToken::new(10, 0)),                             // {{( attribut1 GT 10
            ConditionClose(PositionalToken::new((), 0)),                       // {{( attribut1 GT 10 )
            BinaryLogicalOperator(PositionalToken::new(AND, 0)),               // {{( attribut1 GT 10 ) AND
            ConditionOpen(PositionalToken::new((), 0)),                        // {{( attribut1 GT 10 ) AND (
            Attribute(PositionalToken::new(String::from("attribut2"), 0)),     // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(PositionalToken::new(ComparisonOperator::EQ, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(PositionalToken::new(String::from("\nbonjour\n"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose(PositionalToken::new((), 0)),   // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(PositionalToken::new(OR, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(PositionalToken::new(String::from("attribut3"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(PositionalToken::new(ComparisonOperator::LIKE, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(PositionalToken::new(String::from("den%"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);
        let canonical = match parse_tokens_with_index(&tokens, &index) {
            Ok(expression) => to_canonical_form(&expression).unwrap(),
            Err(err) => {
                log_debug!("Error: {:?}", err);
                panic!()
            }
        };
        let tokens = vec![
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("A"), 0)),
            Operator(PositionalToken::new(LIKE, 0)),
            ValueInt(PositionalToken::new(10, 0)),
            ConditionClose(PositionalToken::new((), 0)),
        ];
        const EXPECTED: &str = "(([attribut1<GT>10]AND[attribut2<EQ>\nbonjour\n])OR[attribut3<LIKE>den%])";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_2() {
        init_logger();
        // (A LIKE 10 )
        let tokens = vec![
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("A"), 0)),
            Operator(PositionalToken::new(LIKE, 0)),
            ValueInt(PositionalToken::new(10, 0)),
            ConditionClose(PositionalToken::new((), 0)),
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_tokens_with_index(&tokens, &index) {
            Ok(expression) => to_canonical_form(&expression).unwrap(),
            Err(err) => {
                log_debug!("Error: {:?}", err);
                panic!()
            }
        };

        const EXPECTED: &str = "[A<LIKE>10]";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_22() {
        init_logger();
        // ([A LIKE 10] OR [B LIKE 10])
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("A"), 0)),
            Operator(PositionalToken::new(LIKE, 0)),
            ValueInt(PositionalToken::new(10, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            BinaryLogicalOperator(PositionalToken::new(OR, 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("B"), 0)),
            Operator(PositionalToken::new(LIKE, 0)),
            ValueInt(PositionalToken::new(10, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            LogicalClose(PositionalToken::new((), 0)),
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_tokens_with_index(&tokens, &index) {
            Ok(expression) => to_canonical_form(&expression).unwrap(),
            Err(err) => {
                log_debug!("Error: {:?}", err);
                panic!()
            }
        };
        const EXPECTED: &str = "([A<LIKE>10]OR[B<LIKE>10])";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_3() {
        init_logger();
        // { { (A LIKE 10 ) OR (BB EQ 45) } AND { (K EQ "victory") OR (K LT 12) } }
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),
            LogicalOpen(PositionalToken::new((), 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("A"), 0)),
            Operator(PositionalToken::new(LIKE, 0)),
            ValueInt(PositionalToken::new(10, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            BinaryLogicalOperator(PositionalToken::new(OR, 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("B"), 0)),
            Operator(PositionalToken::new(EQ, 0)),
            ValueInt(PositionalToken::new(45, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            LogicalClose(PositionalToken::new((), 0)),
            BinaryLogicalOperator(PositionalToken::new(AND, 0)),
            LogicalOpen(PositionalToken::new((), 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("K"), 0)),
            Operator(PositionalToken::new(EQ, 0)),
            ValueString(PositionalToken::new("victory".to_owned(), 0)),
            ConditionClose(PositionalToken::new((), 0)),
            BinaryLogicalOperator(PositionalToken::new(OR, 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("K"), 0)),
            Operator(PositionalToken::new(LT, 0)),
            ValueInt(PositionalToken::new(12, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            LogicalClose(PositionalToken::new((), 0)),
            LogicalClose(PositionalToken::new((), 0)),
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_tokens_with_index(&tokens, &index) {
            Ok(expression) => {
                // log_debug!("Result = {:?}", expression);
                to_canonical_form(&expression).unwrap()
            }
            Err(err) => {
                log_debug!("Error: {:?}", err);
                panic!()
            }
        };

        const EXPECTED: &str = "(([A<LIKE>10]OR[B<EQ>45])AND([K<EQ>victory]OR[K<LT>12]))";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_4() {
        init_logger();
        // "(   [AA => 10]
        //          AND
        //      (
        //         ([DD == 6] OR [BB == 5])
        //         OR
        //         [CC == 4]
        //      )
        //  )"
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("AA"), 0)),
            Operator(PositionalToken::new(GTE, 0)),
            ValueInt(PositionalToken::new(10, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            BinaryLogicalOperator(PositionalToken::new(AND, 0)),
            LogicalOpen(PositionalToken::new((), 0)),
            LogicalOpen(PositionalToken::new((), 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("DD"), 0)),
            Operator(PositionalToken::new(EQ, 0)),
            ValueInt(PositionalToken::new(6, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            BinaryLogicalOperator(PositionalToken::new(OR, 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("BB"), 0)),
            Operator(PositionalToken::new(EQ, 0)),
            ValueInt(PositionalToken::new(5, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            LogicalClose(PositionalToken::new((), 0)),
            BinaryLogicalOperator(PositionalToken::new(OR, 0)),
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("CC"), 0)),
            Operator(PositionalToken::new(EQ, 0)),
            ValueInt(PositionalToken::new(4, 0)),
            ConditionClose(PositionalToken::new((), 0)),
            LogicalClose(PositionalToken::new((), 0)),
            LogicalClose(PositionalToken::new((), 0)),
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_tokens_with_index(&tokens, &index) {
            Ok(expression) => {
                // log_debug!("Result = {:?}", expression);
                to_canonical_form(&expression).unwrap()
            }
            Err(err) => {
                log_debug!("Error: {:?}", err);
                panic!()
            }
        };

        const EXPECTED: &str = "([AA<GTE>10]AND(([DD<EQ>6]OR[BB<EQ>5])OR[CC<EQ>4]))";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn global_fail_1() {
        init_logger();
        // (A LIKE )
        let input = "(A LIKE )";
        match lex3(input) {
            Ok(_) => {}
            Err(e) => match e.error_code {
                FilterErrorCode::WrongNumericValue => {
                    assert_eq!(9, e.char_position);
                }
                _ => {
                    panic!("Error: {:?}", e);
                }
            },
        }
    }

    #[test]
    pub fn parse_token_fail_test_2() {
        init_logger();
        // (([ attribut1 GT 10 ]  [ attribut2 EQ "bonjour" ]) OR [ attribut3 LIKE "den%" ])

        let input = r#"((( attribut1 > 10 )  ( attribut2 == "bonjour" )) OR ( attribut3 LIKE "den%" ))"#;
        let mut lexemes = lex3(input).unwrap();

        log_debug!("Lex3 : {}", TokenSlice(&lexemes));

        normalize_lexeme(&mut lexemes);

        log_debug!("Normalized : {}", TokenSlice(&lexemes));

        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),                      // {
            LogicalOpen(PositionalToken::new((), 0)),                      // {{
            ConditionOpen(PositionalToken::new((), 0)),                    // {{(
            Attribute(PositionalToken::new(String::from("attribut1"), 0)), // {{( attribut1
            Operator(PositionalToken::new(ComparisonOperator::GT, 0)),     // {{( attribut1 GT
            ValueInt(PositionalToken::new(10, 0)),                         // {{( attribut1 GT 10
            ConditionClose(PositionalToken::new((), 0)),                   // {{( attribut1 GT 10 )
            // Introduce a mistake here :  BinaryLogicalOperator(PositionalToken::new(AND, 0)), // {{( attribut1 GT 10 ) AND
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND (
            Attribute(PositionalToken::new(String::from("attribut2"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(PositionalToken::new(EQ, 0)),      // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(PositionalToken::new(String::from("\nbonjour\n"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose(PositionalToken::new((), 0)),   // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(PositionalToken::new(OR, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(PositionalToken::new(String::from("attribut3"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(PositionalToken::new(LIKE, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(PositionalToken::new(String::from("\"den%\""), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);

        let r_exp = parse_tokens_with_index(&tokens, &index);
        match r_exp {
            Ok(v) => {
                assert!(false);
            }
            Err(e) => match e.error_code {
                FilterErrorCode::LogicalOperatorExpected => {
                    assert_eq!(7, e.char_position);
                }
                _ => {
                    assert!(false);
                }
            },
        }
    }

    #[test]
    pub fn parse_token_fail_test_3() {
        init_logger();
        // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" ) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)), // {
            // LogicalOpen(PositionalToken::new((), 0)), // {{
            ConditionOpen(PositionalToken::new((), 0)), // {{(
            Attribute(PositionalToken::new(String::from("attribut1"), 0)), // {{( attribut1
            Operator(PositionalToken::new(ComparisonOperator::GT, 0)), // {{( attribut1 GT
            ValueInt(PositionalToken::new(10, 0)),      // {{( attribut1 GT 10
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 )
            BinaryLogicalOperator(PositionalToken::new(AND, 0)), // {{( attribut1 GT 10 ) AND
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND (
            Attribute(PositionalToken::new(String::from("attribut2"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(PositionalToken::new(ComparisonOperator::EQ, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(PositionalToken::new(String::from("\nbonjour\n"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            // LogicalClose(PositionalToken::new((), 0)), // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(PositionalToken::new(OR, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(PositionalToken::new(String::from("attribut3"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(PositionalToken::new(ComparisonOperator::LIKE, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(PositionalToken::new(String::from("\"den%\""), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);
        let r_exp = parse_tokens_with_index(&tokens, &index);
        match r_exp {
            Ok(_) => {
                assert!(false);
            }
            Err(e) => match e.error_code {
                FilterErrorCode::ClosingExpected => {
                    assert_eq!(12, e.char_position);
                }
                _ => {
                    assert!(false);
                }
            },
        }
    }

    #[test]
    pub fn parse_token_fail_test_4() {
        init_logger();
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )} OR ( LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),                          // {
            LogicalOpen(PositionalToken::new((), 0)),                          // {{
            ConditionOpen(PositionalToken::new((), 0)),                        // {{(
            Attribute(PositionalToken::new(String::from("attribut1"), 0)),     // {{( attribut1
            Operator(PositionalToken::new(ComparisonOperator::GT, 0)),         // {{( attribut1 GT
            ValueInt(PositionalToken::new(10, 0)),                             // {{( attribut1 GT 10
            ConditionClose(PositionalToken::new((), 0)),                       // {{( attribut1 GT 10 )
            BinaryLogicalOperator(PositionalToken::new(AND, 0)),               // {{( attribut1 GT 10 ) AND
            ConditionOpen(PositionalToken::new((), 0)),                        // {{( attribut1 GT 10 ) AND (
            Attribute(PositionalToken::new(String::from("attribut2"), 0)),     // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(PositionalToken::new(ComparisonOperator::EQ, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(PositionalToken::new(String::from("\nbonjour\n"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose(PositionalToken::new((), 0)),   // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(PositionalToken::new(OR, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            // Introduce an error: Attribute(PositionalToken::new(String::from("attribut3"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(PositionalToken::new(LIKE, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(PositionalToken::new(String::from("\"den%\""), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];

        let index = RefCell::new(0usize);
        let r_exp = parse_tokens_with_index(&tokens, &index);

        match r_exp {
            Ok(_) => {
                assert!(false);
            }
            Err(e) => match e.error_code {
                FilterErrorCode::AttributeExpected => {
                    assert_eq!(16, e.char_position as usize);
                }
                _ => {
                    assert!(false);
                }
            },
        }
    }

    #[test]
    pub fn to_sql_test() {
        init_logger();
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),                      // {
            LogicalOpen(PositionalToken::new((), 0)),                      // {{
            ConditionOpen(PositionalToken::new((), 0)),                    // {{(
            Attribute(PositionalToken::new(String::from("attribut1"), 0)), // {{( attribut1
            Operator(PositionalToken::new(GT, 0)),                         // {{( attribut1 GT
            ValueInt(PositionalToken::new(10, 0)),                         // {{( attribut1 GT 10
            ConditionClose(PositionalToken::new((), 0)),                   // {{( attribut1 GT 10 )
            BinaryLogicalOperator(PositionalToken::new(AND, 0)),           // {{( attribut1 GT 10 ) AND
            ConditionOpen(PositionalToken::new((), 0)),                    // {{( attribut1 GT 10 ) AND (
            Attribute(PositionalToken::new(String::from("attribut2"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(PositionalToken::new(EQ, 0)),                         // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(PositionalToken::new(String::from("bonjour"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose(PositionalToken::new((), 0)),   // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(PositionalToken::new(OR, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(PositionalToken::new(String::from("attribut3"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(PositionalToken::new(LIKE, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(PositionalToken::new(String::from("den%"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);
        let sql = match parse_tokens_with_index(&tokens, &index) {
            Ok(expression) => to_sql_form(&expression).unwrap(),
            Err(err) => {
                log_debug!("Error: {:?}", err);
                panic!()
            }
        };

        log_debug!("sql form : {}", sql);
        // const EXPECTED : &str = r#"{{("attribut1"<GT>ValueInt(10))AND("attribut2"<EQ>ValueString("\nbonjour\n"))}OR("attribut3"<LIKE>ValueString("\"den%\""))}"#;
        // assert_eq!(EXPECTED, sql);
    }
}
