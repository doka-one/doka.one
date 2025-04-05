use crate::filter::filter_ast::{LogicalOperator, PositionalToken, Token, TokenSlice};
use crate::filter::filter_lexer::{FilterError, FilterErrorCode};
use commons_error::*;
use log::*;

///
///
///
pub fn normalize_lexeme(tokens: &mut Vec<Token>) {
    log_info!("😇 Normalize lexeme : {}", &TokenSlice(&tokens));
    n1_remove_successive_logical_open_close(tokens);
    log_debug!("After Norm 1 : {}", &TokenSlice(&tokens));
    n2_mark_condition_open_close(tokens);
    log_debug!("After Norm 2 :  {}", &TokenSlice(&tokens));
    n3_binary_logical_operator(tokens);
    log_info!("😎 Final normalisation :  {}", &TokenSlice(&tokens));
}

/// Normalization N3
/// - Ensure all logical operator is strictly binary
/// - If not, place logical delimiter around it, with priority to AND over OR
///
/// This step of normalization suppose that the N2 is fulfilled
fn n3_binary_logical_operator(tokens: &mut Vec<Token>) {
    log_info!("Normalize level 3");
    n3_binary_logical_operator_for_op(tokens, LogicalOperator::AND);
    n3_binary_logical_operator_for_op(tokens, LogicalOperator::OR);
}

fn n3_binary_logical_operator_for_op(tokens: &mut Vec<Token>, for_lop: LogicalOperator) {
    loop {
        let inserting = find_next_ajustable(tokens, &for_lop);
        match inserting {
            None => {
                break;
            }
            Some((x, y)) => {
                let y_positional = 0;
                let x_positional = 0;
                tokens.insert(
                    y as usize,
                    Token::LogicalClose(PositionalToken::new((), y_positional)),
                );
                tokens.insert(
                    x as usize,
                    Token::LogicalOpen(PositionalToken::new((), x_positional)),
                );
            }
        }
    }
}

fn find_next_ajustable(tokens: &Vec<Token>, for_lop: &LogicalOperator) -> Option<(u32, u32)> {
    let mut position_counter: usize = 0;
    let mut inserting: Option<(u32, u32)> = None; //(None, None);
    for token in tokens.iter() {
        match token {
            Token::BinaryLogicalOperator(lop) => match lop.token {
                LogicalOperator::AND => {
                    if *for_lop == LogicalOperator::AND {
                        inserting = check_binary_logical_operator(&tokens, position_counter as u32);
                        log_debug!("Found a position for AND : {:?}", inserting);
                        if inserting.is_some() {
                            break;
                        }
                    }
                }
                LogicalOperator::OR => {
                    if *for_lop == LogicalOperator::OR {
                        inserting = check_binary_logical_operator(&tokens, position_counter as u32);
                        if inserting.is_some() {
                            break;
                        }
                    }
                }
            },
            _ => {}
        }
        position_counter += 1;
    }
    inserting
}

#[derive(PartialEq)]
enum Direction {
    Forward,
    Backward,
}

fn check_binary_logical_operator(tokens: &Vec<Token>, position_counter: u32) -> Option<(u32, u32)> {
    // Check forward
    let left: CheckLogicalBoundary =
        check_logical_one_direction(tokens, position_counter, Direction::Backward);
    // Check backward
    let right: CheckLogicalBoundary =
        check_logical_one_direction(tokens, position_counter, Direction::Forward);
    match (left.boundary_type, right.boundary_type) {
        (BoundaryType::WithLogical, BoundaryType::WithLogical) => {
            // In case we found logical operators surrounding the AND/OR
            None
        }
        (_, _) => {
            // In case either direction has no logical operator
            Some((left.position, right.position))
        }
    }
}

#[derive(Debug)]
enum BoundaryType {
    WithLogical,
    WithoutLogical,
}

/// In the scope of the N3 norm, we try to find the LO/LC around the operators
/// we find the boundaries for each directions, for example (7, WithoutLogical) and (18, WithLogical)
#[derive(Debug)]
struct CheckLogicalBoundary {
    position: u32,
    boundary_type: BoundaryType,
}

/// We start at the <position_counter> in the tokens list (position of the logical operator)
/// and navigate in a direction
/// to see if we find a LogicalOpen or LogicalClose that surrounds the Logical operator
fn check_logical_one_direction(
    tokens: &Vec<Token>,
    position_counter: u32,
    direction: Direction,
) -> CheckLogicalBoundary {
    let mut depth: i32 = 0;
    let mut index: i32 = position_counter as i32;
    let step: i32 = if direction == Direction::Backward {
        -1
    } else {
        1
    };
    let mut position: u32 = index as u32;
    let mut boundary_type = BoundaryType::WithoutLogical;

    loop {
        index += step;

        if index < 0 {
            // Means we haven't found any LO/LC before the end of the list of tokens
            if direction == Direction::Backward {
                // Correct the position in the array to be able to insert the ( at the right place
                position = (index + 1) as u32;
            } else {
                position = index as u32;
            }
            break; // end of the expression
        }

        let t = tokens.get(index as usize);

        match t {
            None => {
                position = index as u32;
                break; // end of the expression
            }
            Some(tt) => {
                match tt {
                    Token::LogicalOpen(pt) => {
                        // If we are backward, an opening is a decrease of the depth (+step)
                        depth += step;
                        if direction == Direction::Backward {
                            let local_logical_close =
                                Token::LogicalClose(PositionalToken::new((), 0));
                            let next_t = tokens
                                .get((index - 1) as usize)
                                .unwrap_or(&local_logical_close);

                            // (count == 0  and lexeme is not LC/LO)
                            if depth == 0 && !(next_t.is_logical_open()) {
                                // Insert the ( _before_ the [
                                position = index as u32;
                                break;
                            }

                            // found an extra "(" that means we are ok in this direction
                            if depth == -1 {
                                boundary_type = BoundaryType::WithLogical;
                                break;
                            }
                        }
                    }
                    Token::LogicalClose(pt) => {
                        // If we are forward, a closing is an increase of the depth (-step)
                        depth += -1 * step;

                        // The depth is back to 0 so we look at the next token
                        // to check if we need a LC at this position
                        if depth == 0 && direction == Direction::Forward {
                            let local_logical_open =
                                Token::LogicalOpen(PositionalToken::new((), 0));
                            let next_t = tokens
                                .get((index + 1) as usize)
                                .unwrap_or(&local_logical_open);

                            if !(next_t.is_logical_close()) {
                                // Insert the ) _after_ the ]
                                position = (index + 1) as u32;
                                break;
                            }
                        }

                        if direction == Direction::Forward {
                            // Means we met an extra LC, so we return the position + WithLogical
                            if depth == -1 {
                                boundary_type = BoundaryType::WithLogical;
                                break;
                            }
                        }
                    }
                    Token::ConditionOpen(pt) => {
                        // The depth is back to 0 so we look at the next lexeme
                        if depth == 0 && direction == Direction::Backward {
                            let local_logical_close =
                                Token::LogicalClose(PositionalToken::new((), 0));
                            let next_t = tokens
                                .get((index - 1) as usize)
                                .unwrap_or(&local_logical_close);

                            // Insert the ( _before_ the [
                            position = index as u32;

                            if next_t.is_logical_open() {
                                boundary_type = BoundaryType::WithLogical;
                            }
                            break;
                        }
                    }
                    Token::ConditionClose(pt) => {
                        // (count == 0  and lexeme is not LC/LO)
                        if depth == 0 && direction == Direction::Forward {
                            let local_logical_open =
                                Token::LogicalOpen(PositionalToken::new((), 0));
                            let next_t = tokens
                                .get((index + 1) as usize)
                                .unwrap_or(&local_logical_open);

                            // Insert the ) _after_ the ]
                            position = (index + 1) as u32;

                            if next_t.is_logical_close() {
                                boundary_type = BoundaryType::WithLogical;
                            }
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    CheckLogicalBoundary {
        position,
        boundary_type,
    }
}

/// Normalization N2
/// - Remove the useless LO/LC around the conditions <br/>
/// - Surround the conditions expression with ConditionOpen and ConditionClose
///
/// Ex :
/// (A == 12) will be [A == 12]  <br/>
/// ( A== 12 AND (B == 5) ) will be ( [A == 12] AND [B == 5] )
fn n2_mark_condition_open_close(tokens: &mut Vec<Token>) {
    log_info!("Normalize level 2");
    let mut position_counter: u32 = 0;
    let mut list_of_replacement: Vec<(Option<u32>, Option<u32>)> = vec![];
    let mut list_of_inserting: Vec<(Option<u32>, Option<u32>)> = vec![]; // The place where to insert the CO/CC

    tokens.insert(0, Token::LogicalOpen(PositionalToken::new((), 0)));
    tokens.push(Token::LogicalClose(PositionalToken::new((), 0)));

    for token in tokens.iter() {
        match token {
            Token::Attribute(_pt) => {
                let mut replacement: (Option<u32>, Option<u32>) = (None, None);
                let mut inserting: (Option<u32>, Option<u32>) = (None, None);

                let pre_position = position_counter - 1;
                let post_position = position_counter + 3;

                let is_logical_opening =
                    check_logical_open_delimiter(&tokens, pre_position as usize);

                let is_logical_closing =
                    check_logical_close_delimiter(&tokens, post_position as usize);

                match (is_logical_opening, is_logical_closing) {
                    (true, true) => {
                        // found delimiters before and after the condition expression, so we replace them
                        replacement.0 = Some(pre_position);
                        replacement.1 = Some(post_position);
                        list_of_replacement.push(replacement);
                    }
                    (_, _) => {
                        inserting.0 = Some(pre_position + 1); // to match the behavior of the "vec::insert", we increment +1
                        inserting.1 = Some(post_position);
                        list_of_inserting.push(inserting);
                    }
                }
            }
            _ => {}
        }
        position_counter += 1;
    }

    // Replace the LO/LC with the CO/CC. This will not change the size of "tokens"
    for (lo, lc) in list_of_replacement {
        if let Some(l) = lo {
            tokens[l as usize] = Token::ConditionOpen(PositionalToken::new((), 0));
        }
        if let Some(l) = lc {
            tokens[l as usize] = Token::ConditionClose(PositionalToken::new((), 0));
        }
    }

    // Prepare the list of element to insert in the tokens list
    let raw_list_of_inserting: Vec<(u32, Token)> = transform_and_sort(
        &list_of_inserting,
        Token::ConditionOpen(PositionalToken::new((), 0)),
        Token::ConditionClose(PositionalToken::new((), 0)),
    );

    for pos in raw_list_of_inserting {
        tokens.insert(pos.0 as usize, pos.1.clone());
    }

    // Delete the first and last item of the tokens vec, to remove the Lo/Lc we added
    tokens.remove(0);
    tokens.pop();
}

/// Transform the list of positions we computed into a list of element ready to be inserted in the tokens list
fn transform_and_sort(
    list_of_inserting: &Vec<(Option<u32>, Option<u32>)>,
    token_left: Token,
    token_right: Token,
) -> Vec<(u32, Token)> {
    // Transform the list of inserting into a list of (position, Token)
    let mut raw_list_of_inserting: Vec<(u32, Token)> = list_of_inserting
        .iter()
        .flat_map(|&(x, y)| {
            let mut result = vec![];
            if let Some(x_pos) = x {
                result.push((x_pos, token_left.clone()));
            }
            if let Some(y_pos) = y {
                result.push((y_pos, token_right.clone()));
            }
            result
        })
        .collect::<Vec<_>>();

    // Sort in descending order
    raw_list_of_inserting.sort_by(|a, b| b.0.cmp(&a.0));

    raw_list_of_inserting
}

fn check_logical_open_delimiter(tokens: &[Token], position: usize) -> bool {
    tokens.get(position).map_or(false, |t| t.is_logical_open())
}

fn check_logical_close_delimiter(tokens: &[Token], position: usize) -> bool {
    tokens.get(position).map_or(false, |t| t.is_logical_close())
}

/// N1  Normalization N1 by removing the duplicated parentheses. <br/>
///  The idea is to keep an array of the openings than are next to each other (delta is 0)
///
/// ex :   
///        
///        0  -> 1 2        means at token 0 we have a pair openings of depth 1 and 2 <br/>
///        10 -> 2 3        means at token 10 we have a pair openings of depth 2 and 3 <br/>
///        11 -> 3 4        .... <br/>
///
/// Every time we find a pair of closings than are next to each other, we can search their siblings
///  in the array of pair openings. If it exists, it means that there are useless couple of parenthesis
///  so we mark then to be removed.
fn n1_remove_successive_logical_open_close(tokens: &mut Vec<Token>) {
    log_info!("Normalize level 1");
    #[derive(Debug)]
    struct PairPosition {
        position: u32, // token position of x opening
        x: i32,        // first opening of the couple
        y: i32,        // second opening of the delta zero couple
    }

    let mut open_delta_zero: Vec<PairPosition> = vec![];
    let mut depth: i32 = 0;
    let mut last_open_info: Option<(i32, u32)> = None; // ( <depth>, <position of the token>)
    let mut last_close_info: Option<(i32, u32)> = None;
    let mut position_counter: u32 = 0;
    let mut to_be_removed: Vec<u32> = vec![]; // list of <LO position> and <LC position> to be removed

    for token in tokens.iter() {
        match token {
            Token::LogicalOpen(pt) => {
                depth += 1;
                // We check if the previous LO was next to this one
                if let Some(loi) = last_open_info {
                    // if delta is 0
                    if position_counter - (loi.1 + 1) == 0 {
                        open_delta_zero.push(PairPosition {
                            position: loi.1,
                            x: loi.0,
                            y: depth,
                        });
                    }
                }
                last_open_info = Some((depth, position_counter));
                log_debug!("Open Delta Zero {:?}", &open_delta_zero);
            }
            Token::LogicalClose(pt) => {
                depth -= 1;

                // We check if the previous LC was next to this one
                if let Some((last_close_depth, last_close_position)) = last_close_info {
                    // if delta is 0
                    if position_counter - (last_close_position + 1) == 0 {
                        // The depth variable is actually late of 1 step when we go over the closings, so we must suppose we are at depth + 1
                        if let Some(matching_pair) =
                            open_delta_zero.iter_mut().find(|m| m.x == depth + 1)
                        {
                            // mark the couple to be removed
                            to_be_removed.push(matching_pair.position);
                            to_be_removed.push(position_counter);
                            // The pair is no longer usable
                            matching_pair.x = -1;
                            matching_pair.y = -1;
                        }
                        log_debug!("Open Delta Zero After Use {:?}", &open_delta_zero);
                    }
                }

                last_close_info = Some((depth, position_counter));
            }
            _ => {
                if let Some((last_close_depth, last_close_position)) = last_close_info {
                    if position_counter - (last_close_position + 1) == 0 {
                        if let Some(matching_pair) =
                            open_delta_zero.iter_mut().find(|m| m.x == last_close_depth)
                        {
                            // The pair is no longer usable
                            matching_pair.x = -1;
                            matching_pair.y = -1;
                        }
                    }
                }
            }
        }
        position_counter += 1;
    }

    // Sort the position in descending order to make sur to safely delete all the items in the tokens vec
    to_be_removed.sort_by(|a, b| b.cmp(a));
    for pos in to_be_removed {
        tokens.remove(pos as usize);
    }
}

fn extract_position_info(token: &Token) -> usize {
    match token {
        Token::LogicalClose(pt) | Token::LogicalOpen(pt) => pt.position,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use crate::filter::filter_ast::{PositionalToken, Token, TokenSlice};
    use crate::filter::filter_lexer::{lex3, FilterError};
    use crate::filter::filter_normalizer::{
        n1_remove_successive_logical_open_close, n2_mark_condition_open_close,
        n3_binary_logical_operator,
    };
    use crate::filter::tests::init_logger;
    use crate::filter::{ComparisonOperator, LogicalOperator};
    use commons_error::*;
    use log::*;

    #[test]
    pub fn normalize_n3() {
        init_logger();
        // ([age]) AND [height == 174]
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
        ];

        n3_binary_logical_operator(&mut tokens);

        // (([age]) AND [height == 174])
        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // Added for the AND
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),   // Added for the AND
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n3_test_2() {
        init_logger();
        // ([age]) AND [height == 174] AND [weight == 25]
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("weight".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(25, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
        ];

        n3_binary_logical_operator(&mut tokens);

        // ((([age]) AND [height == 174]) AND [weight == 25])
        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // Added for the AND 2
            Token::LogicalOpen(PositionalToken::new((), 0)), // Added for the AND 1
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),   // Added for the AND 1
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("weight".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(25, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),   // Added for the AND 2
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n3_test_3() {
        init_logger();
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("weight".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(25, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // Added for the OR
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)), // Added for the AND
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)), // N2
            Token::Attribute(PositionalToken::new("weight".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(25, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)), // N2
            Token::LogicalClose(PositionalToken::new((), 0)),   // Added for the AND
            Token::LogicalClose(PositionalToken::new((), 0)),   // Added for the OR
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    // "[age < 40] OR ([denis < 5] AND [age > 21]) AND [detail == 6]";
    // will give ([age < 40] OR (([denis < 5] AND [age > 21]) AND [detail == 6]))
    pub fn normalize_n3_test_4() {
        init_logger();
        let mut tokens = vec![
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)), // Added for OR
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)), // Added for AND 1
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)), // AND 1
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)), // Add for AND 1
            Token::LogicalClose(PositionalToken::new((), 0)), // Added for OR
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    // "([age < 40] OR [denis < 5] AND [age > 21])";
    // will give ([age < 40] OR ([denis < 5] AND [age > 21]))
    pub fn normalize_n3_test_5() {
        init_logger();
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n3_test_6() {
        init_logger();
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("A".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("B".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("C".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("A".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("B".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("C".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n2() {
        init_logger();
        // "((age >= 20) AND height == 174)";
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, 0)),
            Token::ValueInt(PositionalToken::new(20, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n2_mark_condition_open_close(&mut tokens);

        // "([age >= 20] AND [height == 174])";
        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, 0)),
            Token::ValueInt(PositionalToken::new(20, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        log_debug!("Expected : {}", TokenSlice(&expected));
        log_debug!("Result : {}", TokenSlice(&tokens));
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n2_1() {
        init_logger();
        // "age >= 20 AND height == 174";
        let mut tokens = vec![
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, 0)),
            Token::ValueInt(PositionalToken::new(20, 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
        ];

        n2_mark_condition_open_close(&mut tokens);

        // "[age >= 20] AND [height == 174]";
        let expected = vec![
            //Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, 0)),
            Token::ValueInt(PositionalToken::new(20, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            // Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        log_debug!("Expected : {}", TokenSlice(&expected));
        log_debug!("Result : {}", TokenSlice(&tokens));
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n2_test_2() {
        init_logger();
        // "((age >= 20)) AND height == 174";
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, 0)),
            Token::ValueInt(PositionalToken::new(20, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
        ];

        n2_mark_condition_open_close(&mut tokens);

        // "([age >= 20]) AND [height == 174]";
        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GTE, 0)),
            Token::ValueInt(PositionalToken::new(20, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("height".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(174, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }

    /// Regardless the validity of the expression, the N2 normalization will mark the condition open and close
    /// and remove the useless logical open and close surrounding the conditions

    #[test]
    pub fn normalize_n2_test_3() {
        init_logger();
        // (age < 40) OR (denis < 5 AND age > 21) AND (detail == 6)
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n2_mark_condition_open_close(&mut tokens);

        // [age < 40] OR ([denis < 5] AND [age > 21]) AND [detail == 6]
        let expected = vec![
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::ConditionOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::ConditionClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }

    ////
    #[test]
    pub fn normalize_n1() {
        init_logger();
        // (( 2 3 4 )) 7 ( 9 ((( 13 )) 16 ))
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("2".to_string(), 0)),
            Token::Attribute(PositionalToken::new("3".to_string(), 0)),
            Token::Attribute(PositionalToken::new("4".to_string(), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("7".to_string(), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("9".to_string(), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("13".to_string(), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("16".to_string(), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        //  ( 2 3 4 ) 7 (    9 ( (13) 16 )    )
        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("2".to_string(), 0)),
            Token::Attribute(PositionalToken::new("3".to_string(), 0)),
            Token::Attribute(PositionalToken::new("4".to_string(), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("7".to_string(), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("9".to_string(), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("13".to_string(), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("16".to_string(), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n1_test_2() {
        init_logger();
        log_debug!("*** normalize_n1_test_2");
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ValueInt(PositionalToken::new(0, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::ValueInt(PositionalToken::new(1, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ValueInt(PositionalToken::new(2, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ValueInt(PositionalToken::new(0, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::ValueInt(PositionalToken::new(1, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::ValueInt(PositionalToken::new(2, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    // "(age < 40) OR (denis < 5 AND age > 21) AND (detail == 6)";
    pub fn normalize_n1_test_3() {
        init_logger();
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("denis".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(5, 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    // "((age < 40) OR  (age > 21)) AND (detail == 6)";
    pub fn normalize_n1_test_4() {
        init_logger();
        let mut tokens = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        let expected = vec![
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::LT, 0)),
            Token::ValueInt(PositionalToken::new(40, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::OR, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("age".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::GT, 0)),
            Token::ValueInt(PositionalToken::new(21, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
            Token::BinaryLogicalOperator(PositionalToken::new(LogicalOperator::AND, 0)),
            Token::LogicalOpen(PositionalToken::new((), 0)),
            Token::Attribute(PositionalToken::new("detail".to_string(), 0)),
            Token::Operator(PositionalToken::new(ComparisonOperator::EQ, 0)),
            Token::ValueInt(PositionalToken::new(6, 0)),
            Token::LogicalClose(PositionalToken::new((), 0)),
        ];
        assert_eq!(expected, tokens);
    }
}
