use std::cell::RefCell;
use std::fmt;

use log::warn;
use rs_uuid::uuid8;

use crate::filter::filter_lexer::lex3;
use crate::filter::filter_normalizer::normalize_lexeme;
use crate::filter::{ComparisonOperator, FilterCondition, FilterExpressionAST, FilterValue};

#[cfg(test)]
const COND_OPEN: &str = "[";
#[cfg(test)]
const COND_CLOSE: &str = "]";
#[cfg(test)]
const LOGICAL_OPEN: &str = "(";
#[cfg(test)]
const LOGICAL_CLOSE: &str = ")";

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LogicalOperator {
    AND,
    OR,
}

//// Parser structures
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PositionalToken<T> {
    pub token: T,
    pub position: usize,
}

impl<T> PositionalToken<T> {
    pub fn new(token: T, position: usize) -> Self {
        Self { token, position }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Attribute(PositionalToken<String>),
    Operator(PositionalToken<ComparisonOperator>),
    ValueInt(PositionalToken<i32>),
    ValueString(PositionalToken<String>),
    ValueBool(PositionalToken<bool>),
    BinaryLogicalOperator(PositionalToken<LogicalOperator>),
    ConditionOpen(PositionalToken<()>),  // (
    ConditionClose(PositionalToken<()>), // )
    LogicalOpen(PositionalToken<()>),    // {
    LogicalClose(PositionalToken<()>),   // }
}

impl Token {
    /// Test if the token is LogicalOpen
    pub fn is_logical_open(&self) -> bool {
        matches!(self, Token::LogicalOpen(_))
    }

    /// Test if the token is LogicalClose
    pub fn is_logical_close(&self) -> bool {
        matches!(self, Token::LogicalClose(_))
    }

    /// Test if the token is ConditionOpen
    pub fn is_condition_open(&self) -> bool {
        matches!(self, Token::ConditionOpen(_))
    }

    /// Test if the token is ConditionClose
    pub fn is_condition_close(&self) -> bool {
        matches!(self, Token::ConditionClose(_))
    }
}

// for debug only
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Attribute(pt) => write!(f, "{}", pt.token),
            Token::Operator(pt) => write!(
                f,
                "{}",
                match pt.token {
                    ComparisonOperator::EQ => "=",
                    ComparisonOperator::NEQ => "!=",
                    ComparisonOperator::GT => ">",
                    ComparisonOperator::GTE => ">=",
                    ComparisonOperator::LT => "<",
                    ComparisonOperator::LTE => "<=",
                    ComparisonOperator::LIKE => "LIKE",
                }
            ),
            Token::ValueInt(pt) => write!(f, "{}", pt.token),
            Token::ValueString(pt) => write!(f, "\"{}\"", pt.token),
            Token::ValueBool(pt) => write!(f, "{}", pt.token),
            Token::BinaryLogicalOperator(pt) => write!(
                f,
                "{}",
                match pt.token {
                    LogicalOperator::AND => "AND",
                    LogicalOperator::OR => "OR",
                }
            ),
            Token::ConditionOpen(_) => write!(f, "("),
            Token::ConditionClose(_) => write!(f, ")"),
            Token::LogicalOpen(_) => write!(f, "("),
            Token::LogicalClose(_) => write!(f, ")"),
        }
    }
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
pub(crate) fn to_canonical_form(
    filter_expression: &FilterExpressionAST,
) -> Result<String, TokenParseError> {
    let mut content: String = String::from("");
    match filter_expression {
        FilterExpressionAST::Condition(FilterCondition {
            key,
            attribute,
            operator,
            value,
        }) => {
            let s = format!(
                "{}{}<{:?}>{}{}",
                COND_OPEN, attribute, operator, value, COND_CLOSE
            );
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
pub(crate) fn parse_tokens(tokens: &[Token]) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    let index = RefCell::new(0usize);
    parse_tokens_with_index(&tokens, &index)
}

fn parse_tokens_with_index(
    tokens: &[Token],
    index: &RefCell<usize>,
) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    // Read the fist token
    // we start at 0
    let t = tokens.get(*index.borrow());

    if let Some(token) = t {
        match token {
            Token::LogicalOpen(pt) => {
                // The expression starts with a bracket, it's a logical
                println!("found a logical at index {}", *index.borrow());
                let logical_expression = parse_logical(tokens, &index)?;
                println!(
                    "logical expression was [{:?}], now index is [{}]",
                    &logical_expression,
                    *index.borrow()
                );
                Ok(logical_expression)
            }
            Token::ConditionOpen(pt) => {
                println!("found a condition at index {}", *index.borrow());
                let c = parse_condition(&tokens, &index)?;
                println!(
                    "condition expression was [{:?}], now index is [{}]",
                    &c,
                    *index.borrow()
                );
                Ok(c)
            }
            _ => {
                warn!("Wrong opening");
                return Err(TokenParseError::OpeningExpected((
                    *index.borrow(),
                    Some(token.clone()),
                )));
            }
        }
    } else {
        return Err(TokenParseError::OpeningExpected((*index.borrow(), None)));
    }
}

/// At this point we know the tokens starting at <index>
/// are of the form : LO EXPRESSION LOP EXPRESSION LC
fn parse_logical(
    tokens: &[Token],
    index: &RefCell<usize>,
) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    // TODO get rid of all the println!
    println!("parse_logical at [{}]", *index.borrow());

    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow());

    println!("next token is [{:?}]", &t);

    if let Some(token) = t {
        match token {
            Token::ConditionOpen(pt) | Token::LogicalOpen(pt) => {
                // Read the Left member of the Logical Expression
                println!("found a new expression at index {}", *index.borrow());
                let left = parse_tokens_with_index(&tokens, &index)?;
                println!(
                    "logical expression_left was [{:?}], now index is [{}]",
                    &left,
                    *index.borrow()
                );

                // here we must found the LOP
                *index.borrow_mut() += 1;
                let op_fop = tokens.get(*index.borrow());

                let operator = if let Some(t_op) = op_fop {
                    match t_op {
                        Token::BinaryLogicalOperator(op) => op,
                        _ => {
                            warn!("Must be an operator");
                            return Err(TokenParseError::LogicalOperatorExpected((
                                *index.borrow(),
                                Some(t_op.clone()),
                            )));
                        }
                    }
                } else {
                    warn!("Must be an operator");
                    return Err(TokenParseError::LogicalOperatorExpected((
                        *index.borrow(),
                        None,
                    )));
                }
                .clone();

                println!(
                    "Found the logical operator [{:?}], index is [{}]",
                    &operator,
                    *index.borrow()
                );

                // and then the right expression

                *index.borrow_mut() += 1;
                println!(
                    "looking for the right expression at index {}",
                    *index.borrow()
                );
                let right = parse_tokens_with_index(&tokens, &index)?;
                println!(
                    "logical expression_right was [{:?}], now index is [{}]",
                    &left,
                    *index.borrow()
                );

                // then the logical closing
                *index.borrow_mut() += 1;
                let t = tokens.get(*index.borrow());

                println!(
                    "Looking for the logical close at index {}, token=[{:?}]",
                    *index.borrow(),
                    &t
                );

                if let Some(Token::LogicalClose(_)) = t {
                    Ok(Box::new(FilterExpressionAST::Logical {
                        // FIXME : should keep the position
                        operator: operator.token,  //left,
                        leaves: vec![left, right], //right,
                    }))
                } else {
                    warn!("Expected logical closing");
                    Err(TokenParseError::ClosingExpected((
                        *index.borrow(),
                        t.map(|x| x.clone()),
                    )))
                }
            }
            _ => Err(TokenParseError::OpeningExpected((
                *index.borrow(),
                Some(token.clone()),
            ))),
        }
    } else {
        return Err(TokenParseError::OpeningExpected((*index.borrow(), None)));
    }
}

/// At this point we know the tokens starting at <index>
/// are of the form : C_OPEN ATTRIBUTE  FOP  VALUE C_CLOSE
fn parse_condition(
    tokens: &[Token],
    index: &RefCell<usize>,
) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    // Here we know that the form is C_OPEN ATTRIBUTE  FOP  VALUE C_CLOSE

    println!("parse_condition at [{}]", *index.borrow());

    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow());

    println!("next condition token is [{:?}]", &t);

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
                            return Err(TokenParseError::OperatorExpected((
                                *index.borrow(),
                                Some(t_op.clone()),
                            )));
                        }
                    }
                } else {
                    warn!("Must be a comparison operator"); // TODO NORM
                    return Err(TokenParseError::OperatorExpected((*index.borrow(), None)));
                }
                .clone();

                println!(
                    "comparison operator [{:?}] at [{}]",
                    &operator,
                    *index.borrow()
                );

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
                            return Err(TokenParseError::ValueExpected((
                                *index.borrow(),
                                Some(t_value.clone()),
                            )));
                        }
                    }
                } else {
                    warn!("Must be a value"); // TODO NORM
                    return Err(TokenParseError::ValueExpected((
                        *index.borrow(),
                        t.map(|x| x.clone()),
                    )));
                };

                *index.borrow_mut() += 1;
                let op_value = tokens.get(*index.borrow());

                println!(
                    "CLOSE parse_condition at [{}], token=[{:?}]",
                    *index.borrow(),
                    &op_value
                );
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
                return Err(TokenParseError::AttributeExpected((
                    *index.borrow(),
                    Some(token.clone()),
                )));
            }
        }
    } else {
        return Err(TokenParseError::AttributeExpected((
            *index.borrow(),
            t.map(|x| x.clone()),
        )));
    }
}

#[cfg(test)]
mod tests {

    //cargo test --color=always --bin document-server filter_ast::tests   -- --show-output

    use crate::filter::filter_ast::LogicalOperator::{AND, OR};
    use crate::filter::filter_ast::Token::{
        Attribute, BinaryLogicalOperator, ConditionClose, ConditionOpen, LogicalClose, LogicalOpen,
        Operator, ValueInt, ValueString,
    };
    use crate::filter::filter_ast::{
        parse_tokens, parse_tokens_with_index, to_canonical_form, PositionalToken, Token,
        TokenParseError,
    };
    use crate::filter::filter_lexer::lex3;
    use crate::filter::filter_normalizer::normalize_lexeme;
    use crate::filter::ComparisonOperator::{EQ, GT, GTE, LIKE, LT};
    use crate::filter::{analyse_expression, to_sql_form, ComparisonOperator};
    use std::cell::RefCell;

    #[test]
    pub fn global_analyser_1() {
        // let input = " age < 40 AND (( limit == 5 OR birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\") ";
        let input1 = " (country == \"FR\"  AND  (science >= 40) OR (lost_in_hell == \"TRUE\") )";
        let input2 = "((country==\"FR\" AND (science>=40)) OR (lost_in_hell==\"TRUE\") )";
        let input3 = "country == \"FR\"  AND  (science => 40) OR (lost_in_hell == \"TRUE\")";
        let input4 = "country == \"FR\"  AND  science >= 40 OR lost_in_hell == \"TRUE\"";

        println!("Analyse...");
        let tree1 = analyse_expression(input1).unwrap();
        let tree2 = analyse_expression(input2).unwrap();
        let tree3 = analyse_expression(input3).unwrap();
        let tree4 = analyse_expression(input4).unwrap();

        let canonical1 = to_canonical_form(tree1.as_ref()).unwrap();
        let canonical2 = to_canonical_form(tree2.as_ref()).unwrap();
        let canonical3 = to_canonical_form(tree3.as_ref()).unwrap();
        let canonical4 = to_canonical_form(tree4.as_ref()).unwrap();

        println!("canonical...{canonical1}");

        let expected = "(([country<EQ>\"FR\"]AND[science<GTE>40])OR[lost_in_hell<EQ>\"TRUE\"])";
        assert_eq!(expected, &canonical1);
        assert_eq!(expected, &canonical2);
        assert_eq!(expected, &canonical3);
        assert_eq!(expected, &canonical4);
    }

    #[test]
    pub fn global_test_1() {
        let input = "(age < 40) OR (denis < 5 AND age > 21) AND (detail == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR(([denis<LT>5]AND[age<GT>21])AND[detail<EQ>6]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_1_1() {
        let input = "(age < 40) OR (question == TRUE)";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR[question<EQ>TRUE])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_2() {
        let input = "(A < 40) OR (B > 21) AND (C == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([A<LT>40]OR([B<GT>21]AND[C<EQ>6]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_2_2() {
        let input = "(A < 40) AND (B > 21) AND (C == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([A<LT>40]AND[B<GT>21])AND[C<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_3() {
        let input = "((age < 40) OR (age > 21)) AND (detail == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]OR[age<GT>21])AND[detail<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_4() {
        let input = "(A < 40) OR (B > 21) OR (C == 6)";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);

        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([A<LT>40]OR[B<GT>21])OR[C<EQ>6])";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_5() {
        let input = "(age < 40 OR (   age > 21 AND detail == \"bonjour\"  )   )";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]OR([age<GT>21]AND[detail<EQ>\"bonjour\"]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_6() {
        let input =
            "age < 40 OR  birthdate >= \"2001-01-01\"  OR  age > 21 AND detail == \"bonjour\"  ";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]OR[birthdate<GTE>\"2001-01-01\"])OR([age<GT>21]AND[detail<EQ>\"bonjour\"]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_7() {
        let input =
            "age < 40 AND ( birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\"";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "(([age<LT>40]AND[birthdate<GTE>\"2001-01-01\"])OR([age<GT>21]AND[detail<EQ>\"bonjour\"]))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn global_test_8() {
        let input = " age < 40 AND (( limit == 5 OR birthdate >= \"2001-01-01\") OR  age > 21 AND detail == \"bonjour\") ";
        println!("Lexer...");
        let mut tokens = lex3(input).unwrap();

        println!("Normalizing...");
        normalize_lexeme(&mut tokens);
        println!("norm {:?}", &tokens);
        println!("Parsing...");
        let r = parse_tokens(&mut tokens);
        let s = to_canonical_form(r.unwrap().as_ref());
        let expected = "([age<LT>40]AND(([limit<EQ>5]OR[birthdate<GTE>\"2001-01-01\"])OR([age<GT>21]AND[detail<EQ>\"bonjour\"])))";
        assert_eq!(expected, s.unwrap());
    }

    #[test]
    pub fn parse_token_test() {
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),   // {
            LogicalOpen(PositionalToken::new((), 0)),   // {{
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
            LogicalClose(PositionalToken::new((), 0)), // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
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
                println!("Error: {:?}", err);
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
        const EXPECTED: &str =
            "(([attribut1<GT>10]AND[attribut2<EQ>\"\nbonjour\n\"])OR[attribut3<LIKE>\"den%\"])";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_2() {
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
                println!("Error: {:?}", err);
                panic!()
            }
        };

        const EXPECTED: &str = "[A<LIKE>10]";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_22() {
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
                println!("Error: {:?}", err);
                panic!()
            }
        };
        const EXPECTED: &str = "([A<LIKE>10]OR[B<LIKE>10])";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_test_3() {
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
                // println!("Result = {:?}", expression);
                to_canonical_form(&expression).unwrap()
            }
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            }
        };

        const EXPECTED: &str = "(([A<LIKE>10]OR[B<EQ>45])AND([K<EQ>\"victory\"]OR[K<LT>12]))";
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
                // println!("Result = {:?}", expression);
                to_canonical_form(&expression).unwrap()
            }
            Err(err) => {
                println!("Error: {:?}", err);
                panic!()
            }
        };

        const EXPECTED: &str = "([AA<GTE>10]AND(([DD<EQ>6]OR[BB<EQ>5])OR[CC<EQ>4]))";
        assert_eq!(EXPECTED, canonical);
    }

    #[test]
    pub fn parse_token_fail_test_1() {
        // (A LIKE )
        let tokens = vec![
            ConditionOpen(PositionalToken::new((), 0)),
            Attribute(PositionalToken::new(String::from("A"), 0)),
            Operator(PositionalToken::new(LIKE, 0)),
            // Introduce a mistake here:  ValueInt(PositionalToken::new(10, 0)),
            ConditionClose(PositionalToken::new((), 0)),
        ];
        let index = RefCell::new(0usize);

        let r_exp = parse_tokens_with_index(&tokens, &index);
        match r_exp {
            Ok(_) => {
                assert!(false);
            }
            Err(e) => match e {
                TokenParseError::ValueExpected((index, token)) => {
                    assert_eq!(3, index);
                    assert_eq!(true, token.unwrap().is_condition_close());
                }
                _ => {
                    assert!(false);
                }
            },
        }
    }

    #[test]
    pub fn parse_token_fail_test_2() {
        // {{( attribut1 GT 10 )  ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),   // {
            LogicalOpen(PositionalToken::new((), 0)),   // {{
            ConditionOpen(PositionalToken::new((), 0)), // {{(
            Attribute(PositionalToken::new(String::from("attribut1"), 0)), // {{( attribut1
            Operator(PositionalToken::new(ComparisonOperator::GT, 0)), // {{( attribut1 GT
            ValueInt(PositionalToken::new(10, 0)),      // {{( attribut1 GT 10
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 )
            // Introduce a mistake here :  BinaryLogicalOperator(PositionalToken::new(AND, 0)), // {{( attribut1 GT 10 ) AND
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND (
            Attribute(PositionalToken::new(String::from("attribut2"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(PositionalToken::new(EQ, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(PositionalToken::new(String::from("\nbonjour\n"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose(PositionalToken::new((), 0)), // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
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
            Ok(_) => {
                assert!(false);
            }
            Err(e) => match e {
                TokenParseError::LogicalOperatorExpected((index, token)) => {
                    assert_eq!(7, index);
                    assert_eq!(true, token.unwrap().is_condition_open());
                }
                _ => {
                    assert!(false);
                }
            },
        }
    }

    #[test]
    pub fn parse_token_fail_test_3() {
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
            Err(e) => match e {
                TokenParseError::ClosingExpected((index, token)) => {
                    assert_eq!(12, index);
                    assert_eq!(
                        BinaryLogicalOperator(PositionalToken::new(OR, 0)),
                        token.unwrap()
                    );
                }
                _ => {
                    assert!(false);
                }
            },
        }
    }

    #[test]
    pub fn parse_token_fail_test_4() {
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )} OR ( LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),   // {
            LogicalOpen(PositionalToken::new((), 0)),   // {{
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
            LogicalClose(PositionalToken::new((), 0)), // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
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
            Err(e) => match e {
                TokenParseError::AttributeExpected((index, token)) => {
                    assert_eq!(16, index);
                    assert_eq!(Operator(PositionalToken::new(LIKE, 0)), token.unwrap());
                }
                _ => {
                    assert!(false);
                }
            },
        }
    }

    #[test]
    pub fn to_sql_test() {
        // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )) OR ( attribut3 LIKE "den%" )}
        let tokens = vec![
            LogicalOpen(PositionalToken::new((), 0)),   // {
            LogicalOpen(PositionalToken::new((), 0)),   // {{
            ConditionOpen(PositionalToken::new((), 0)), // {{(
            Attribute(PositionalToken::new(String::from("attribut1"), 0)), // {{( attribut1
            Operator(PositionalToken::new(GT, 0)),      // {{( attribut1 GT
            ValueInt(PositionalToken::new(10, 0)),      // {{( attribut1 GT 10
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 )
            BinaryLogicalOperator(PositionalToken::new(AND, 0)), // {{( attribut1 GT 10 ) AND
            ConditionOpen(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND (
            Attribute(PositionalToken::new(String::from("attribut2"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2
            Operator(PositionalToken::new(EQ, 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ
            ValueString(PositionalToken::new(String::from("bonjour"), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour"
            ConditionClose(PositionalToken::new((), 0)), // {{( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )
            LogicalClose(PositionalToken::new((), 0)), // {( attribut1 GT 10 ) AND ( attribut2 EQ "bonjour" )}
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
                println!("Error: {:?}", err);
                panic!()
            }
        };

        println!(">>>> SQL {}", sql);
        // const EXPECTED : &str = r#"{{("attribut1"<GT>ValueInt(10))AND("attribut2"<EQ>ValueString("\nbonjour\n"))}OR("attribut3"<LIKE>ValueString("\"den%\""))}"#;
        // assert_eq!(EXPECTED, sql);
    }
}
