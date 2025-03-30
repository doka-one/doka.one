use std::cell::RefCell;

use crate::filter::filter_ast::Token::{LogicalClose, LogicalOpen};
use crate::filter::filter_ast::{LogicalOperator, PositionalToken, Token};
use crate::filter::ComparisonOperator::{EQ, GT, GTE, LIKE, LT, LTE, NEQ};
use commons_error::*;
use log::{debug, error, info};
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

enum ExpressionExpectedLexeme {
    ExpressionOrCondition,
    LogicalOperatorOrNothing,
}

enum ConditionExpectedLexeme {
    Attribute,
    FilterOperator,
    Value,
}

pub(crate) enum LopexpExpectedLexeme {
    LogicalOperator,
    ExpressionOrCondition,
}

#[derive(Debug, PartialEq)]
pub(crate) enum FilterErrorCode {
    EmptyCondition, // "Nothing to read inside a condition"
    EmptyLogicalOperation,
    WrongLogicalOperator,
    UnknownFilterOperator,
    WrongNumericValue,
    UnclosedQuote,
    IncorrectAttributeChar, // "Wrong char in attribute"
    IncompleteExpression,
    InvalidLogicalDepth,
}

#[derive(Debug)]
pub(crate) struct FilterError {
    pub(crate) char_position: usize,
    pub(crate) error_code: FilterErrorCode,
    pub(crate) message: String,
}

const TRUE: &str = "TRUE";
const FALSE: &str = "FALSE";

const LOP_AND: &str = "AND";
const LOP_OR: &str = "OR";

const FOP_EQ: &str = "==";
const FOP_NEQ: &str = "!=";
const FOP_GTE_1: &str = ">=";
const FOP_GTE_2: &str = "=>";
const FOP_LTE_1: &str = "<=";
const FOP_LTE_2: &str = "=<";
const FOP_GT: &str = ">";
const FOP_LT: &str = "<";
const FOP_LIKE: &str = "LIKE";
const LIST_OF_FOP: &[&str] = &[
    FOP_EQ, FOP_NEQ, FOP_GTE_1, FOP_GTE_2, FOP_LTE_1, FOP_LTE_2, FOP_GT, FOP_LT, FOP_LIKE,
];

fn pad_with_tabs(input: &str, num_tabs: u32) -> String {
    let tabs = "\t".repeat(num_tabs as usize);
    format!("{}{}", tabs, input)
}

fn println(input: &str, num_tabs: u32) {
    println!("[{}] {}", num_tabs, pad_with_tabs(&input, num_tabs));
}

/**
REF_TAG : Parsing doka search expressions.md
*/
pub(crate) fn lex3(input: &str) -> Result<Vec<Token>, FilterError> {
    let closed_input = format!("({})", input); // Encapsulate the conditions in a root ()

    let mut input_chars: Vec<char> = vec![];
    for g in UnicodeSegmentation::graphemes(closed_input.as_str(), true) {
        match g.chars().next() {
            Some(c) => {
                input_chars.push(c);
            }
            _ => {}
        }
    }

    let index = RefCell::new(0usize);
    let expression_marker = 0;
    println!(
        "OPEN EXP {}  - a new expression is starting",
        expression_marker
    );
    let tokens = exp_lexer_index(&index, &input_chars, 0)?;
    println!(
        "CLOSE EXP {} Expression Sub token: {:?}",
        expression_marker, &tokens
    );
    Ok(tokens)
}

// ( + "( attribut1 >= 10 AND attribut2 == \"bonjour\") OR (attribut3 LIKE \"den%\" )" + )
// EXP ::= '(' ( EXP | COND ) ( LOP EXP | COND )* ')'
// LOP ::= 'AND' | 'OR'
// COND ::= ATTR FOP VALUE
// VALUE ::= VALTXT | VALNUM | VALBOOL
// ATTR ::= ( lettre | chiffre )*
// FOP ::= '>=' | '>' | '<' | '<=' | '==' | 'LIKE'
// VALTXT ::= '"' ( unicode_char )* '"'
// VALNUM ::= ( chiffre )+ ( '.' ( chiffre )+ )?
// VALBOOL ::= 'TRUE' | 'FALSE'
// lettre ::= 'a'-'z' | 'A'-'Z'
// chiffre ::= '0'-'9'

fn exp_lexer_index(
    index: &RefCell<usize>,
    mut input_chars: &Vec<char>,
    depth: u32,
) -> Result<Vec<Token>, FilterError> {
    let mut tokens: Vec<Token> = vec![];
    let mut expected_lexem = ExpressionExpectedLexeme::ExpressionOrCondition; // or an attribute

    tokens.push(LogicalOpen(PositionalToken::new((), *index.borrow())));
    let mut expression_marker: i32 = -1;
    loop {
        println("EXP Move 1 step", depth);
        *index.borrow_mut() += 1;

        let grapheme_at_index = match read_char_at_index(&index, &input_chars, depth) {
            None => {
                break;
            }
            Some(value) => value,
        };

        match grapheme_at_index {
            '(' => {
                expression_marker = *index.borrow() as i32;
                println(
                    &format!(
                        "OPEN EXP {} Opening parenthesis - a new expression is starting",
                        expression_marker
                    ),
                    depth,
                );
                let sub_tokens = exp_lexer_index(&index, &mut input_chars, depth + 1)?;
                println(
                    &format!(
                        "CLOSE EXP {} Expression Sub token: {:?}",
                        expression_marker, &sub_tokens
                    ),
                    depth,
                );
                let _ = read_char_at_index(&index, &input_chars, depth);
                tokens.extend(sub_tokens);
                expected_lexem = ExpressionExpectedLexeme::LogicalOperatorOrNothing;
            }
            ')' => {
                println(
                    &format!(
                        "EXP Closing parenthesis - end of the expression {}",
                        expression_marker
                    ),
                    depth,
                );
                break; // Out of the routine
            }
            ' ' => {
                println(&format!("Blank space"), depth);
            }
            _c => {
                match expected_lexem {
                    ExpressionExpectedLexeme::ExpressionOrCondition => {
                        // Here we are at a "expression" level, so the chars is the start for a new condition
                        let sub_tokens = condition_lexer_index(&index, &mut input_chars, depth)?;
                        println(
                            &format!("EXP Condition Sub token: {:?}", &sub_tokens),
                            depth,
                        );
                        let out_char = read_char_at_index(&index, &input_chars, depth);
                        tokens.extend(sub_tokens);
                        expected_lexem = ExpressionExpectedLexeme::LogicalOperatorOrNothing;

                        match out_char {
                            None => {
                                // return an error
                                return Err(FilterError {
                                    char_position: *index.borrow(),
                                    error_code: FilterErrorCode::IncompleteExpression,
                                    message: "Looks like your filter is not complete".to_string(),
                                });
                            }
                            Some(c) => {
                                if c == ')' {
                                    *index.borrow_mut() -= 1;
                                }
                            }
                        }
                    }
                    ExpressionExpectedLexeme::LogicalOperatorOrNothing => {
                        // Here we are at a "expression" level, so the chars is the start for a new condition
                        let sub_tokens = lopexp_lexer_index(&index, &mut input_chars, depth)?;
                        println(&format!("EXP lopexp Sub token: {:?}", &sub_tokens), depth);
                        tokens.extend(sub_tokens);
                    }
                }
                // We are in the expression parsing, so it there is no LogicalOperator it means the expression if finished
                expected_lexem = ExpressionExpectedLexeme::LogicalOperatorOrNothing;
                // Optional
            }
        }
    }
    println("EXP out of the loop", depth);
    tokens.push(LogicalClose(PositionalToken::new((), *index.borrow())));

    // Control if we did not exit the loop because of a extra closing parenthesis,
    // ignore the last closing parenthesis in the length comparison
    if depth == 0 && *index.borrow() < (input_chars.len() - 1) {
        return Err(FilterError {
            char_position: *index.borrow(),
            error_code: FilterErrorCode::InvalidLogicalDepth,
            message: "Too many parenthesis".to_string(),
        });
    }

    Ok(tokens)
}

/// Read a condition which is "COND ::= ATTR FOP VALUE"
fn condition_lexer_index(
    index: &RefCell<usize>,
    input_chars: &Vec<char>,
    depth: u32,
) -> Result<Vec<Token>, FilterError> {
    let mut tokens: Vec<Token> = vec![];
    let mut expected_lexeme: ConditionExpectedLexeme = ConditionExpectedLexeme::Attribute;
    let mut attribute: String = String::new();
    let mut value: String = String::new();
    let mut fop: String = String::new();
    let mut text_mode = false;

    println(
        &format!("Condition reading start at {}", *index.borrow()),
        depth,
    );
    loop {
        let grapheme_at_index = match read_char_at_index(&index, &input_chars, depth) {
            None => {
                println(
                    &format!(
                        "COND Condition reading start at {} - Nothing to read inside the condition",
                        *index.borrow()
                    ),
                    depth,
                );
                return Err(FilterError {
                    char_position: *index.borrow(),
                    error_code: FilterErrorCode::EmptyCondition,
                    message: "Nothing to read inside the condition".to_string(),
                });
            }
            Some(value) => value,
        };

        match grapheme_at_index {
            ' ' => {
                match expected_lexeme {
                    ConditionExpectedLexeme::Attribute => {
                        // Add the attribute and change the expected lexeme to FilterOperator
                        append_attribute(
                            &mut attribute,
                            &mut expected_lexeme,
                            &mut tokens,
                            *index.borrow(),
                        )?;
                    }
                    ConditionExpectedLexeme::FilterOperator => {
                        // Add the filter operator and change the expected lexeme to Value
                        append_fop(&mut fop, &mut expected_lexeme, &mut tokens, *index.borrow())?;
                    }
                    ConditionExpectedLexeme::Value => {
                        if text_mode {
                            value.push(grapheme_at_index);
                        } else {
                            // for non text value, it marks the end of the condition
                            if !value.is_empty() {
                                append_value(&mut value, &mut tokens, *index.borrow())?;
                                break; // Here is the end of the condition processing
                            }
                        }
                    }
                }
            }
            ')' => {
                println(
                    &format!("COND End the condition because of closing parenthesis"),
                    depth,
                );

                if text_mode {
                    // Cannot exit condition processing if we are in text mode
                    return Err(FilterError {
                        char_position: *index.borrow() - value.chars().count(),
                        error_code: FilterErrorCode::UnclosedQuote,
                        message: "Missing closing quote".to_string(),
                    });
                }
                append_value(&mut value, &mut tokens, *index.borrow())?;
                *index.borrow_mut() -= 1;
                break;
            }
            c => {
                // Here we are at a "condition" level
                match expected_lexeme {
                    ConditionExpectedLexeme::Attribute => {
                        if is_valid_char_attribute(c) {
                            attribute.push(c);
                        } else {
                            // check if c is the first char of the filter operator, return true if it is
                            let still_valid = LIST_OF_FOP.iter().any(|fop| fop.starts_with(c));

                            if !still_valid {
                                // A wrong char is found in an attribute, return an error
                                return Err(FilterError {
                                    char_position: *index.borrow(),
                                    error_code: FilterErrorCode::IncorrectAttributeChar,
                                    message: "Wrong character in attribute".to_string(),
                                });
                            } else {
                                // Because the char is the first symbol of one of the filter operator, we mark the end of the attribute section and allow to continue
                                append_attribute(
                                    &mut attribute,
                                    &mut expected_lexeme,
                                    &mut tokens,
                                    *index.borrow(),
                                )?;
                                *index.borrow_mut() -= 1;
                            }
                        }
                    }
                    ConditionExpectedLexeme::FilterOperator => {
                        // we must check the char to know if its compatible with any of the Filter Operator
                        if find_possible_operator_with(c, &fop, LIST_OF_FOP) {
                            fop.push(c)
                        } else {
                            append_fop(
                                &mut fop,
                                &mut expected_lexeme,
                                &mut tokens,
                                *index.borrow(),
                            )?;
                            *index.borrow_mut() -= 1;
                        }
                    }
                    ConditionExpectedLexeme::Value => {
                        value.push(c);
                        if c == '"' {
                            text_mode = !text_mode;

                            if !text_mode {
                                println(&format!("COND Read a QUOTE - Exit text mode"), depth);
                                append_value(&mut value, &mut tokens, *index.borrow())?;
                                break; // Here is the end of the condition processing
                            } else {
                                println(&format!("COND Read a QUOTE - Enter text mode"), depth);
                            }
                        }
                    }
                }
            }
        }

        // println(&format!("Condition Move 1 step"), depth);
        *index.borrow_mut() += 1;
    }
    println(
        &format!("loop was out for index {}", *index.borrow()),
        depth,
    );
    Ok(tokens)
}

/// Read a lopexp which is "LOP EXP|COND"
fn lopexp_lexer_index(
    index: &RefCell<usize>,
    mut input_chars: &Vec<char>,
    depth: u32,
) -> Result<Vec<Token>, FilterError> {
    let mut tokens: Vec<Token> = vec![];
    let mut expected_lexeme: LopexpExpectedLexeme = LopexpExpectedLexeme::LogicalOperator;
    let mut lop: String = String::new();

    println(
        &format!("Lopexp reading start at {}", *index.borrow()),
        depth,
    );
    loop {
        let grapheme_at_index = match read_char_at_index(&index, &input_chars, depth) {
            None => {
                return Err(FilterError {
                    char_position: *index.borrow(),
                    error_code: FilterErrorCode::EmptyLogicalOperation,
                    message: "Nothing to read inside the logical operation".to_string(),
                });
            }
            Some(value) => value,
        };

        match grapheme_at_index {
            ' ' => {
                match expected_lexeme {
                    LopexpExpectedLexeme::LogicalOperator => {
                        if !lop.is_empty() {
                            let lexeme = match lop.to_uppercase().as_str() {
                                LOP_AND => Token::BinaryLogicalOperator(PositionalToken::new(
                                    LogicalOperator::AND,
                                    *index.borrow() - LOP_AND.len(),
                                )),
                                LOP_OR => Token::BinaryLogicalOperator(PositionalToken::new(
                                    LogicalOperator::OR,
                                    *index.borrow() - LOP_OR.len(),
                                )),
                                value => {
                                    return Err(FilterError {
                                        char_position: *index.borrow() - value.len() - 1,
                                        error_code: FilterErrorCode::WrongLogicalOperator,
                                        message: "Unknown logical operator".to_string(),
                                    });
                                }
                            };
                            tokens.push(lexeme);
                            expected_lexeme = LopexpExpectedLexeme::ExpressionOrCondition;
                        }
                    }
                    LopexpExpectedLexeme::ExpressionOrCondition => {
                        // Nothing to do !
                    }
                }
            }
            '(' => {
                println(
                    &format!("LOPEXP Opening parenthesis - a new expression is starting"),
                    depth,
                );
                let sub_tokens = exp_lexer_index(&index, &mut input_chars, depth + 1)?;
                println(
                    &format!("LOPEXP Expression Sub token: {:?}", &sub_tokens),
                    depth,
                );
                tokens.extend(sub_tokens);

                let out_char = read_char_at_index(&index, &input_chars, depth);

                match out_char {
                    None => {
                        // return an error
                        return Err(FilterError {
                            char_position: *index.borrow(),
                            error_code: FilterErrorCode::IncompleteExpression,
                            message: "Looks like your filter is not complete".to_string(),
                        });
                    }
                    Some(c) => {
                        if c == ')' {
                            *index.borrow_mut() -= 1;
                        }
                    }
                }
            }
            ')' => {
                println(
                    &format!("LOPEXP Closing parenthesis - end of the expression"),
                    depth,
                );
                break; // Out of the routine
            }
            c => {
                // Here we are at a "condition" level
                match expected_lexeme {
                    LopexpExpectedLexeme::LogicalOperator => {
                        lop.push(c);
                    }
                    LopexpExpectedLexeme::ExpressionOrCondition => {
                        // Here we are at a "lopexp" level, expecting a condition or an expression, so the chars is the start for a new condition
                        println(&format!("LOPEXP new condition is starting"), depth);
                        let sub_tokens = condition_lexer_index(&index, &mut input_chars, depth)?;
                        println(&format!("Condition Sub token: {:?}", &sub_tokens), depth);
                        let out_char = read_char_at_index(&index, &input_chars, depth);
                        tokens.extend(sub_tokens);

                        match out_char {
                            None => {
                                // return an error
                                return Err(FilterError {
                                    char_position: *index.borrow(),
                                    error_code: FilterErrorCode::IncompleteExpression,
                                    message: "Looks like your filter is not complete".to_string(),
                                });
                            }
                            Some(c) => {
                                if c == ')' {
                                    *index.borrow_mut() -= 1;
                                }
                            }
                        }

                        // if out_char.unwrap() == ')' {
                        //     *index.borrow_mut() -= 1;
                        // }

                        let _ = read_char_at_index(&index, &input_chars, depth);
                        break; // After the expression or conditin, the lopexp is finished
                    }
                }
            }
        }
        println(&format!("LOPEXP Move 1 step"), depth);
        *index.borrow_mut() += 1;
    }
    Ok(tokens)
}

fn find_possible_operator_with(c: char, op: &str, operators: &[&str]) -> bool {
    for operator in operators {
        // Is there an operator starting with the new op
        if operator.starts_with(&(op.to_owned() + &c.to_string())) {
            return true;
        }
    }
    false
}

fn is_valid_char_attribute(c: char) -> bool {
    match Regex::new(r"[a-zA-Z0-9_]") {
        Ok(re) => re.is_match(c.to_string().as_str()),
        Err(e) => {
            log_error!("Incorrect regex: [{:?}]", e);
            false
        }
    }
}

fn read_char_at_index(index: &RefCell<usize>, input_chars: &Vec<char>, depth: u32) -> Option<char> {
    match input_chars.get(*index.borrow()) {
        None => {
            println(&format!("No more characters"), depth);
            None
        }
        Some(value) => {
            println(
                &format!("Position [{}] char [{}]", *index.borrow(), value),
                depth,
            );
            Some(*value)
        }
    }
}

fn create_fop(fop: &str, index: usize) -> Result<Token, FilterError> {
    // current position - 1 // to step back from the current position
    // - fop.len()
    // + 1 // to start at 1
    let char_pos = index - fop.len();
    if !fop.is_empty() {
        match fop.as_ref() {
            FOP_EQ => Ok(Token::Operator(PositionalToken::new(EQ, char_pos))),
            FOP_NEQ => Ok(Token::Operator(PositionalToken::new(NEQ, char_pos))),
            FOP_GT => Ok(Token::Operator(PositionalToken::new(GT, char_pos))),
            FOP_GTE_1 | FOP_GTE_2 => Ok(Token::Operator(PositionalToken::new(GTE, char_pos))),
            FOP_LT => Ok(Token::Operator(PositionalToken::new(LT, char_pos))),
            FOP_LTE_1 | FOP_LTE_2 => Ok(Token::Operator(PositionalToken::new(LTE, char_pos))),
            FOP_LIKE => Ok(Token::Operator(PositionalToken::new(LIKE, char_pos))),
            _ => Err(FilterError {
                char_position: char_pos,
                error_code: FilterErrorCode::UnknownFilterOperator,
                message: "Unknown filter operator".to_string(),
            }),
        }
    } else {
        Err(FilterError {
            char_position: char_pos,
            error_code: FilterErrorCode::UnknownFilterOperator,
            message: "Empty filter operator".to_string(),
        })
    }
}

fn append_fop(
    fop: &mut String,
    expected_lexeme: &mut ConditionExpectedLexeme,
    tokens: &mut Vec<Token>,
    index: usize,
) -> Result<(), FilterError> {
    let lexeme = create_fop(fop, index)?;
    tokens.push(lexeme);
    fop.clear();
    *expected_lexeme = ConditionExpectedLexeme::Value;
    Ok(())
}

fn append_attribute(
    attribute: &mut String,
    expected_lexeme: &mut ConditionExpectedLexeme,
    tokens: &mut Vec<Token>,
    index: usize,
) -> Result<(), FilterError> {
    if !attribute.is_empty() {
        tokens.push(Token::Attribute(PositionalToken::new(
            attribute.clone(),
            index - attribute.chars().count(), // -1 + 1
        )));
        attribute.clear();
        *expected_lexeme = ConditionExpectedLexeme::FilterOperator;
    } else {
        return Err(FilterError {
            char_position: index,
            error_code: FilterErrorCode::WrongNumericValue,
            message: "Empty attribute".to_string(),
        });
    }
    Ok(())
}

fn append_value(
    value: &mut String,
    tokens: &mut Vec<Token>,
    index: usize,
) -> Result<(), FilterError> {
    let lexeme = if value.starts_with("\"") {
        let raw_value = value.trim_matches('"').to_string();
        let n = raw_value.chars().count();

        Token::ValueString(PositionalToken::new(raw_value, index - n))
    } else if value == TRUE {
        Token::ValueBool(PositionalToken::new(true, index - TRUE.len()))
    } else if value == FALSE {
        Token::ValueBool(PositionalToken::new(false, index - FALSE.len()))
    } else {
        match value.parse() {
            Ok(parsed) => Token::ValueInt(PositionalToken::new(parsed, index - value.len())),
            Err(_) => {
                return Err(FilterError {
                    char_position: index - value.len(),
                    error_code: FilterErrorCode::WrongNumericValue,
                    message: "The value in the condition is not a valid number".to_string(),
                });
            }
        }
    };
    tokens.push(lexeme);
    value.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use crate::filter::filter_ast::{PositionalToken, Token};
    use crate::filter::filter_lexer::{lex3, FilterError, FilterErrorCode};
    use crate::filter::{ComparisonOperator, LogicalOperator};

    // ok
    #[test]
    pub fn lexer_simple() {
        let pos = vec![1, 11, 13];
        let input = "attribut1 > 10";
        let tokens = lex3(input).unwrap();

        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[0])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[1])),
            Token::ValueInt(PositionalToken::new(10, pos[2])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)),
        ];

        assert_eq!(expected, tokens);
    }

    // ok
    #[test]
    pub fn lexer_simple_2() {
        let pos = vec![1, 2, 12, 14, 16];
        let input = "(attribut1 > 10)";
        let tokens = lex3(input).unwrap();

        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::LogicalClose(PositionalToken::new((), pos[4])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)),
        ];

        assert_eq!(expected, tokens);
    }

    /// The filter operator is glued to the attribute name
    #[test]
    pub fn lexer_simple_3() {
        let pos = vec![1, 2, 11, 13, 15];
        let input = "(attribut1> 10)";
        let tokens = lex3(input).unwrap();

        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::LogicalClose(PositionalToken::new((), pos[4])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_4() {
        let pos = vec![1, 2, 12, 13, 15];
        let input = "(attribut1 >10)";
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::LogicalClose(PositionalToken::new((), pos[4])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_and_extra() {
        let pos = vec![1, 2, 12, 14, 16, 18, 22, 32, 36];
        let input = r#"(attribut1 > 10) AND attribut2 == "bonjour""#;
        //                  12         12141618  22        32  36
        let tokens = lex3(input).unwrap();

        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::LogicalClose(PositionalToken::new((), pos[4])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[5])),
            Token::Attribute(PositionalToken::new("attribut2".to_string(), pos[6])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[7])),
            Token::ValueString(PositionalToken::new("bonjour".to_string(), pos[8])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)),
        ];

        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_and_boolean() {
        let pos = vec![1, 2, 6, 8, 10, 12, 15, 16, 25, 28, 32];
        let input = "(age < 40) OR (question == TRUE)";
        //                12   6 8 1012 1516      25 28  32
        let tokens = lex3(input).unwrap();

        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("age".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, pos[2])),
            Token::ValueInt(PositionalToken::new(40, pos[3])),
            Token::LogicalClose(PositionalToken::new((), pos[4])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[5])),
            Token::LogicalOpen(PositionalToken::new((), pos[6])),
            Token::Attribute(PositionalToken::new("question".to_string(), pos[7])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[8])),
            Token::ValueBool(PositionalToken::new(true, pos[9])),
            Token::LogicalClose(PositionalToken::new((), pos[10])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)),
        ];

        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_triple_grouped_with_boolean() {
        let pos = vec![1, 3, 13, 16, 22, 26, 36, 40, 48, 50, 53, 54, 64, 70, 76];
        let input =
            r#"( attribut1 == FALSE AND attribut2 == "bonjour") OR (attribut3 LIKE "den%" )"#;
        //     1 3         13 16    22  26        36  40      48 50 53 54     64    70     76
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[2])),
            Token::ValueBool(PositionalToken::new(false, pos[3])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[4])),
            Token::Attribute(PositionalToken::new("attribut2".to_string(), pos[5])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[6])),
            Token::ValueString(PositionalToken::new("bonjour".to_string(), pos[7])),
            Token::LogicalClose(PositionalToken::new((), pos[8])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[9])),
            Token::LogicalOpen(PositionalToken::new((), pos[10])),
            Token::Attribute(PositionalToken::new("attribut3".to_string(), pos[11])),
            Token::Operator(PositionalToken::new(ComparisonOperator::LIKE, pos[12])),
            Token::ValueString(PositionalToken::new("den%".to_string(), pos[13])),
            Token::LogicalClose(PositionalToken::new((), pos[14])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_and_extra_packed() {
        let pos = vec![1, 2, 11, 12, 14, 16, 20, 29, 32];
        let input = r#"(attribut1>10) AND attribut2=="bonjour""#;
        //                  12        11121416 20       29 32
        let tokens = lex3(input).unwrap();

        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::LogicalClose(PositionalToken::new((), pos[4])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[5])),
            Token::Attribute(PositionalToken::new("attribut2".to_string(), pos[6])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[7])),
            Token::ValueString(PositionalToken::new("bonjour".to_string(), pos[8])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];

        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_two_level_s() {
        let pos = vec![
            1, 2, 3, 5, 7, 10, 12, 16, 18, 20, 23, 25, 26, 28, 31, 33, 35, 38, 40,
        ];
        //                1 3   7    12    18   23 26   31  35   40
        let input = "((A > 10 ) AND ( B == 5 )) OR ( C == 2 )";
        //                 2  5    10     16 20   25 28   33   38
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::LogicalOpen(PositionalToken::new((), pos[1])),
            Token::Attribute(PositionalToken::new("A".to_string(), pos[2])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[3])),
            Token::ValueInt(PositionalToken::new(10, pos[4])),
            Token::LogicalClose(PositionalToken::new((), pos[5])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[6])),
            Token::LogicalOpen(PositionalToken::new((), pos[7])),
            Token::Attribute(PositionalToken::new("B".to_string(), pos[8])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[9])),
            Token::ValueInt(PositionalToken::new(5, pos[10])),
            Token::LogicalClose(PositionalToken::new((), pos[11])),
            Token::LogicalClose(PositionalToken::new((), pos[12])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[13])),
            Token::LogicalOpen(PositionalToken::new((), pos[14])),
            Token::Attribute(PositionalToken::new("C".to_string(), pos[15])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[16])),
            Token::ValueInt(PositionalToken::new(2, pos[17])),
            Token::LogicalClose(PositionalToken::new((), pos[18])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_two_level() {
        let pos = [
            1, 2, 3, 13, 15, 18, 20, 24, 26, 36, 40, 44, 45, 47, 50, 52, 62, 68, 74,
        ];
        let input =
        //     1 3           15   20    26            40    45   50          62          74
            r#"((attribut1 > 10 ) AND ( attribut2 == "你好" )) OR ( attribut3 LIKE "den%" )"#;
        //      2          13   18    24          36       44 47   52              68
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::LogicalOpen(PositionalToken::new((), pos[1])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[2])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[3])),
            Token::ValueInt(PositionalToken::new(10, pos[4])),
            Token::LogicalClose(PositionalToken::new((), pos[5])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[6])),
            Token::LogicalOpen(PositionalToken::new((), pos[7])),
            Token::Attribute(PositionalToken::new("attribut2".to_string(), pos[8])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[9])),
            Token::ValueString(PositionalToken::new("你好".to_string(), pos[10])),
            Token::LogicalClose(PositionalToken::new((), pos[11])),
            Token::LogicalClose(PositionalToken::new((), pos[12])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[13])),
            Token::LogicalOpen(PositionalToken::new((), pos[14])),
            Token::Attribute(PositionalToken::new("attribut3".to_string(), pos[15])),
            Token::Operator(PositionalToken::new(ComparisonOperator::LIKE, pos[16])),
            Token::ValueString(PositionalToken::new("den%".to_string(), pos[17])),
            Token::LogicalClose(PositionalToken::new((), pos[18])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_two_level_2() {
        let pos = vec![
            1, 2, 12, 16, 18, 20, 24, 25, 27, 41, 45, 47, 50, 52, 62, 68, 74, 75,
        ];

        let input =
        //     1          12    18    24 27                 45    50         62          74
            r#"(attribut1 => 10 ) AND (( attribut2 == "你好" ) OR ( attribut3 LIKE "den%" ))"#;
        //      2            16   20   25              41     47   52              68     75
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[4])),
            Token::LogicalOpen(PositionalToken::new((), pos[5])),
            Token::Attribute(PositionalToken::new("attribut2".to_string(), pos[6])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[7])),
            Token::ValueString(PositionalToken::new("你好".to_string(), pos[8])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[9])),
            Token::Attribute(PositionalToken::new("attribut3".to_string(), pos[10])),
            Token::Operator(PositionalToken::new(ComparisonOperator::LIKE, pos[11])),
            Token::ValueString(PositionalToken::new("den%".to_string(), pos[12])),
            Token::LogicalClose(PositionalToken::new((), pos[13])),
            Token::LogicalClose(PositionalToken::new((), pos[14])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_ne!(expected, tokens);
    }

    #[test]
    pub fn lexer_three_levels() {
        let pos = vec![
            1, 2, 3, 6, 9, 11, 13, 17, 18, 19, 22, 25, 26, 28, 31, 33, 36, 39, 41, 42, 43, 45, 48,
            50, 53, 56, 58,
        ];
        //                1 3      9   13  18  22  26   31   36   41 43   48   53  58
        let input = "((AA => 10) AND ((DD == 6) OR ( BB == 5 ))) OR ( CC == 4 )";
        //                 2   6    11    17 19   25 28   33    39 42 45   50     56
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::LogicalOpen(PositionalToken::new((), pos[1])),
            Token::Attribute(PositionalToken::new("AA".to_string(), pos[2])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, pos[3])),
            Token::ValueInt(PositionalToken::new(10, pos[4])),
            Token::LogicalClose(PositionalToken::new((), pos[5])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[6])),
            Token::LogicalOpen(PositionalToken::new((), pos[7])),
            Token::LogicalOpen(PositionalToken::new((), pos[8])),
            Token::Attribute(PositionalToken::new("DD".to_string(), pos[9])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[10])),
            Token::ValueInt(PositionalToken::new(6, pos[11])),
            Token::LogicalClose(PositionalToken::new((), pos[12])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[13])),
            Token::LogicalOpen(PositionalToken::new((), pos[14])),
            Token::Attribute(PositionalToken::new("BB".to_string(), pos[15])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[16])),
            Token::ValueInt(PositionalToken::new(5, pos[17])),
            Token::LogicalClose(PositionalToken::new((), pos[18])),
            Token::LogicalClose(PositionalToken::new((), pos[19])),
            Token::LogicalClose(PositionalToken::new((), pos[20])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[21])),
            Token::LogicalOpen(PositionalToken::new((), pos[22])),
            Token::Attribute(PositionalToken::new("CC".to_string(), pos[23])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[24])),
            Token::ValueInt(PositionalToken::new(4, pos[25])),
            Token::LogicalClose(PositionalToken::new((), pos[26])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_eq!(expected, tokens);
    }

    /// triple conditions without group
    #[test]
    pub fn lexer_triple_conditions() {
        let pos = vec![1, 4, 6, 9, 13, 16, 19, 22, 25, 28, 31];
        //                1     6     13     19    25    31
        let input = "AA > 10 AND BB == 20 OR CC == 30";
        //                   4    9      16     22   28
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::Attribute(PositionalToken::new("AA".to_string(), pos[0])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[1])),
            Token::ValueInt(PositionalToken::new(10, pos[2])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[3])),
            Token::Attribute(PositionalToken::new("BB".to_string(), pos[4])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[5])),
            Token::ValueInt(PositionalToken::new(20, pos[6])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[7])),
            Token::Attribute(PositionalToken::new("CC".to_string(), pos[8])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[9])),
            Token::ValueInt(PositionalToken::new(30, pos[10])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_eq!(expected, tokens);
    }

    /// triple conditions without group
    #[test]
    pub fn lexer_triple() {
        let pos = vec![
            1, 3, 6, 8, 11, 13, 17, 19, 22, 25, 28, 30, 33, 35, 38, 41, 44,
        ];
        //                 1   6    11     17  22    28    33  38    44
        let input = "( AA > 10 ) AND ( BB == 20 ) OR ( CC == 30 )";
        //                  3     8   13    19    25   30    35   41
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("AA".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::LogicalClose(PositionalToken::new((), pos[4])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[5])),
            Token::LogicalOpen(PositionalToken::new((), pos[6])),
            Token::Attribute(PositionalToken::new("BB".to_string(), pos[7])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[8])),
            Token::ValueInt(PositionalToken::new(20, pos[9])),
            Token::LogicalClose(PositionalToken::new((), pos[10])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[11])),
            Token::LogicalOpen(PositionalToken::new((), pos[12])),
            Token::Attribute(PositionalToken::new("CC".to_string(), pos[13])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[14])),
            Token::ValueInt(PositionalToken::new(30, pos[15])),
            Token::LogicalClose(PositionalToken::new((), pos[16])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_boolean() {
        let pos = vec![1, 9, 12, 17, 21, 31, 34];
        //                1          12       21            34
        let input = "my_bool == TRUE AND your_bool == FALSE";
        //                         9      17            31
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::Attribute(PositionalToken::new("my_bool".to_string(), pos[0])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[1])),
            Token::ValueBool(PositionalToken::new(true, pos[2])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[3])),
            Token::Attribute(PositionalToken::new("your_bool".to_string(), pos[4])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[5])),
            Token::ValueBool(PositionalToken::new(false, pos[6])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_eq!(expected, tokens);
    }

    // ok
    #[test]
    pub fn lexer_triple_grouped() {
        let pos = vec![1, 3, 13, 16, 19, 23, 33, 37, 45, 47, 50, 51, 61, 67, 73];

        //                  1           13     19           33           45  50         61           73
        let input = r#"( attribut1 >= 10 AND attribut2 == "*** 👻 *") OR (attribut3 LIKE "den%" )"#;
        //                    3            16     23            37         47 51              67
        let tokens = lex3(input).unwrap();
        let expected: Vec<Token> = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // fixed pos value
            Token::LogicalOpen(PositionalToken::new((), pos[0])),
            Token::Attribute(PositionalToken::new("attribut1".to_string(), pos[1])),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, pos[2])),
            Token::ValueInt(PositionalToken::new(10, pos[3])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, pos[4])),
            Token::Attribute(PositionalToken::new("attribut2".to_string(), pos[5])),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, pos[6])),
            Token::ValueString(PositionalToken::new("*** 👻 *".to_string(), pos[7])),
            Token::LogicalClose(PositionalToken::new((), pos[8])),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, pos[9])),
            Token::LogicalOpen(PositionalToken::new((), pos[10])),
            Token::Attribute(PositionalToken::new("attribut3".to_string(), pos[11])),
            Token::Operator(PositionalToken::new(ComparisonOperator::LIKE, pos[12])),
            Token::ValueString(PositionalToken::new("den%".to_string(), pos[13])),
            Token::LogicalClose(PositionalToken::new((), pos[14])),
            Token::LogicalClose(PositionalToken::new((), input.chars().count() + 1)), // fixed pos value
        ];
        assert_eq!(expected, tokens);
    }

    // Failure zone

    #[test]
    pub fn lexer_incorrect_numeric_fail() {
        let pos = vec![1, 3, 5, 7, 11, 14, 17];
        let input = "AA > 10AND BB == 20";
        //                1  3 5 7   11 14 17
        match lex3(input) {
            Ok(tokens) => {
                assert!(false);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::WrongNumericValue, e.error_code);
                assert_eq!(6, e.char_position);
            }
        }
    }

    #[test]
    pub fn lexer_incorrect_operator_fail() {
        let input = "AA > 10 ANDBB == 20";
        match lex3(input) {
            Ok(tokens) => {
                assert!(false);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::WrongLogicalOperator, e.error_code);
                assert_eq!(8, e.char_position);
            }
        }
    }

    #[test]
    pub fn lexer_incorrect_text_value_fail() {
        // we forgot the closing quote after the ghost
        let input = r#"name == "papin 👻 AND age >= 20"#;
        match lex3(input) {
            Ok(tokens) => {
                let dummy: Vec<Token> = vec![];
                assert_eq!(dummy, tokens);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::UnclosedQuote, e.error_code);
                assert_eq!(9, e.char_position);
            }
        }
    }

    #[test]
    pub fn lexer_incorrect_text_value_fail_2() {
        // we forgot the opening quote before 'papin'
        let input = r#"name == papin 👻" AND age >= 20"#;
        match lex3(input) {
            Ok(tokens) => {
                let dummy: Vec<Token> = vec![];
                assert_eq!(dummy, tokens);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::WrongNumericValue, e.error_code);
                assert_eq!(9, e.char_position);
            }
        }
    }

    #[test]
    pub fn lexer_incorrect_logical_op_fail() {
        // we forgot the opening quote before 'papin'
        let input = r#"name == "papin 👻" XAND age >= 20"#;
        match lex3(input) {
            Ok(tokens) => {
                let dummy: Vec<Token> = vec![];
                assert_eq!(dummy, tokens);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::WrongLogicalOperator, e.error_code);
                assert_eq!(18, e.char_position);
            }
        }
    }

    #[test]
    pub fn lexer_incorrect_attribute_fail() {
        // use of a invalid charactere in the attribute name
        let input = r#"na👻me == "papin 👻" XAND age >= 20"#;
        match lex3(input) {
            Ok(tokens) => {
                let dummy: Vec<Token> = vec![];
                assert_eq!(dummy, tokens);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::IncorrectAttributeChar, e.error_code);
                assert_eq!(3, e.char_position);
            }
        }
    }

    /// Missing parenthesis
    #[test]
    pub fn lexer_extra_closing_parenthesis_fail() {
        let input = "(A == 1)) AND ((B == 2))";

        match lex3(&input) {
            Ok(lexemes) => {
                let dummy: Vec<Token> = vec![];
                assert_eq!(dummy, lexemes);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::InvalidLogicalDepth, e.error_code);
                assert_eq!(9, e.char_position);
            }
        }
    }

    #[test]
    pub fn lexer_incorrect_parenthesis_fail() {
        let input = r#"(A == 10) AND B == 20)"#;
        match lex3(&input) {
            Ok(lexemes) => {
                let dummy: Vec<Token> = vec![];
                assert_eq!(dummy, lexemes);
            }
            Err(e) => {
                assert_eq!(FilterErrorCode::InvalidLogicalDepth, e.error_code);
                assert_eq!(22, e.char_position);
            }
        }
    }

    #[test]
    pub fn token_is_logical_close() {
        let my_token = Token::LogicalClose(PositionalToken::new((), 42));
        assert_eq!(true, my_token.is_logical_close());

        let my_token = Token::LogicalOpen(PositionalToken::new((), 42));
        assert_eq!(true, !my_token.is_logical_close());
        assert_eq!(true, my_token.is_logical_open());
    }
}
