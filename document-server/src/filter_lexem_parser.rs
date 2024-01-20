use std::str::Chars;
use unicode_segmentation::UnicodeSegmentation;
use crate::filter_lexem_parser::StreamMode::{Free, PendingAttribute, PendingOperator, PendingValue};
use crate::filter_token_parser::ComparisonOperator::{EQ, GT, GTE, LIKE, LT, LTE, NEQ};
use crate::filter_token_parser::Token;

enum StreamMode {
    Free,
    PendingAttribute,
    PendingOperator,
    PendingValue,
}

fn lex(input: &str) -> Vec<Token> {
    let mut stream_mode: StreamMode = StreamMode::Free;
    let mut attribute: String = String::new();
    let mut fop: String = String::new();
    let mut lop: String = String::new();
    let mut value: String = String::new();
    let mut tokens: Vec<Token> = vec![];

    for g in UnicodeSegmentation::graphemes(input, true) {
        let token = match  g.chars().next() {
            Some('{') => Token::LogicalOpen,
            Some('}') => Token::LogicalClose,
            Some('(') => {
                stream_mode = PendingAttribute;
                attribute.clear();
                Token::ConditionOpen
            }
            Some(')') => {
                stream_mode = Free;
                Token::ConditionClose
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
                        "EQ" => Token::Operator(EQ),
                        "NEQ" => Token::Operator(NEQ),
                        "GT" => Token::Operator(GT),
                        "GTE" => Token::Operator(GTE),
                        "LT" => Token::Operator(LT),
                        "LTE" => Token::Operator(LTE),
                        "LIKE" => Token::Operator(LIKE),
                        _ => Token::Ignore, // TODO handle errors
                    }
                }
                PendingValue => {
                    stream_mode = Free;
                    let c_value = value.clone();
                    if value.starts_with("\"") {
                        Token::ValueString(c_value)
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



const LOP_AND : &str = "AND";
const LOP_OR : &str = "OR";
const FOP_EQ: &str = "EQ";


#[cfg(test)]
mod tests {
    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use crate::filter_lexem_parser::lex;
    use crate::filter_token_parser::{parse_expression, to_sql_form};

    #[test]
    pub fn parse_token_test() {
        let input = r#"{{(attribut1 GT 10 ) AND ( attribut2 EQ "你好" )} OR ( attribut3 LIKE "den%" )}"#;

        let tokens = lex(input);

        for token in &tokens {
            println!("{:?}", token);
        }

        let exp= parse_expression(&tokens).unwrap();

        let s = to_sql_form(&exp).unwrap();

        println!("{:?}", s);


    }

}
