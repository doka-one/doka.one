
use unicode_segmentation::UnicodeSegmentation;

use crate::filter_lexem_parser::StreamMode::{Free, PendingAttribute, PendingOperator, PendingValue};
use crate::filter_token_parser::ComparisonOperator::{EQ, GT, GTE, LIKE, LT, LTE, NEQ};
use crate::filter_token_parser::{LogicalOperator, Token};

enum StreamMode {
    Free,
    PendingAttribute,
    PendingOperator,
    PendingValue,
}

pub (crate) fn lex(input: &str) -> Vec<Token> {
    let mut stream_mode: StreamMode = StreamMode::Free;
    let mut attribute: String = String::new();
    let mut fop: String = String::new();
    let mut lop: String = String::new();
    let mut value: String = String::new();
    let mut tokens: Vec<Token> = vec![];

    for g in UnicodeSegmentation::graphemes(input, true) {
        println!("Grapheme: {}", &g);
        let token = match  g.chars().next() {
            Some('{') => vec![Token::LogicalOpen],
            Some('}') => vec![Token::LogicalClose],
            Some('(') => {
                stream_mode = PendingAttribute;
                attribute.clear();

                let mut token_list = vec![];
                if !lop.is_empty() {
                    token_list.push(Token::BinaryLogicalOperator(LogicalOperator::from_str(&lop)));
                    lop.clear();
                }

                token_list.push(Token::ConditionOpen);
                token_list
            }
            Some(')') => match stream_mode {
                PendingValue => {
                    stream_mode = Free;
                    let c_value = value.clone();
                    vec![if value.starts_with("\"") {
                        Token::ValueString(c_value)
                    } else {
                        match c_value.parse() {
                            Ok(parsed) => Token::ValueInt(parsed),
                            Err(_) => {
                                // TODO handle errors
                                panic!("not a number")
                            },
                        }
                    }, Token::ConditionClose]
                }
                _ => {
                    stream_mode = Free;
                    vec![Token::ConditionClose]
                }

            }
            Some(' ') => match stream_mode {
                Free => vec![Token::Ignore],
                PendingAttribute => {
                    match attribute.is_empty() {
                        false => {
                            stream_mode = PendingOperator;
                            fop.clear();
                            vec![Token::Attribute(attribute.clone())]
                        }
                        true => {
                            vec![Token::Ignore]
                        }
                    }
                }
                PendingOperator => {
                    stream_mode = StreamMode::PendingValue;
                    value.clear();
                    vec![match fop.as_str() {
                        "EQ" => Token::Operator(EQ),
                        "NEQ" => Token::Operator(NEQ),
                        "GT" => Token::Operator(GT),
                        "GTE" => Token::Operator(GTE),
                        "LT" => Token::Operator(LT),
                        "LTE" => Token::Operator(LTE),
                        "LIKE" => Token::Operator(LIKE),
                        _ => Token::Ignore, // TODO handle errors
                    }]
                }
                PendingValue => {
                    stream_mode = Free;
                    let c_value = value.clone();
                    vec![if value.starts_with("\"") {
                        Token::ValueString(c_value)
                    } else {
                        match c_value.parse() {
                            Ok(parsed) => Token::ValueInt(parsed),
                            Err(_) => {
                                // TODO handle errors
                                panic!("not a number")
                            },
                        }
                    }]
                }
            },
            Some(c) => {
                match stream_mode {
                    Free => {
                        println!("LOP C : {}", c);
                        lop.push(c);
                        println!("LOP : {}", &lop);
                    }
                    PendingAttribute => attribute.push(c),
                    PendingOperator => fop.push(c),
                    PendingValue => value.push(c),
                }
                vec![Token::Ignore]
            }
            None => vec![Token::Ignore],
        };

        for t in token {
            if t != Token::Ignore {
                println!("token generated {:?}", &t);
                tokens.push(t);
            }
        }
    }

    tokens
}



const LOP_AND : &str = "AND";
const LOP_OR : &str = "OR";
const FOP_EQ: &str = "EQ";


#[cfg(test)]
mod tests {
    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use crate::filter_lexem_parser::lex;
    use crate::filter_token_parser::{ComparisonOperator, LogicalOperator, parse_expression, to_sql_form, Token};
    use crate::filter_token_parser::FilterExpression::Logical;


    #[test]
    pub fn parse_token_test_1() {
        let input = r#"{(attribut1 GT 10 )}"#;
        let tokens = lex(input);

        // for token in &tokens {
        //     println!("{:?}", token);
        // }

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
    pub fn parse_token_test_2() {
        let input = r#"{{(attribut1 GT 10 ) AND ( attribut2 EQ "你好" )} OR ( attribut3 LIKE "den%" )}"#;

        let tokens = lex(input);

        // for token in &tokens {
        //     println!("{:?}", token);
        // }

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
            Token::ValueString("\"你好\"".to_string()),
            Token:: ConditionClose,
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::ConditionOpen,
            Token:: Attribute("attribut3".to_string()),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString("\"den%\"".to_string()),
            Token::ConditionClose,
            Token::LogicalClose
        ];

        assert_eq!(expected, tokens);

    }

}
