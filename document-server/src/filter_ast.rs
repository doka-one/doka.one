use std::cell::RefCell;
use std::fmt;

use log::warn;

use crate::filter_ast::FilterValue::{ValueInt, ValueString};

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

#[derive(Debug)]
pub(crate) enum FilterValue {
    ValueInt(i32),
    ValueString(String),
}

impl fmt::Display for FilterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueInt(i) => {
                write!(f, "{}", i)
            }
            ValueString(s) => {
                write!(f, "\"{}\"", s.as_str())
            }
        }

    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LogicalOperator {
    AND,
    OR,
}

impl LogicalOperator {
    pub fn from_str(lop: &str) -> Self {
        match lop {
            "AND" => LogicalOperator::AND,
            "OR" => LogicalOperator::OR,
            _ => panic!()
        }
    }

    pub fn to_string(&self) -> String {
        match  self {
            LogicalOperator::AND => {"AND".to_string()}
            LogicalOperator::OR => {"OR".to_string()}
        }
    }
}

#[derive(Debug)]
pub(crate) enum FilterExpressionAST {
    Condition {
        attribute: String,
        operator: ComparisonOperator,
        value: FilterValue,
    },
    Logical {
        operator: LogicalOperator,
        leaves: Vec<Box<FilterExpressionAST>>,
    },
}

//// Parser structures

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Attribute(String),
    Operator(ComparisonOperator),
    ValueInt(i32),
    ValueString(String),
    BinaryLogicalOperator(LogicalOperator),
    ConditionOpen, // (
    ConditionClose, // )
    LogicalOpen, // {
    LogicalClose, // }
    // Ignore,
}

#[derive(Debug)]
pub(crate) enum TokenParseError {
    ValueExpected((usize, Option<Token>)),
    LogicalOperatorExpected((usize, Option<Token>)),
    OperatorExpected((usize, Option<Token>)),
    AttributeExpected((usize, Option<Token>)),
    OpeningExpected((usize, Option<Token>)),
    ClosingExpected((usize, Option<Token>)),
}

/**
REF_TAG : Parsing doka search expressions.md
 */
#[cfg(test)]
pub (crate) fn to_canonical_form(filter_expression : &FilterExpressionAST) -> Result<String, TokenParseError> {
    let mut content : String = String::from("");
    match filter_expression {
        FilterExpressionAST::Condition { attribute, operator, value } => {
            let s = format!("{}{}<{:?}>{}{}", COND_OPEN, attribute, operator, value, COND_CLOSE);
            content.push_str(&s);
        }
        FilterExpressionAST::Logical {  operator, leaves } => {
            content.push_str(LOGICAL_OPEN);

            for (i,l) in leaves.iter().enumerate() {
                let r_leaf_content = to_canonical_form(l);
                if let Ok(leaf) = r_leaf_content {
                    content.push_str(&leaf);
                }
                if i < leaves.len() - 1  {
                    content.push_str(&format!("{:?}", &operator));
                }
            }
            content.push_str(LOGICAL_CLOSE);
        }
    }
    Ok(content)
}

pub (crate) fn to_sql_form(filter_expression : &FilterExpressionAST) -> Result<String, TokenParseError> {
    let mut content : String = String::from("");
    match filter_expression {
        FilterExpressionAST::Condition { attribute, operator, value } => {
            let sql_op = match operator {
                ComparisonOperator::EQ => {"="}
                ComparisonOperator::NEQ => {"<>"}
                ComparisonOperator::GT => {">"}
                ComparisonOperator::LT => {"<"}
                ComparisonOperator::GTE => {">="}
                ComparisonOperator::LTE => {"<="}
                ComparisonOperator::LIKE => {"LIKE"}
            };

            let s = format!("({} {} {})", attribute, sql_op, value);
            content.push_str(&s);
        }
        FilterExpressionAST::Logical { operator, leaves } => {
            content.push_str("(");

            for (i,l) in leaves.iter().enumerate() {
                let r_leaf_content = to_sql_form(l);
                if let Ok(leaf) = r_leaf_content {
                    content.push_str(&leaf);
                }
                if i < leaves.len() - 1  {
                    content.push_str(&format!(" {:?} ", &operator));
                }
            }
            content.push_str(")");
        }
    }
    Ok(content)
}

/// Parse a list of tokens to create the FilterExpression (AST)
pub(crate) fn parse_expression(tokens: &[Token]) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    let index = RefCell::new(0usize);
    parse_expression_with_index(&tokens, &index)
}

fn parse_expression_with_index(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    // Read the fist token
    // we start at 0
    let t = tokens.get(*index.borrow());

    if let Some(token) = t {
        match token {
            Token::LogicalOpen => {
                // The expression starts with a bracket, it's a logical
                println!("found a logical at index {}", *index.borrow());
                let logical_expression = parse_logical(tokens, &index)?;
                println!("logical expression was [{:?}], now index is [{}]", &logical_expression, *index.borrow());
                Ok(logical_expression)
            }
            Token::ConditionOpen => {
                println!("found a condition at index {}", *index.borrow());
                let c = parse_condition(&tokens, &index)?;
                println!("condition expression was [{:?}], now index is [{}]", &c, *index.borrow());
                Ok(c)
            }
            _ => {
                warn!("Wrong opening");
                return Err(TokenParseError::OpeningExpected((*index.borrow(), Some(token.clone()))));
            }

        }
    } else {
        return Err(TokenParseError::OpeningExpected((*index.borrow(), None)));
    }
}

fn parse_logical(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    // here we know the expression is of form  L_OPEN  EXPRESSION LOP EXPRESSION L_CLOSE

    println!("parse_logical at [{}]", *index.borrow());

    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow());

    println!("next token is [{:?}]", &t);

    if let Some(token) = t {
        match token {
            Token::ConditionOpen |  Token::LogicalOpen => {
                // Read the Left member of the Logical Expression
                println!("found a new expression at index {}", *index.borrow());
                let left = parse_expression_with_index(&tokens, &index)?;
                println!("logical expression_left was [{:?}], now index is [{}]", &left, *index.borrow());

                // here we must found the LOP
                *index.borrow_mut() += 1;
                let op_fop = tokens.get(*index.borrow() );

                let operator = if let Some(t_op) = op_fop {
                    match t_op {
                        Token::BinaryLogicalOperator(op) => {
                            op
                        }
                        _ => {
                            warn!("Must be an operator");
                            return Err(TokenParseError::LogicalOperatorExpected((*index.borrow(), Some(t_op.clone()) )));
                        },
                    }
                } else {
                    warn!("Must be an operator");
                    return Err(TokenParseError::LogicalOperatorExpected((*index.borrow(), None )));
                }.clone();

                // and then the right expression

                *index.borrow_mut() += 1;
                println!("expect the right expression at index {}", *index.borrow());
                let right = parse_expression_with_index(&tokens, &index)?;
                println!("logical expression_right was [{:?}], now index is [{}]", &left, *index.borrow());

                // then the logical closing
                *index.borrow_mut() += 1;
                let t = tokens.get(*index.borrow() );

                println!("Expect the logical close at index {}, token=[{:?}]", *index.borrow(), &t);

                if let Some(Token::LogicalClose) = t {
                    Ok(Box::new(FilterExpressionAST::Logical {
                        //left,
                        operator,
                        //right,
                        leaves: vec![left, right],
                    }))
                } else {
                    warn!("Expected logical closing");
                    return Err(TokenParseError::ClosingExpected((*index.borrow(), t.map(|x| x.clone()) )));
                }

            }
            _ =>  return Err(TokenParseError::OpeningExpected((*index.borrow(), Some(token.clone()) ))),
        }
    } else {
        return Err(TokenParseError::OpeningExpected((*index.borrow(), None )));
    }
}

fn parse_condition(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    // Here we know that the form is C_OPEN ATTRIBUTE  FOP  VALUE C_CLOSE

    println!("parse_condition at [{}]", *index.borrow());

    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow() );

    println!("next condition token is [{:?}]", &t);

    if let Some(token) = t {
        match token {
            Token::Attribute(attr) => {
                let attribute = attr.clone();
                *index.borrow_mut() += 1;
                let op_fop = tokens.get(*index.borrow() );

                let operator = if let Some(t_op) = op_fop {
                    match t_op {
                        Token::Operator(op) => {
                            op
                        }
                        _ => {
                            warn!("Must be an comparison operator"); // TODO NORM
                            return Err(TokenParseError::OperatorExpected((*index.borrow(), Some(t_op.clone()) )));
                        },
                    }
                } else {
                    warn!("Must be a comparison operator");  // TODO NORM
                    return Err(TokenParseError::OperatorExpected((*index.borrow(), None )));

                }.clone();

                *index.borrow_mut() += 1;
                let op_value = tokens.get(*index.borrow() );

                let value = if let Some(t_value) = op_value {
                    match t_value {
                        Token::ValueInt(op) => {
                            FilterValue::ValueInt(*op)
                        }
                        Token::ValueString(op) => {
                            FilterValue::ValueString(op.clone())
                        }
                        _ => {
                            warn!("Must be a value"); // TODO NORM
                            return Err(TokenParseError::ValueExpected((*index.borrow(), Some(t_value.clone()) )));
                        },
                    }
                } else {
                    warn!("Must be a value");  // TODO NORM
                    return Err(TokenParseError::ValueExpected((*index.borrow(), t.map(|x| x.clone()) )));
                };

                *index.borrow_mut() += 1;
                let op_value = tokens.get(*index.borrow() );

                println!("CLOSE parse_condition at [{}], token=[{:?}]", *index.borrow(), &op_value);

                Ok(Box::new(FilterExpressionAST::Condition {
                    attribute,
                    operator,
                    value,
                }))
            }
            _ => return Err(TokenParseError::AttributeExpected((*index.borrow(), Some(token.clone()) ))),
        }
    } else {
        return Err(TokenParseError::AttributeExpected((*index.borrow(), t.map(|x| x.clone()) )));
    }
}

#[cfg(test)]
mod tests {

    //cargo test --color=always --bin document-server filter_ast::tests   -- --show-output

    use std::cell::RefCell;

    use crate::filter_ast::{ComparisonOperator, parse_expression, parse_expression_with_index, to_canonical_form, to_sql_form, TokenParseError};
    use crate::filter_ast::ComparisonOperator::LIKE;
    use crate::filter_ast::LogicalOperator::{AND, OR};
    use crate::filter_ast::Token::{Attribute, BinaryLogicalOperator, ConditionClose, ConditionOpen, LogicalClose, LogicalOpen, Operator, ValueInt, ValueString};
    use crate::filter_lexer::lex3;
    use crate::filter_normalizer::normalize_lexeme;

    #[test]
    pub fn global_test_1() {
        let input = "(age < 40) OR (denis < 5 AND age > 21) AND (detail == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR(([denis<LT>5]AND[age<GT>21])AND[detail<EQ>6]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_2() {
        let input = "(A < 40) OR (B > 21) AND (C == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([A<LT>40]OR([B<GT>21]AND[C<EQ>6]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_3() {
        let input = "((age < 40) OR (age > 21)) AND (detail == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]OR[age<GT>21])AND[detail<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_4() {
        let input = "(A < 40) OR (B > 21) OR (C == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([A<LT>40]OR[B<GT>21])OR[C<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_5() {
        let input = "(age < 40 OR (   age > 21 AND detail == \"bonjour\"  )   )";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR([age<GT>21]AND[detail<EQ>\"bonjour\"]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_6() {
        let input = "age < 40 OR  birthdate >= \"2001-01-01\"  OR  age > 21 AND detail == \"bonjour\"  ";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]OR[birthdate<GTE>\"2001-01-01\"])OR([age<GT>21]AND[detail<EQ>\"bonjour\"]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_7() {
        let input = "age < 40 AND ( birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\"";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]AND[birthdate<GTE>\"2001-01-01\"])OR([age<GT>21]AND[detail<EQ>\"bonjour\"]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_8() {
        let input = " age < 40 AND (( limit == 5 OR birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\") ";
        println!("Lexer...");
        let mut tokens = lex3(input);

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_expression(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]AND(([limit<EQ>5]OR[birthdate<GTE>\"2001-01-01\"])OR([age<GT>21]AND[detail<EQ>\"bonjour\"])))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn parse_token_test() {
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen, // {
            LogicalOpen, // {{
            ConditionOpen, // {{(
            Attribute(String::from("attribut1")), // {{( attribut1
            Operator(ComparisonOperator::GT), // {{( attribut1 GT
            ValueInt(10), // {{( attribut1 GT 10
            ConditionClose, // {{( attribut1 GT 10 )
            BinaryLogicalOperator(AND), // {{( attribut1 GT 10 ) AND
            ConditionOpen, // {{( attribut1 GT 10 ) AND (
            Attribute(String::from("attribut2")), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(ComparisonOperator::EQ), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(String::from("\nbonjour\n")),  // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose, // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(OR), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(String::from("attribut3")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(ComparisonOperator::LIKE), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(String::from("den%")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);
        let canonical = match parse_expression_with_index(&tokens, &index) {
            Ok(expression) => {
                to_canonical_form(&expression).unwrap()
            },
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            },
        };

        const EXPECTED : &str = "(([attribut1<GT>10]AND[attribut2<EQ>\"\nbonjour\n\"])OR[attribut3<LIKE>\"den%\"])";
        assert_eq!(EXPECTED, canonical);
    }


    #[test]
    pub fn parse_token_test_2() {
        // (A LIKE 10 )
        let tokens = vec![
            ConditionOpen,
            Attribute(String::from("A")),
            Operator(ComparisonOperator::LIKE),
            ValueInt(10),
            ConditionClose,
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_expression_with_index(&tokens, &index) {
            Ok(expression) => {
                to_canonical_form(&expression).unwrap()
            },
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            },
        };

        const EXPECTED : &str = "[A<LIKE>10]";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_22() {
        // ([A LIKE 10] OR [B LIKE 10])
        let tokens = vec![
            LogicalOpen,
            ConditionOpen,
            Attribute(String::from("A")),
            Operator(ComparisonOperator::LIKE),
            ValueInt(10),
            ConditionClose,
            BinaryLogicalOperator(OR),
            ConditionOpen,
            Attribute(String::from("B")),
            Operator(ComparisonOperator::LIKE),
            ValueInt(10),
            ConditionClose,
            LogicalClose
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_expression_with_index(&tokens, &index) {
            Ok(expression) => {
                to_canonical_form(&expression).unwrap()
            },
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            },
        };
        const EXPECTED : &str = "([A<LIKE>10]OR[B<LIKE>10])";
        assert_eq!(EXPECTED, canonical);
    }


    #[test]
    pub fn parse_token_test_3() {
        // { { (A LIKE 10 ) OR (BB EQ 45) } AND { (K EQ "victory") OR (K LT 12) } }
        let tokens = vec![
            LogicalOpen,
            LogicalOpen,
            ConditionOpen,
            Attribute(String::from("A")),
            Operator(ComparisonOperator::LIKE),
            ValueInt(10),
            ConditionClose,
            BinaryLogicalOperator(OR),
            ConditionOpen,
            Attribute(String::from("B")),
            Operator(ComparisonOperator::EQ),
            ValueInt(45),
            ConditionClose,
            LogicalClose,
            BinaryLogicalOperator(AND),
            LogicalOpen,
            ConditionOpen,
            Attribute(String::from("K")),
            Operator(ComparisonOperator::EQ),
            ValueString("victory".to_owned()),
            ConditionClose,
            BinaryLogicalOperator(OR),
            ConditionOpen,
            Attribute(String::from("K")),
            Operator(ComparisonOperator::LT),
            ValueInt(12),
            ConditionClose,
            LogicalClose,
            LogicalClose
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_expression_with_index(&tokens, &index) {
            Ok(expression) => {
                // println!("Result = {:?}", expression);
                to_canonical_form(&expression).unwrap()
            },
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            },
        };

        const EXPECTED : &str = "(([A<LIKE>10]OR[B<EQ>45])AND([K<EQ>\"victory\"]OR[K<LT>12]))";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_4() {
        // "(   [AA => 10]
        //          AND
        //      (
        //         ([DD == 6] OR [BB == 5])
        //         OR
        //         [CC == 4]
        //      )
        //  )"
        let tokens = vec![
            LogicalOpen,
            ConditionOpen,
            Attribute(String::from("AA")),
            Operator(ComparisonOperator::GTE),
            ValueInt(10),
            ConditionClose,
            BinaryLogicalOperator(AND),
            LogicalOpen,
            LogicalOpen,
            ConditionOpen,
            Attribute(String::from("DD")),
            Operator(ComparisonOperator::EQ),
            ValueInt(6),
            ConditionClose,
            BinaryLogicalOperator(OR),
            ConditionOpen,
            Attribute(String::from("BB")),
            Operator(ComparisonOperator::EQ),
            ValueInt(5),
            ConditionClose,
            LogicalClose,
            BinaryLogicalOperator(OR),
            ConditionOpen,
            Attribute(String::from("CC")),
            Operator(ComparisonOperator::EQ),
            ValueInt(4),
            ConditionClose,
            LogicalClose,
            LogicalClose
        ];
        let index = RefCell::new(0usize);

        let canonical = match parse_expression_with_index(&tokens, &index) {
            Ok(expression) => {
                // println!("Result = {:?}", expression);
                to_canonical_form(&expression).unwrap()
            },
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            },
        };

        const EXPECTED : &str = "([AA<GTE>10]AND(([DD<EQ>6]OR[BB<EQ>5])OR[CC<EQ>4]))";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_fail_test_1() {
        // (A LIKE )
        let tokens = vec![
            ConditionOpen,
            Attribute(String::from("A")),
            Operator(ComparisonOperator::LIKE),
            // Introduce a mistake here:  ValueInt(10),
            ConditionClose,
        ];
        let index = RefCell::new(0usize);

        let r_exp = parse_expression_with_index(&tokens, &index);
        match r_exp {
            Ok(_) => {
                assert!(false);
            }
            Err(e) => {
                match e {
                    TokenParseError::ValueExpected((index, token)) => {
                        assert_eq!(3, index);
                        assert_eq!(ConditionClose, token.unwrap());
                    }
                    _ => {
                        assert!(false);
                    }
                }
            }
        }
    }


    #[test]
    pub fn parse_token_fail_test_2() {
        // {{( attribut1 GT 10 )  ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen, // {
            LogicalOpen, // {{
            ConditionOpen, // {{(
            Attribute(String::from("attribut1")), // {{( attribut1
            Operator(ComparisonOperator::GT), // {{( attribut1 GT
            ValueInt(10), // {{( attribut1 GT 10
            ConditionClose, // {{( attribut1 GT 10 )
            // Introduce a mistake here :  BinaryLogicalOperator(AND), // {{( attribut1 GT 10 ) AND
            ConditionOpen, // {{( attribut1 GT 10 ) AND (
            Attribute(String::from("attribut2")), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(ComparisonOperator::EQ), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(String::from("\nbonjour\n")),  // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose, // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(OR), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(String::from("attribut3")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(ComparisonOperator::LIKE), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(String::from("\"den%\"")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);

        let r_exp = parse_expression_with_index(&tokens, &index);
        match r_exp {
            Ok(_) => {
                assert!(false);
            }
            Err(e) => {
                match e {
                    TokenParseError::LogicalOperatorExpected((index, token)) => {
                        assert_eq!(7, index);
                        assert_eq!(ConditionOpen, token.unwrap());
                    }
                    _ => {
                        assert!(false);
                    }
                }
            }
        }
    }

    #[test]
    pub fn parse_token_fail_test_3() {
        // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" ) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen, // {
            // LogicalOpen, // {{
            ConditionOpen, // {{(
            Attribute(String::from("attribut1")), // {{( attribut1
            Operator(ComparisonOperator::GT), // {{( attribut1 GT
            ValueInt(10), // {{( attribut1 GT 10
            ConditionClose, // {{( attribut1 GT 10 )
            BinaryLogicalOperator(AND), // {{( attribut1 GT 10 ) AND
            ConditionOpen, // {{( attribut1 GT 10 ) AND (
            Attribute(String::from("attribut2")), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(ComparisonOperator::EQ), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(String::from("\nbonjour\n")),  // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            // LogicalClose, // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(OR), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(String::from("attribut3")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(ComparisonOperator::LIKE), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(String::from("\"den%\"")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);
        let r_exp = parse_expression_with_index(&tokens, &index);
        match r_exp {
            Ok(_) => {
                assert!(false);
            }
            Err(e) => {
                match e {
                    TokenParseError::ClosingExpected((index, token)) => {
                        assert_eq!(12, index);
                        assert_eq!(BinaryLogicalOperator(OR), token.unwrap());
                    }
                    _ => {
                        assert!(false);
                    }
                }
            }
        }
    }

    #[test]
    pub fn parse_token_fail_test_4() {
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )} OR ( LIKE "den%" )}
        let tokens = vec![
            LogicalOpen, // {
            LogicalOpen, // {{
            ConditionOpen, // {{(
            Attribute(String::from("attribut1")), // {{( attribut1
            Operator(ComparisonOperator::GT), // {{( attribut1 GT
            ValueInt(10), // {{( attribut1 GT 10
            ConditionClose, // {{( attribut1 GT 10 )
            BinaryLogicalOperator(AND), // {{( attribut1 GT 10 ) AND
            ConditionOpen, // {{( attribut1 GT 10 ) AND (
            Attribute(String::from("attribut2")), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(ComparisonOperator::EQ), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(String::from("\nbonjour\n")),  // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose, // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(OR), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            // Introduce an error: Attribute(String::from("attribut3")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(ComparisonOperator::LIKE), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(String::from("\"den%\"")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];

        let index = RefCell::new(0usize);
        let r_exp = parse_expression_with_index(&tokens, &index);

        match r_exp {
            Ok(_) => {
                assert!(false);
            }
            Err(e) => {
                match e {
                    TokenParseError::AttributeExpected((index, token)) => {
                        assert_eq!(16, index);
                        assert_eq!(Operator(LIKE), token.unwrap());
                    }
                    _ => {
                        assert!(false);
                    }
                }
            }
        }
    }

    #[test]
    pub fn to_sql_test() {
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen, // {
            LogicalOpen, // {{
            ConditionOpen, // {{(
            Attribute(String::from("attribut1")), // {{( attribut1
            Operator(ComparisonOperator::GT), // {{( attribut1 GT
            ValueInt(10), // {{( attribut1 GT 10
            ConditionClose, // {{( attribut1 GT 10 )
            BinaryLogicalOperator(AND), // {{( attribut1 GT 10 ) AND
            ConditionOpen, // {{( attribut1 GT 10 ) AND (
            Attribute(String::from("attribut2")), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(ComparisonOperator::EQ), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(String::from("bonjour")),  // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose, // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
            BinaryLogicalOperator(OR), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR
            ConditionOpen, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR (
            Attribute(String::from("attribut3")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3
            Operator(ComparisonOperator::LIKE), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE
            ValueString(String::from("den%")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
            ConditionClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )
            LogicalClose, // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        ];
        let index = RefCell::new(0usize);
        let sql = match parse_expression_with_index(&tokens, &index) {
            Ok(expression) => {
                to_sql_form(&expression).unwrap()
            },
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            },
        };

        println!(">>>> SQL {}", sql);
        // const EXPECTED : &str = r#"{{("attribut1"<GT>ValueInt(10))AND("attribut2"<EQ>ValueString("\nbonjour\n"))}OR("attribut3"<LIKE>ValueString("\"den%\""))}"#;
        // assert_eq!(EXPECTED, sql);
    }

}
