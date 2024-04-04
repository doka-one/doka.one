use unicode_segmentation::UnicodeSegmentation;
use crate::filter_lexer::StreamMode::{Free, PendingAttribute, PendingOperator, PendingValue};
use crate::filter_ast::ComparisonOperator::{EQ, GT, GTE, LIKE, LT, LTE, NEQ};
use crate::filter_ast::{LogicalOperator, Token};
use crate::filter_ast::Token::BinaryLogicalOperator;

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

    use crate::filter_lexer::{lex};
    use crate::filter_ast::{ComparisonOperator, LogicalOperator, parse_expression, to_canonical_form, to_sql_form, Token};


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
