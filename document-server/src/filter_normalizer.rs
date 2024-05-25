use crate::filter_ast::{LogicalOperator, Token};

///
///
///
pub fn normalize_lexeme(tokens: &mut Vec<Token>) {
    n1_remove_successive_logical_open_close(tokens);
    println!("N1 {:?}", &tokens);
    n2_mark_condition_open_close(tokens);
    println!("N2 {:?}", &tokens);
    n3_binary_logical_operator(tokens);
}

///
/// Normalization N3 - Ensure all logical operator is strictly binary
/// If not, place logical delimiter around it, with priority to AND over OR
///
/// This step of normalization suppose that the N2 is fulfilled
///
fn n3_binary_logical_operator(tokens: &mut Vec<Token>)  {
    n3_binary_logical_operator_for_op(tokens, LogicalOperator::AND);
    n3_binary_logical_operator_for_op(tokens, LogicalOperator::OR);
}

fn n3_binary_logical_operator_for_op(tokens: &mut Vec<Token>, for_lop: LogicalOperator) {
    loop {
        let inserting = find_next_ajustable(tokens, &for_lop);
        dbg!(&inserting);
        match inserting {
            None => {
                break;
            }
            Some((x,y)) => {
                tokens.insert(y as usize, Token::LogicalClose);
                tokens.insert(x as usize, Token::LogicalOpen);
            }
        }
    }
}

fn find_next_ajustable(tokens: &Vec<Token>, for_lop: &LogicalOperator) -> Option<(u32, u32)> {
    let mut position_counter: u32 = 0;
    let mut inserting: Option<(u32, u32)> = None; //(None, None);
    for token in tokens.iter() {
        match token {
            Token::BinaryLogicalOperator(lop) => {
                match lop {
                    LogicalOperator::AND => {
                        if *for_lop == LogicalOperator::AND {
                            dbg!(position_counter);
                            dbg!(&tokens);
                            inserting = check_binary_logical_operator(&tokens, position_counter);
                            println!("Found a position for AND : {:?}", inserting);
                            if inserting.is_some() {
                                break;
                            }
                        }
                    }
                    LogicalOperator::OR => {
                        if *for_lop == LogicalOperator::OR {
                            inserting = check_binary_logical_operator(&tokens, position_counter);
                            if inserting.is_some() {
                                break;
                            }
                        }
                    }
                }
            }
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
    let left : CheckLogicalBoundary = check_logical_one_direction(tokens, position_counter, Direction::Backward);
    // Check backward
    let right : CheckLogicalBoundary = check_logical_one_direction(tokens, position_counter, Direction::Forward);
    match (left.boundary_type, right.boundary_type) {
        (BoundaryType::WithLogical, BoundaryType::WithLogical) => {
            // In case we found logical operators surrounding the AND/OR
            None
        }
        (_, _) => {
            // In case either direction has no logical operator
           Some((left.position, right.position))
        }
        //_ => {panic!("We must have a pair of LO/LC")}
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

fn check_logical_one_direction(tokens: &Vec<Token>, position_counter: u32, direction: Direction) -> CheckLogicalBoundary {
    let mut depth : i32 = 0;
    let mut index : i32 = position_counter as i32;
    let step: i32 = if direction == Direction::Backward { -1 } else { 1 };
    let mut position: u32 = index as u32;
    let mut boundary_type = BoundaryType::WithoutLogical;

    loop {
        index += step;

        if index < 0 {
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
                    Token::LogicalOpen => {
                        // if we are backward, an opening is a decrease of the depth (+step)
                        depth += step;
                        if direction == Direction::Backward {
                            let next_t = tokens.get((index-1) as usize).unwrap_or(&Token::LogicalClose);

                            // (count == 0  and lexeme is not LC/LO)
                            if depth == 0 &&  *next_t != Token::LogicalOpen {
                                // Insert the ( _before_ the [
                                position = index as u32;
                                break;
                            }

                            // found an extra "(" that means we are ok in this direction
                            if depth == -1 {
                                //position = None;
                                boundary_type = BoundaryType::WithLogical;
                                break;
                            }
                        }
                    }
                    Token::LogicalClose => {
                        // if we are forward, a closing is an increase of the depth (-step)
                        depth += -1 * step;

                        let next_t = tokens.get((index+1) as usize).unwrap_or(&Token::LogicalOpen);

                        // (count == 0  and lexeme is not LC/LO)
                        if depth == 0 && direction == Direction::Forward &&  *next_t != Token::LogicalClose {
                            // Insert the ) _after_ the ]
                            position = (index + 1) as u32;
                            break;
                        }

                        if direction == Direction::Forward {
                            if depth == -1 {
                                // position = None;
                                boundary_type = BoundaryType::WithLogical;
                                break;
                            }
                        }

                    }
                    Token::ConditionOpen => {
                        let next_t = tokens.get((index-1) as usize).unwrap_or(&Token::LogicalClose);

                        // (count == 0  and lexeme is not LC/LO)
                        if depth == 0 && direction == Direction::Backward {
                            // Insert the ( _before_ the [
                            position = index as u32;

                            if *next_t == Token::LogicalOpen {
                                boundary_type = BoundaryType::WithLogical;
                            }

                            break;
                        }
                    }
                    Token::ConditionClose => {
                        let next_t = tokens.get((index+1) as usize).unwrap_or(&Token::LogicalOpen);

                        // (count == 0  and lexeme is not LC/LO)
                        if depth == 0 && direction == Direction::Forward  {
                            // Insert the ) _after_ the ]
                            position = (index + 1) as u32;

                            if *next_t == Token::LogicalClose {
                                boundary_type = BoundaryType::WithLogical;
                            }

                            break;
                        }
                    }
                    _ => { }
                }
            }
        }
    }
    CheckLogicalBoundary {
        position,
        boundary_type
    }
}

///
/// Normalization N2 - Remove the useless LO/LC around the conditions
/// Surround the conditions expression with ConditionOpen and ConditionClose
/// Ex :   (A == 12) will be [A == 12]
///         ( A== 12 AND (B == 5) ) will be ( [A == 12] AND [B == 5] )
///
fn n2_mark_condition_open_close(tokens: &mut Vec<Token>)  {
    let mut position_counter: u32 = 0;
    let mut list_of_replacement: Vec<(Option<u32>, Option<u32>)> = vec![];
    let mut list_of_inserting: Vec<(Option<u32>, Option<u32>)> = vec![]; // The place where to insert the CO/CC

    for token in tokens.iter() {
        match token {
            Token::Attribute(_) => {
                let mut replacement : (Option<u32>, Option<u32>) = (None, None);
                let mut inserting : (Option<u32>, Option<u32>) = (None, None);

                let pre_position = position_counter - 1;
                let post_position = position_counter + 3;

                let is_logical_opening = check_logical_delimiter(&tokens, pre_position as usize, Token::LogicalOpen);
                let op_t: Option<&Token> = tokens.get((position_counter+1) as usize);
                let _is_operator = match op_t {
                    None => {
                        // TODO send error
                        false
                    }
                    Some(_t) => {
                        true
                        // if let TokenOperator(tt) == t {
                        //     true
                        // } else {
                        //     false
                        // }
                    }
                };

                let op_t: Option<&Token> = tokens.get((position_counter+2) as usize);
                let _ = match op_t {
                    None => {
                        false
                    }
                    Some(t) => {
                        match *t {
                            Token::ValueInt(_) | Token::ValueString(_) => {true}
                             _ => {false}
                        }
                    }
                };

                let is_logical_closing = check_logical_delimiter(&tokens, post_position as usize, Token::LogicalClose);

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
            tokens[l as usize] = Token::ConditionOpen;
        }
        if let Some(l) = lc {
            tokens[l as usize] = Token::ConditionClose;
        }
    }

    let raw_list_of_inserting: Vec<(u32, Token)> = transform_and_sort(&list_of_inserting, Token::ConditionOpen, Token::ConditionClose);

    for pos in raw_list_of_inserting {
        tokens.insert(pos.0 as usize, pos.1.clone());
        println!(">>> {:?}", &pos);
    }
}

fn transform_and_sort(list_of_inserting: &Vec<(Option<u32>, Option<u32>)>, token_left: Token, token_right: Token) -> Vec<(u32, Token)> {
    // Transformation et collecte des valeurs dans un vecteur de paires
    let mut raw_list_of_inserting: Vec<(u32, Token)> = list_of_inserting
        .iter()
        .flat_map(|&(x, y)| {
            vec![
                (x.unwrap(), token_left.clone()),
                (y.unwrap(), token_right.clone()),
            ]
        })
        .collect::<Vec<_>>();

    // Tri du vecteur par ordre décroissant des valeurs u32
    raw_list_of_inserting.sort_by(|a, b| b.0.cmp(&a.0));

    raw_list_of_inserting
}

fn check_logical_delimiter(tokens: &Vec<Token>, position: usize, logical_delimiter: Token) -> bool {
    let op_t = tokens.get(position);
    match op_t {
        Some(&Token::LogicalOpen) => logical_delimiter == Token::LogicalOpen,
        Some(&Token::LogicalClose) => logical_delimiter == Token::LogicalClose,
        _ => false,
    }
}

///
/// N1  Normalization N1 by removing the duplicated parentheses
/// The idea is to keep an array of the openings than are next to each other (delta is 0)
/// ex :   1 2 0        means at position 0 we have a pair openings of depth 1 and 2
///        2 3 10       means at position 10 we have a pair openings of depth 2 and 3
///        3 4 11       ....
/// Every time we find a pair of closings than are next to each other, we can search their siblings
/// in the array of pair openings. If it exists, it means that there are useless couple of parenthesis
/// so we mark then to be removed.
///
fn n1_remove_successive_logical_open_close(tokens: &mut Vec<Token>)  {
    struct PairPosition {
        x: i32, // first opening of the couple
        y: i32, // second opening of the delta zero couple
        position: u32, // position of x opening
    }

    let mut open_delta_zero : Vec<PairPosition> = vec![];
    let mut depth: i32 = 0;
    let mut last_open_info: Option<(i32, u32)> = None; // ( <depth>, <position of the token>)
    let mut last_close_info: Option<(i32, u32)> = None;
    let mut position_counter: u32 = 0;
    let mut to_be_removed: Vec<u32> = vec![]; // list of <LO position> and <LC position> to be removed

    for token in tokens.iter() {
        match token {
            Token::LogicalOpen => {
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
            }
            Token::LogicalClose => {
                depth -= 1;

                // We check if the previous LC was next to this one
                if let Some((last_close_depth, last_close_position)) = last_close_info {
                    // if delta is 0
                    if position_counter - (last_close_position + 1) == 0 {
                        // The depth variable is actually late of 1 step when we go over the closings, so we must suppose we are at depth + 1
                        if let Some(matching_pair) = open_delta_zero.iter_mut().find(|m| m.x == depth + 1) {
                            // mark the couple to be removed
                            to_be_removed.push(matching_pair.position);
                            to_be_removed.push(position_counter);
                            // The pair is no longer usable
                            matching_pair.x = -1;
                            matching_pair.y = -1;
                        }
                    }
                }

                last_close_info = Some((depth, position_counter));
            }
            _ => {
                if let Some((last_close_depth, last_close_position)) = last_close_info {
                    if position_counter - (last_close_position + 1) == 0 {
                        if let Some(matching_pair) = open_delta_zero.iter_mut().find(|m| m.x == last_close_depth) {
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


#[cfg(test)]
mod tests {
    //cargo test --color=always --bin document-server expression_filter_parser::tests   -- --show-output

    use crate::filter_ast::{ComparisonOperator, LogicalOperator, Token};
    use crate::filter_normalizer::{n1_remove_successive_logical_open_close, n2_mark_condition_open_close, n3_binary_logical_operator};

    #[test]
    pub fn normalize_n3() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::ConditionOpen, // N2
            Token::Attribute("age".to_string()),
            Token::ConditionClose, // N2
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose, // N2
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen, // Added for the AND

            Token::LogicalOpen,
            Token::ConditionOpen, // N2
            Token::Attribute("age".to_string()),
            Token::ConditionClose, // N2
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose, // N2

            Token::LogicalClose, // Added for the AND
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n3_test_2() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::ConditionOpen, // N2
            Token::Attribute("age".to_string()),
            Token::ConditionClose, // N2
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose, // N2

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("weight".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(25),
            Token::ConditionClose, // N2
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen, // Added for the AND 2
            Token::LogicalOpen, // Added for the AND 1

            Token::LogicalOpen,
            Token::ConditionOpen, // N2
            Token::Attribute("age".to_string()),
            Token::ConditionClose, // N2
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose, // N2

            Token::LogicalClose, // Added for the AND 1

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("weight".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(25),
            Token::ConditionClose, // N2

            Token::LogicalClose, // Added for the AND 2
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n3_test_3() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::ConditionOpen, // N2
            Token::Attribute("age".to_string()),
            Token::ConditionClose, // N2
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::ConditionOpen, // N2
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose, // N2

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("weight".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(25),
            Token::ConditionClose, // N2
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen, // Added for the OR
            Token::LogicalOpen,
            Token::ConditionOpen, // N2
            Token::Attribute("age".to_string()),
            Token::ConditionClose, // N2
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen, // Added for the AND
            Token::ConditionOpen, // N2
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose, // N2

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen, // N2
            Token::Attribute("weight".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(25),
            Token::ConditionClose, // N2

            Token::LogicalClose, // Added for the AND
            Token::LogicalClose, // Added for the OR
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    // "[age < 40] OR ([denis < 5] AND [age > 21]) AND [detail == 6]";
    // will give ([age < 40] OR (([denis < 5] AND [age > 21]) AND [detail == 6]))
    pub fn normalize_n3_test_4() {
        let mut tokens = vec![
            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::ConditionClose,
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::ConditionClose,
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen, // Added for OR
            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen, // Added for AND 1
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::ConditionClose,
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND), // AND 1

            Token::ConditionOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::ConditionClose,
            Token::LogicalClose, // Add for AND 1
            Token::LogicalClose, // Added for OR
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    // "([age < 40] OR [denis < 5] AND [age > 21])";
    // will give ([age < 40] OR ([denis < 5] AND [age > 21]))
    pub fn normalize_n3_test_5() {
        let mut tokens = vec![
            Token::LogicalOpen,

            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::ConditionOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::ConditionClose,

            Token::LogicalClose,
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen,

            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::ConditionClose,
            Token::LogicalClose,

            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n3_test_6() {
        let mut tokens = vec![
            Token::LogicalOpen,

            Token::ConditionOpen,
            Token::Attribute("A".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::ConditionOpen,
            Token::Attribute("B".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("C".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::ConditionClose,

            Token::LogicalClose
        ];

        n3_binary_logical_operator(&mut tokens);

        let expected = vec![
            Token::LogicalOpen,

            Token::ConditionOpen,
            Token::Attribute("A".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,

            Token::ConditionOpen,
            Token::Attribute("B".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("C".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::ConditionClose,

            Token::LogicalClose,

            Token::LogicalClose
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n2() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(20),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),

            Token::LogicalClose,
        ];

        n2_mark_condition_open_close(&mut tokens);
        let expected = vec![
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(20),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose,

            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
        // println!(".... {:?}", &tokens);
    }

    #[test]
    pub fn normalize_n2_test_2() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(20),
            Token::LogicalClose,
            Token::LogicalClose,
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
        ];

        n2_mark_condition_open_close(&mut tokens);
        let expected = vec![
            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GTE),
            Token::ValueInt(20),
            Token::ConditionClose,
            Token::LogicalClose,

            Token::ConditionOpen,
            Token::Attribute("height".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(174),
            Token::ConditionClose,
        ];
        assert_eq!(expected, tokens);
        // println!(".... {:?}", &tokens);
    }


    #[test]
    // "(age < 40) OR (denis < 5 AND age > 21) AND (detail == 6)";
    pub fn normalize_n2_test_3() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::LogicalOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::LogicalClose,
        ];

        n2_mark_condition_open_close(&mut tokens);

        let expected = vec![
            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::ConditionOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),
            Token::ConditionClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::ConditionClose,
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::ConditionOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::ConditionClose,
        ];
        assert_eq!(expected, tokens);
    }

    ////
    #[test]
    pub fn normalize_n1() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("2".to_string()),
            Token::Attribute("3".to_string()),
            Token::Attribute("4".to_string()),
            Token::LogicalClose,
            Token::LogicalClose,
            Token::Attribute("7".to_string()),
            Token::LogicalOpen,
            Token::Attribute("9".to_string()),
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("13".to_string()),
            Token::LogicalClose,
            Token::LogicalClose,
            Token::Attribute("16".to_string()),
            Token::LogicalClose,
            Token::LogicalClose,
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        let expected = vec![
            Token::LogicalOpen,
            Token::Attribute("2".to_string()),
            Token::Attribute("3".to_string()),
            Token::Attribute("4".to_string()),
            Token::LogicalClose,
            Token::Attribute("7".to_string()),
            Token::LogicalOpen,
            Token::Attribute("9".to_string()),
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("13".to_string()),
            Token::LogicalClose,
            Token::Attribute("16".to_string()),
            Token::LogicalClose,
            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    pub fn normalize_n1_test_2() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::ValueInt(0),
            Token::LogicalOpen,
            Token::LogicalClose,
            Token::ValueInt(1),
            Token::LogicalClose,
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::ValueInt(2),
            Token::LogicalClose,
            Token::LogicalClose,
            Token::LogicalClose,
            Token::LogicalClose,
            Token::LogicalClose,
            Token::LogicalClose,
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        let expected = vec![
            Token::LogicalOpen,
            Token::ValueInt(0),
            Token::LogicalOpen,
            Token::LogicalClose,
            Token::ValueInt(1),
            Token::LogicalClose,
            Token::LogicalOpen,
            Token::ValueInt(2),
            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
    }


    #[test]
    // "(age < 40) OR (denis < 5 AND age > 21) AND (detail == 6)";
    pub fn normalize_n1_test_3() {
        let mut tokens = vec![
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::LogicalOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::LogicalClose,
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        let expected = vec![
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::OR),

            Token::LogicalOpen,
            Token::Attribute("denis".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(5),

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::LogicalClose,

            Token::BinaryLogicalOperator(LogicalOperator::AND),

            Token::LogicalOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::LogicalClose,
        ];
        assert_eq!(expected, tokens);
    }

    #[test]
    // "((age < 40) OR  (age > 21)) AND (detail == 6)";
    pub fn normalize_n1_test_4() {
        let mut tokens = vec![
            //Token::LogicalOpen,
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::LogicalClose,
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::LogicalOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::LogicalClose,
            //Token::LogicalClose
        ];

        n1_remove_successive_logical_open_close(&mut tokens);

        let expected = vec![
            //Token::LogicalOpen,
            Token::LogicalOpen,
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::LT),
            Token::ValueInt(40),
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::OR),
            Token::LogicalOpen,
            Token::Attribute("age".to_string()),
            Token::Operator(ComparisonOperator::GT),
            Token::ValueInt(21),
            Token::LogicalClose,
            Token::LogicalClose,
            Token::BinaryLogicalOperator(LogicalOperator::AND),
            Token::LogicalOpen,
            Token::Attribute("detail".to_string()),
            Token::Operator(ComparisonOperator::EQ),
            Token::ValueInt(6),
            Token::LogicalClose,
            //Token::LogicalClose
        ];
        assert_eq!(expected, tokens);
    }

    //
}