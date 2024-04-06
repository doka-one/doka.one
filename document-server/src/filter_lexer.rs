use std::cell::RefCell;
use log::info;
use unicode_segmentation::{Graphemes, UnicodeSegmentation};
use crate::filter_lexer::StreamMode::{Free, PendingAttribute, PendingOperator, PendingValue};
use crate::filter_ast::ComparisonOperator::{EQ, GT, GTE, LIKE, LT, LTE, NEQ};
use crate::filter_ast::{LogicalOperator, Token};
use crate::filter_ast::Token::{BinaryLogicalOperator, ConditionClose, Ignore, LogicalClose, LogicalOpen};

enum StreamMode {
    Free,
    PendingAttribute,
    PendingOperator,
    PendingValue,
}

enum ParenthesisMode {
    RightAfterOpening,
    RightAfterClosing,
    Other,
}

const LOP_AND : &str = "AND";
const LOP_OR : &str = "OR";
const FOP_EQ: &str = "EQ";

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

    dbg!(&input_chars);

    // let mut grapheme_iterator = UnicodeSegmentation::graphemes(closed_input.as_str(), true);
    let index = RefCell::new(0usize);
    let tokens = lex_index(&index, &input_chars);
    tokens
}

enum LexerParsingMode {
    Logical,
    Conditional,
}

fn lex_index(index: &RefCell<usize>, mut input_chars: &Vec<char>) -> Vec<Token> {
    let mut tokens: Vec<Token> = vec![];
    let mut lexer_mode: LexerParsingMode = LexerParsingMode::Logical;
    let mut attribute: String = String::new();
    let mut lexem: String = String::new();

    tokens.push(LogicalOpen);

    println!("New start {}", *index.borrow());

    loop {
        *index.borrow_mut() += 1;
        println!("Position {}", *index.borrow());
        let grapheme_at_index = match input_chars.get(*index.borrow()) {
            None => {
                println!("No more characters");
                break;
            }
            Some(value) => {value}
        };

        println!("Graphem: {}", &grapheme_at_index);

        match grapheme_at_index {
            '(' => {
                println!("Opening parenthesis");
                lexer_mode = LexerParsingMode::Logical;
                let sub_tokens = lex_index(&index, &mut input_chars);
                println!("Sub token: {:?}", &sub_tokens);
                tokens.extend(sub_tokens);
            }
            ')' => {
                println!("Closing parenthesis");
                break; // Out of the routine
            }
            ' ' => {
                println!("Blank space");
                //add_attribute(&mut attribute, &mut tokens);
                tokens.push(Ignore);
            }
            c => {
                println!("Char {}", &c);
                // We found some text in the
                // lexer_mode = LexerParsingMode::Conditional;
                lexem.push(*c);
            }
            _ => {
                println!("Other");
            }
        }

    }

    match lexer_mode {
        LexerParsingMode::Logical => {
            tokens.push(LogicalClose)
        }
        LexerParsingMode::Conditional => {
            tokens.push(ConditionClose)
        }
    }

    tokens
}


fn add_attribute(attr: &mut String, tokens: &mut Vec<Token>) {
    if ! attr.is_empty() {
        attr.clear();
        tokens.push(Token::Attribute(attr.clone()));
    } else {
        tokens.push(Token::Ignore)
    }
}

pub (crate) fn lex(input: &str) -> Vec<Token> {
    let mut stream_mode: StreamMode = StreamMode::Free;
    let mut attribute: String = String::new();
    let mut fop: String = String::new();
    let mut lop: String = String::new();
    let mut value: String = String::new();
    let mut tokens: Vec<Token> = vec![];
    let mut parenthesis_mode: ParenthesisMode = ParenthesisMode::Other;

    let closed_input = format!("({})", input); // Encapsulate the conditions in a root ()

    for g in UnicodeSegmentation::graphemes(closed_input.as_str(), true) {
        let token = match  g.chars().next() {
            Some('(') => {
                match parenthesis_mode {
                    ParenthesisMode::RightAfterOpening => {
                        // Corrige le token précedent
                        if let Some(dernier) = tokens.last_mut() {
                            *dernier = Token::LogicalOpen;
                        }
                    }
                    ParenthesisMode::RightAfterClosing => {

                    }
                    ParenthesisMode::Other => {
                    }
                }

                match stream_mode {
                    Free => {
                        match lop.as_str() {
                            LOP_AND => {tokens.push(BinaryLogicalOperator(LogicalOperator::AND))}
                            LOP_OR => {tokens.push(BinaryLogicalOperator(LogicalOperator::OR))}
                            _ => {}
                        }
                        lop.clear();
                    }
                    _ => {}
                }
                parenthesis_mode = ParenthesisMode::RightAfterOpening;
                stream_mode = PendingAttribute;
                attribute.clear();
                Token::ConditionOpen
            }
            Some(')') => {
                match parenthesis_mode {
                    ParenthesisMode::RightAfterOpening => {
                        panic!("Closing parenthesis cannot be here after an opening");
                    }
                    ParenthesisMode::RightAfterClosing => {
                        Token::LogicalClose
                    }
                    ParenthesisMode::Other => {
                        parenthesis_mode = ParenthesisMode::RightAfterClosing;
                        stream_mode = Free;
                        Token::ConditionClose
                    }
                }
            }
            Some(' ') => match stream_mode {
                Free => Token::Ignore,
                PendingAttribute => {
                    match attribute.is_empty() {
                        false => {
                            stream_mode = PendingOperator;
                            fop.clear();
                            Token::Attribute(attribute.clone())
                        }
                        true => {
                            Token::Ignore
                        }
                    }
                }
                PendingOperator => {
                    stream_mode = StreamMode::PendingValue;
                    value.clear();
                    match fop.as_str() {
                        "==" => Token::Operator(EQ),
                        "!=" => Token::Operator(NEQ),
                        ">" => Token::Operator(GT),
                        "=>" | ">=" => Token::Operator(GTE),
                        "<" => Token::Operator(LT),
                        "<=" | "=<" => Token::Operator(LTE),
                        "LIKE" => Token::Operator(LIKE),
                        _ => Token::Ignore, // TODO handle errors
                    }
                }
                PendingValue => {
                    stream_mode = Free;
                    let c_value = value.clone();
                    if value.starts_with("\"") {
                        dbg!(&c_value);
                        Token::ValueString(c_value.trim_matches('"').to_string())
                    } else {
                        match c_value.parse() {
                            Ok(parsed) => Token::ValueInt(parsed),
                            Err(_) => {
                                // TODO handle errors
                                panic!("not a number")
                            },
                        }
                    }

                }
            },
            Some(c) => {
                match stream_mode {
                    Free => {
                        println!("LOP C : {}", c);
                        lop.push(c);
                    }
                    PendingAttribute => attribute.push(c),
                    PendingOperator => fop.push(c),
                    PendingValue => value.push(c),
                }
                parenthesis_mode = ParenthesisMode::Other;
                Token::Ignore
            }
            None => Token::Ignore,
        };

        if token != Token::Ignore {
            tokens.push(token);
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use crate::filter_lexer::{lex, lex3};
    use crate::filter_ast::{ComparisonOperator, LogicalOperator, Token};


    #[test]
    pub fn v3_parse() {
        let input = "(AA => 10) AND (CC == \"üü\")";
        let tokens = lex3(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("AA".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(10),
            Token::ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("DD".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token:: ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("BB".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(5),
            Token::ConditionClose,
            Token::LogicalClose,
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("CC".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(4),
            Token::ConditionClose,
            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
    }


    #[test]
    pub fn parse_token_simple() {
        let input = "(attribut1 > 10 )";
        let tokens = lex(input);

        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::ConditionClose,
            Token::LogicalClose];

        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn parse_token_two_level() {
        let input = "((attribut1 > 10 ) AND ( attribut2 == \"你好\" )) OR ( attribut3 LIKE \"den%\" )";
        let tokens = lex(input);

        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::ConditionOpen,
            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("你好".to_string()),
            Token::ConditionClose,
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("attribut3".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString("den%".to_string()),
            Token::ConditionClose,
            Token::LogicalClose
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn parse_token_two_level_2() {
        let input = "(attribut1 => 10 ) AND (( attribut2 == \"你好\" ) OR ( attribut3 LIKE \"den%\" ))";
        let tokens = lex(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("你好".to_string()),
            Token:: ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("attribut3".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString("den%".to_string()),
            Token::ConditionClose,
            Token::LogicalClose,
            Token::LogicalClose
        ];
        assert_ne!(expected, tokens);
    }

    #[test]
    pub fn parse_token_three_levels() {
        let input = "((AA => 10) AND ((DD == 6) OR ( BB == 5 ))) OR ( CC == 4 )";
        let tokens = lex(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("AA".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(10),
            Token::ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("DD".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token:: ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("BB".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(5),
            Token::ConditionClose,
            Token::LogicalClose,
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("CC".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(4),
            Token::ConditionClose,
            Token::LogicalClose,
        ];
        assert_ne!(expected, tokens);
    }

    /// triple conditions without group - fail because we don't support chained conditions without parenthesis
    #[test]
    pub fn parse_token_triple_fail() {
        let input = "(attribut1 > 10) AND ( attribut2 == \"你好\" ) OR ( attribut3 LIKE \"den%\" )";
        let tokens = lex(input);
        let expected: Vec<Token> = vec![
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("attribut1".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10), // will be missing
            Token::ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::ConditionOpen,
            Token::Attribute("attribut2".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString("你好".to_string()),
            Token:: ConditionClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("attribut3".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString("den%".to_string()),
            Token::ConditionClose,
            Token::LogicalClose
        ];
        assert_ne!(expected, tokens);
    }
}
