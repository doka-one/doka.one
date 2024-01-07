use std::cell::RefCell;

#[derive(Debug, Clone)]
pub(crate) enum ComparisonOperator {
    EQ,
    GT,
    LIKE,
    // Ajoutez d'autres opérateurs au besoin
}

#[derive(Debug)]
pub(crate) enum FilterCondition {
    Attribute(String),
    ValueInt(i32),
    ValueString(String),
}

#[derive(Debug)]
pub(crate) enum LogicalOperator {
    AND,
    OR,
    // Ajoutez d'autres opérateurs logiques au besoin
}

#[derive(Debug)]
pub(crate) enum FilterExpression {
    Comparison {
        attribute: String,
        operator: ComparisonOperator,
        value: FilterCondition,
    },
    Logical {
        left: Box<FilterExpression>,
        operator: LogicalOperator,
        right: Box<FilterExpression>,
    },
}

//// Parser structures

#[derive(Debug)]
enum Token {
    Attribute(String),
    Operator(ComparisonOperator),
    ValueInt(i32),
    ValueString(String),
    LogicalOperator(LogicalOperator),
    ParenthesisOpen,
    ParenthesisClose,
}

#[derive(Debug)]
enum ParseError {
    UnexpectedToken,
}

fn parse_expression(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpression>, ParseError> {

    dbg!("parse_expression", &index);


    let mut expression = parse_logical(&tokens, &index )?;
    dbg!("parse_expression", &index, &expression);

    let mut t = tokens.get(*index.borrow());
    dbg!("*** parse_expression", &t);

    while let Some(token) = t {
        match token {
            Token::LogicalOperator(LogicalOperator::AND) | Token::LogicalOperator(LogicalOperator::OR) => {
                 // Consume the logical operator
                *index.borrow_mut() += 1;
                dbg!("parse_expression après OP", &index);
                let right = parse_logical(tokens, &index)?;
                dbg!("parse_expression", &index, &right);
                expression = Box::new(FilterExpression::Logical {
                    left: expression,
                    operator: match token {
                        Token::LogicalOperator(LogicalOperator::AND) => LogicalOperator::AND,
                        Token::LogicalOperator(LogicalOperator::OR) => LogicalOperator::OR,
                        _ => unreachable!(),
                    },
                    right,
                });
            }
            _ => {
               // *index.borrow_mut() += 1;
                break;
                 },
        }
        t = tokens.get(*index.borrow());
    }

    Ok(expression)
}

fn parse_logical(tokens: &[Token], index: &RefCell<usize>) -> Result<Box<FilterExpression>, ParseError> {
    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow() );

    dbg!("parse_logical", &index, &t);

    if let Some(token) = t {
        match token {
            Token::ParenthesisOpen => {
                let expression = parse_expression(&tokens, &index)?;

                *index.borrow_mut() += 1;
                let t = tokens.get(*index.borrow() );
                if let Some(Token::ParenthesisClose) = t {
                    Ok(expression)
                } else {
                    Err(ParseError::UnexpectedToken)
                }
            }
            Token::Attribute(attribute) => {
                *index.borrow_mut() += 1;
                let t = tokens.get(*index.borrow() );
                let op = match t {
                    Some(Token::Operator(op)) => op.clone(),
                    _ => return Err(ParseError::UnexpectedToken),
                };

                Ok(Box::new(FilterExpression::Comparison {
                    attribute: attribute.clone(),
                    operator: op,
                    value: parse_condition(&tokens, index)?,
                }))
            }
            _ => Err(ParseError::UnexpectedToken),
        }
    } else {
        Err(ParseError::UnexpectedToken)
    }
}

fn parse_condition(tokens: &[Token], index: &RefCell<usize>) -> Result<FilterCondition, ParseError> {
    *index.borrow_mut() += 1;
    let t = tokens.get(*index.borrow() );
    if let Some(token) = t {
        match token {
            Token::ValueInt(value) => Ok(FilterCondition::ValueInt(*value)),
            Token::ValueString(value) => Ok(FilterCondition::ValueString(value.clone())),
            _ => Err(ParseError::UnexpectedToken),
        }
    } else {
        Err(ParseError::UnexpectedToken)
    }
}

/*

parse expression
    Open
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
    use std::cell::RefCell;
    use crate::expression_filter_parser::{ComparisonOperator, LogicalOperator, parse_expression, Token};

    #[test]
    pub fn parse_token_test() {
        let tokens = vec![
            Token::ParenthesisOpen,
            Token::ParenthesisOpen,
            Token::Attribute(String::from("attribut1")),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(10),
            Token::ParenthesisClose,
            Token::LogicalOperator(LogicalOperator::AND),
            Token::ParenthesisOpen,
            Token::Attribute(String::from("attribut2")),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueString(String::from("bonjour")),
            Token::ParenthesisClose,
            Token::ParenthesisClose,
            Token::LogicalOperator(LogicalOperator::OR),
            Token::ParenthesisOpen,
            Token::Attribute(String::from("attribut3")),
            Token::Operator(ComparisonOperator::LIKE),
            Token::ValueString(String::from("den%")),
            Token::ParenthesisClose,
        ];
        let index = RefCell::new(0usize);

        match parse_expression(&tokens, &index) {
            Ok(expression) => println!("Result = {:?}", expression),
            Err(err) => println!("Error: {:?}", err),
        }
    }
}
