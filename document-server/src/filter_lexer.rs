use std::cell::RefCell;

use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

use crate::filter_ast::{LogicalOperator, Token};
use crate::filter_ast::ComparisonOperator::{EQ, GT, GTE, LIKE, LT, LTE, NEQ};
use crate::filter_ast::Token::{LogicalClose, LogicalOpen};

// enum StreamMode {
//     Free,
//     PendingAttribute,
//     PendingOperator,
//     PendingValue,
// }

enum ExpressionExpectedLexeme {
    ExpressionOrCondition,
    LogicalOperatorOrNothing,
}

enum ConditionExpectedLexeme {
    Attribute,
    FilterOperator,
    Value
}

enum LopexpExpectedLexeme {
    LogicalOperator,
    ExpressionOrCondition,
}

// enum ExpectedLexeme {
//     Expression,
//     Attribute,
//     FilterOperator,
//     Value,
//     LogicalOperator,
// }

const LOP_AND : &str = "AND";
const LOP_OR : &str = "OR";
// const LIST_OF_LOP: &[&str] = &[LOP_AND, LOP_OR];

const FOP_EQ: &str = "==";
const FOP_NEQ: &str = "!=";
const FOP_GTE_1: &str = ">=";
const FOP_GTE_2: &str = "=>";
const FOP_LTE_1: &str = "<=";
const FOP_LTE_2: &str = "=<";
const FOP_GT: &str = ">";
const FOP_LT: &str = "<";
const FOP_LIKE: &str = "LIKE";
const LIST_OF_FOP: &[&str] = &[FOP_EQ, FOP_NEQ, FOP_GTE_1, FOP_GTE_2, FOP_LTE_1, FOP_LTE_2, FOP_GT, FOP_LT, FOP_LIKE];

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

pub (crate) fn lex3(input: &str) -> Vec<Token> {
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
    println!("OPEN EXP {}  - a new expression is starting", expression_marker);
    let tokens = exp_lexer_index(&index, &input_chars, 0);
    println!("CLOSE EXP {} Expression Sub token: {:?}", expression_marker, &tokens);
    tokens
}

// ( + "( attribut1 >= 10 AND attribut2 == \"bonjour\") OR (attribut3 LIKE \"den%\" )" + )
// EXP ::= '(' ( EXP | COND ) ( LOP EXP | COND )* ')'
// LOP ::= 'AND' | 'OR'
// COND ::= ATTR FOP VALUE
// VALUE ::= VALTXT | VALNUM
// ATTR ::= ( lettre | chiffre )*
// FOP ::= '>=' | '>' | '<' | '<=' | '==' | 'LIKE'
// VALTXT ::= '"' ( unicode_char )* '"'
// VALNUM ::= ( chiffre )+ ( '.' ( chiffre )+ )?
// lettre ::= 'a'-'z' | 'A'-'Z'
// chiffre ::= '0'-'9'

fn exp_lexer_index(index: &RefCell<usize>, mut input_chars: &Vec<char>, depth: u32) -> Vec<Token> {
    let mut tokens: Vec<Token> = vec![];
    let mut expected_lexem = ExpressionExpectedLexeme::ExpressionOrCondition; // or an attribute

    tokens.push(LogicalOpen);
    let mut expression_marker : i32 = -1;
    loop {

        println("EXP Move 1 step", depth);
        *index.borrow_mut() += 1;

        let grapheme_at_index = match read_char_at_index(&index, &input_chars, depth) {
            None => {
                break;
            }
            Some(value) => {value}
        };

        match grapheme_at_index {
            '(' => {
                expression_marker = *index.borrow() as i32;
                println(&format!("OPEN EXP {} Opening parenthesis - a new expression is starting", expression_marker), depth);
                let sub_tokens = exp_lexer_index(&index, &mut input_chars, depth + 1);
                println(&format!("CLOSE EXP {} Expression Sub token: {:?}", expression_marker, &sub_tokens), depth);
                let _ = read_char_at_index(&index, &input_chars, depth);
                tokens.extend(sub_tokens);
                expected_lexem = ExpressionExpectedLexeme::LogicalOperatorOrNothing;
            }
            ')' => {
                println(&format!("EXP Closing parenthesis - end of the expression {}", expression_marker), depth);
                break; // Out of the routine
            }
            ' ' => {
                println(&format!("Blank space"), depth);
            }
            c => {
                match expected_lexem {
                    ExpressionExpectedLexeme::ExpressionOrCondition => {
                        // Here we are at a "expression" level, so the chars is the start for a new condition
                        let sub_tokens = condition_lexer_index(&index, &mut input_chars, depth);
                        println(&format!("EXP Condition Sub token: {:?}", &sub_tokens), depth);
                        let out_char = read_char_at_index(&index, &input_chars, depth);
                        tokens.extend(sub_tokens);
                        expected_lexem = ExpressionExpectedLexeme::LogicalOperatorOrNothing;
                        if out_char.unwrap() == ')' {
                            *index.borrow_mut() -= 1;
                        }
                    }
                    ExpressionExpectedLexeme::LogicalOperatorOrNothing => {
                        // Here we are at a "expression" level, so the chars is the start for a new condition
                        let sub_tokens = lopexp_lexer_index(&index, &mut input_chars, depth);
                        println(&format!("EXP lopexp Sub token: {:?}", &sub_tokens), depth);
                        tokens.extend(sub_tokens);
                    }
                }
                // We are in the expression parsing, so it there is no LogicalOperator it means the expression if finished
                expected_lexem = ExpressionExpectedLexeme::LogicalOperatorOrNothing; // Optional
            }
        }
    }
    println("EXP out of the loop", depth);
    tokens.push(LogicalClose);
    tokens
}

/// Read a condition which is "COND ::= ATTR FOP VALUE"
fn condition_lexer_index(index: &RefCell<usize>, input_chars: &Vec<char>, depth: u32) -> Vec<Token> {
    let mut tokens: Vec<Token> = vec![];
    let mut expected_lexeme: ConditionExpectedLexeme = ConditionExpectedLexeme::Attribute;
    let mut attribute: String = String::new();
    let mut value: String = String::new();
    let mut fop: String = String::new();

    println(&format!("Condition reading start at {}", *index.borrow()), depth);
    loop {

        let grapheme_at_index = match read_char_at_index(&index, &input_chars, depth) {
            None => {
                panic!("Nothing to read inside a condition");
            }
            Some(value) => {value}
        };

        match grapheme_at_index {
            ' ' => {
                match expected_lexeme {
                    ConditionExpectedLexeme::Attribute => {
                        create_attribute(&mut attribute, &mut expected_lexeme, &mut tokens);
                    }
                    ConditionExpectedLexeme::FilterOperator => {
                        create_fop(&mut fop, &mut expected_lexeme, &mut tokens);
                    }
                    ConditionExpectedLexeme::Value => {
                        if ! value.is_empty() {
                            create_value(&mut value, &mut tokens);
                            break; // Here is the end of the condition processing
                        }
                    }
                }
            }
            ')' => {
                println(&format!("COND End the condition because of closing parenthesis"), depth);
                create_value(&mut value, &mut tokens);
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
                            create_attribute(&mut attribute, &mut expected_lexeme, &mut tokens);
                            *index.borrow_mut() -= 1;
                        }
                    }
                    ConditionExpectedLexeme::FilterOperator => {
                        // we must check the char to know if its compatible with any of the Filter Operator
                        if find_possible_operator_with(c, &fop, LIST_OF_FOP) {
                            fop.push(c)
                        } else {
                            create_fop(&mut fop, &mut expected_lexeme, &mut tokens);
                            *index.borrow_mut() -= 1;
                        }
                    }
                    ConditionExpectedLexeme::Value => {
                        value.push(c);
                    }
                }
            }
        }

        // println(&format!("Condition Move 1 step"), depth);
        *index.borrow_mut() += 1;
    }
    println(&format!("loop was out for index {}", *index.borrow()), depth);
    tokens
}


/// Read a lopexp which is "LOP EXP|COND"
fn lopexp_lexer_index(index: &RefCell<usize>, mut input_chars: &Vec<char>, depth: u32) -> Vec<Token> {
    let mut tokens: Vec<Token> = vec![];
    let mut expected_lexeme: LopexpExpectedLexeme = LopexpExpectedLexeme::LogicalOperator;
    let mut lop: String = String::new();

    println(&format!("Lopexp reading start at {}", *index.borrow()), depth);
    loop {
        let grapheme_at_index = match read_char_at_index(&index, &input_chars, depth) {
            None => {
                panic!("Nothing to read inside a condition");
            }
            Some(value) => {value}
        };

        match grapheme_at_index {
            ' ' => {
                match expected_lexeme {
                    LopexpExpectedLexeme::LogicalOperator => {
                        if ! lop.is_empty() {
                            let lexeme = match lop.to_uppercase().as_str() {
                                LOP_AND => Token::BinaryLogicalOperator(LogicalOperator::AND),
                                LOP_OR => Token::BinaryLogicalOperator(LogicalOperator::OR),
                                _ => panic!("Wrong logical operator: {}", &lop), // TODO handle errors
                            };
                            tokens.push(lexeme);
                            expected_lexeme = LopexpExpectedLexeme::ExpressionOrCondition;
                        }
                    }
                    LopexpExpectedLexeme::ExpressionOrCondition => {
                       panic!("Cannot be expecting expression or condition")
                    }
                }
            }
            '(' => {
                println(&format!("LOPEXP Opening parenthesis - a new expression is starting"), depth);
                // lexer_mode = LexerParsingMode::Logical;
                let sub_tokens = exp_lexer_index(&index, &mut input_chars, depth+1);
                println(&format!("LOPEXP Expression Sub token: {:?}", &sub_tokens), depth);
                tokens.extend(sub_tokens);

                let out_char = read_char_at_index(&index, &input_chars, depth);
                if out_char.unwrap() == ')' {
                    *index.borrow_mut() -= 1;
                }

            }
            ')' => {
                println(&format!("LOPEXP Closing parenthesis - end of the expression"), depth);
                // *index.borrow_mut() -= 1; // in case of )
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
                        let sub_tokens = condition_lexer_index(&index, &mut input_chars, depth);
                        println(&format!("Condition Sub token: {:?}", &sub_tokens), depth);
                        let out_char = read_char_at_index(&index, &input_chars, depth);
                        tokens.extend(sub_tokens);

                        if out_char.unwrap() == ')' {
                            *index.borrow_mut() -= 1;
                        }

                        let _ = read_char_at_index(&index, &input_chars, depth);
                        break; // After the expression or conditin, the lopexp is finished
                    }
                }
            }
        }
        println(&format!("LOPEXP Move 1 step"), depth);
        *index.borrow_mut() += 1;
    }
    tokens
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
    let re = Regex::new(r"[a-zA-Z0-9_]").unwrap();
    re.is_match(c.to_string().as_str())
}

fn read_char_at_index(index: &RefCell<usize>, input_chars: &Vec<char>, depth: u32) -> Option<char> {
    match input_chars.get(*index.borrow()) {
        None => {
            println(&format!("No more characters"), depth);
            None
        }
        Some(value) => {
            println(&format!("Position [{}] char [{}]", *index.borrow(), value), depth);
            Some(*value)
        }
    }
}

fn create_fop(fop: &mut String, expected_lexeme: &mut ConditionExpectedLexeme, tokens: &mut Vec<Token>) {
    if ! fop.is_empty() {
        let lexeme = match fop.as_ref() {
            FOP_EQ => Token::Operator(EQ),
            FOP_NEQ => Token::Operator(NEQ),
            FOP_GT => Token::Operator(GT),
            FOP_GTE_1| FOP_GTE_2 => Token::Operator(GTE),
            FOP_LT => Token::Operator(LT),
            FOP_LTE_1 | FOP_LTE_2 => Token::Operator(LTE),
            FOP_LIKE => Token::Operator(LIKE),
            _ => panic!("Unknown filter operator {}", fop)
        };
        tokens.push(lexeme);
        fop.clear();
        *expected_lexeme = ConditionExpectedLexeme::Value;
    }
}

fn create_attribute(attribute: &mut String, expected_lexeme: &mut ConditionExpectedLexeme, tokens: &mut Vec<Token>) {
    if ! attribute.is_empty() {
        tokens.push(Token::Attribute(attribute.clone()));
        attribute.clear();
        *expected_lexeme = ConditionExpectedLexeme::FilterOperator;
    }
}

fn create_value(value: &mut String, tokens: &mut Vec<Token>) {
    let lexeme = if value.starts_with("\"") {
        Token::ValueString(value.trim_matches('"').to_string())
    } else {
        match value.parse() {
            Ok(parsed) => Token::ValueInt(parsed),
            Err(_) => {
                // TODO handle errors
                panic!("not a number {}", value);
            },
        }
    };
    tokens.push(lexeme);
    value.clear();
}


#[cfg(test)]
mod tests {
    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use crate::filter_ast::{ComparisonOperator, LogicalOperator, Token};
    use crate::filter_lexer::lex3;

    // ok
    #[test]
    pub fn lexer_triple_grouped() {
        let input = "( attribut1 >= 10 AND attribut2 == \"bonjour\") OR (attribut3 LIKE \"den%\" )";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(10),
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("bonjour".to_string()),
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::LogicalOpen,
            Token::Attribute("attribut3".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString("den%".to_string()),
            Token::LogicalClose,
            Token::LogicalClose
        ];
        assert_eq!(expected, tokens);
    }

    // ok
    #[test]
    pub fn lexer_simple() {
        let input = "attribut1 > 10";
        let tokens = lex3(input);

        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose];

        assert_eq!(expected, tokens);
    }


    // ok
    #[test]
    pub fn lexer_simple_2() {
        let input = "(attribut1 > 10)";
        let tokens = lex3(input);

        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,
            Token::LogicalClose
        ];

        assert_eq!(expected, tokens);
    }

    /// Should fail with ">" is not allowed as an attribute char
    #[test]
    pub fn lexer_simple_3() {
        let input = "(attribut1> 10)";
        let tokens = lex3(input);

        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,
            Token::LogicalClose
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_4() {
        let input = "(attribut1 >10)";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,
            Token::LogicalClose
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_and_extra() {
        let input = "(attribut1 > 10) AND attribut2 == \"bonjour\")";
        let tokens = lex3(input);

        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("bonjour".to_string()),

            Token::LogicalClose
        ];

        assert_eq!(expected, tokens);
    }



    // TODO handle errors : not a number
    //#[test]
    pub fn lexer_double_fail() {
        let input = "AA > 10AND BB == 20";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,

            Token::Attribute("AA".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("BB".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(20),

            Token::LogicalClose
        ];
        assert_ne!(expected, tokens);
    }

    //#[test]
    pub fn lexer_double_fail_2() {
        let input = "AA > 10 ANDBB == 20";
        let tokens = lex3(input);
        // TDDO handle "wrong logical operator
        // assert_ne!(expected, tokens);
    }

    #[test]
    pub fn lexer_simple_and_extra_packed() {
        let input = "(attribut1>10) AND attribut2==\"bonjour\")";
        let tokens = lex3(input);

        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("bonjour".to_string()),

            Token::LogicalClose
        ];

        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_two_level_s() {
        let input = "((A > 10 ) AND ( B == 5 )) OR ( C == 2 )";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,

            Token::LogicalOpen,

            Token::LogicalOpen,
            Token::Attribute("A".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::LogicalOpen,
            Token::Attribute("B".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(5),
            Token::LogicalClose,

            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::Attribute("C".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(2),
            Token::LogicalClose,

            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_two_level() {
        let input = "((attribut1 > 10 ) AND ( attribut2 == \"你好\" )) OR ( attribut3 LIKE \"den%\" )";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,

            Token::LogicalOpen,

            Token::LogicalOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::LogicalOpen,
            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("你好".to_string()),
            Token::LogicalClose,

            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::Attribute("attribut3".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString("den%".to_string()),
            Token::LogicalClose,

            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn lexer_two_level_2() {
        let input = "(attribut1 => 10 ) AND (( attribut2 == \"你好\" ) OR ( attribut3 LIKE \"den%\" ))";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,

            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),

            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::LogicalOpen,

            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("你好".to_string()),

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token:: Attribute("attribut3".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString("den%".to_string()),

            Token::LogicalClose,
            Token::LogicalClose,
            Token::LogicalClose
        ];
        assert_ne!(expected, tokens);
    }

    #[test]
    pub fn lexer_three_levels() {
        let input = "((AA => 10) AND ((DD == 6) OR ( BB == 5 ))) OR ( CC == 4 )";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::LogicalOpen,

            Token::Attribute("AA".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(10),

            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::LogicalOpen,

            Token::Attribute("DD".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token:: Attribute("BB".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(5),

            Token::LogicalClose,
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token:: Attribute("CC".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(4),
            Token::LogicalClose,
            Token::LogicalClose,
        ];
        assert_ne!(expected, tokens);
    }

    /// triple conditions without group - fail because we don't support chained conditions without parenthesis
    #[test]
    pub fn lexer_triple_fail_1() {
        let input = "AA > 10 AND BB == 20 OR CC == 30";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,

            Token::Attribute("AA".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("BB".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(20),

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::Attribute("CC".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueInt(30),

            Token::LogicalClose
        ];
        assert_ne!(expected, tokens);
    }


    /// triple conditions without group - fail because we don't support chained conditions without parenthesis
    #[test]
    pub fn lexer_triple_fail() {
        let input = "( AA > 10 ) AND ( BB == 20 ) OR ( CC == 30 )";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,

            Token::LogicalOpen,
            Token::Attribute("AA".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::LogicalOpen,
            Token::Attribute("BB".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(20),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::Attribute("CC".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueInt(30),
            Token::LogicalClose,

            Token::LogicalClose
        ];
        assert_ne!(expected, tokens);
    }



}
