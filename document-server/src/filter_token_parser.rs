use std::cell::RefCell;
use std::ops::Add;

use log::warn;

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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LogicalOperator {
    AND,
    OR,
}

#[derive(Debug)]
pub(crate) enum FilterExpression {
    Condition {
        attribute: String,
        operator: ComparisonOperator,
        value: FilterValue,
    },
    Logical {
        left: Box<FilterExpression>,
        operator: LogicalOperator,
        right: Box<FilterExpression>,
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
    Ignore,
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

fn to_canonical_form(filter_expression : &FilterExpression) -> Result<String, TokenParseError> {
    let mut content : String = String::from("");
    match filter_expression {
        FilterExpression::Condition { attribute, operator, value } => {
            let s = format!("({:?}<{:?}>{:?})", attribute, operator, value);
            content.push_str(&s);
        }
        FilterExpression::Logical {  left, operator, right } => {
            content.push_str("{");
            let r_left_content = to_canonical_form(left);
            if let Ok(left) = r_left_content {
                content.push_str(&left);
            }
            content.push_str(&format!("{:?}", &operator));
            let r_right_content = to_canonical_form(right);
            if let Ok(right) = r_right_content {
                content.push_str(&right);
            }
            content.push_str("}");
        }
    }
    Ok(content)
}

pub (crate) fn to_sql_form(filter_expression : &FilterExpression) -> Result<String, TokenParseError> {
    let mut content : String = String::from("");
    match filter_expression {
        FilterExpression::Condition { attribute, operator, value } => {
            let sql_op = match operator {
                ComparisonOperator::EQ => {"="}
                ComparisonOperator::NEQ => {"<>"}
                ComparisonOperator::GT => {">"}
                ComparisonOperator::LT => {"<"}
                ComparisonOperator::GTE => {">="}
                ComparisonOperator::LTE => {"<="}
                ComparisonOperator::LIKE => {"LIKE"}
            };

            let s = format!("(\"{:?}\" {:?} {:?})", attribute, sql_op, value);
            content.push_str(&s);
        }
        FilterExpression::Logical {  left, operator, right } => {
            content.push_str("(");
            let r_left_content = to_sql_form(left);
            if let Ok(left) = r_left_content {
                content.push_str(&left);
            }
            content.push_str(&format!("{:?}", &operator));
            let r_right_content = to_sql_form(right);
            if let Ok(right) = r_right_content {
                content.push_str(&right);
            }
            content.push_str(")");
        }
    }
    Ok(content)
}

pub(crate) fn parse_expression(tokens: &[Token]) -> Result<Box<FilterExpression>, TokenParseError> {
    let index = RefCell::new(0usize);
    parse_expression_with_index(&tokens, &index)
}

fn parse_expression_with_index(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpression>, TokenParseError> {
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

fn parse_logical(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpression>, TokenParseError> {
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
                    Ok(Box::new(FilterExpression::Logical {
                        left,
                        operator,
                        right,
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

fn parse_condition(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpression>, TokenParseError> {
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

                Ok(Box::new(FilterExpression::Condition {
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

/*

0 1 2   3         4  5  6 7   8 9         10 11        12    13    14    15    16        17   18     19    20
{ { (   attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )     }     OR    (     attribut3 LIKE "den%" )     }


EXPRESSION ::=  CONDITION | LOGICAL_EXP
CONDITION ::= C_OPEN ATTRIBUTE  FOP  VALUE C_CLOSE
LOGICAL_EXP ::=  L_OPEN  EXPRESSION LOP EXPRESSION L_CLOSE

C_OPEN ::= "("
C_CLOSE ::= ")"
L_OPEN ::= "{"
L_CLOSE ::= "}"
LOP ::= AND | OR
FOP ::= GT | EQ | LT | LIKE

parse expression
    // A expression is either a logical (meaning there is logical op in the middle) or a condition (three terms with a filter op)
    Open
        // We open a parenthésis it means it's a logical
        token next // -1 => 0
        parse logical
    LogicalOp
        incorrect
    Close
        incorrect // the logical parsing take care of the closing parenthesis
    Attribute
        parse condition
    CompOp
        incorrect
    Value
        incorrect

 parse logical
     left = parse expression
     lecture de l'operateur de comparaison  -> op
     right = parse expression
     build FilterExpression::Comparison
     token next // closing parenth

 */


#[cfg(test)]
mod tests {

    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use std::cell::RefCell;

    use crate::filter_token_parser::{ComparisonOperator, LogicalOperator, parse_expression_with_index, to_canonical_form, Token, TokenParseError};
    use crate::filter_token_parser::ComparisonOperator::LIKE;
    use crate::filter_token_parser::LogicalOperator::{AND, OR};
    use crate::filter_token_parser::Token::{Attribute, BinaryLogicalOperator, ConditionClose, ConditionOpen, LogicalClose, LogicalOpen, Operator, ValueInt, ValueString};

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
            ValueString(String::from("\"den%\"")), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIkE "den%"
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

        const EXPECTED : &str = r#"{{("attribut1"<GT>ValueInt(10))AND("attribut2"<EQ>ValueString("\nbonjour\n"))}OR("attribut3"<LIKE>ValueString("\"den%\""))}"#;
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

        const EXPECTED : &str = r#"("A"<LIKE>ValueInt(10))"#;
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

        const EXPECTED : &str = r#"{{("A"<LIKE>ValueInt(10))OR("B"<EQ>ValueInt(45))}AND{("K"<EQ>ValueString("victory"))OR("K"<LT>ValueInt(12))}}"#;
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

}
