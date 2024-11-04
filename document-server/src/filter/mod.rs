use log::error;

use commons_error::*;
use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use std::fmt;
use chrono::format::Numeric::Second;
use commons_error::tr_fwd;
use dkdto::{ClearTextReply, TagElement, TagType};
use crate::filter::filter_ast::{LogicalOperator, parse_tokens, TokenParseError};
use crate::filter::filter_lexer::lex3;
use crate::filter::filter_normalizer::normalize_lexeme;

mod filter_ast;
mod filter_lexer;
mod filter_normalizer;

const EXTRA_TABLE_PREFIX : &str = "ot";

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

#[derive(Debug, Clone)]
pub(crate) enum FilterValue {
    ValueInt(i32),
    ValueString(String),
}

impl fmt::Display for FilterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterValue::ValueInt(i) => {
                write!(f, "{}", i)
            }
            FilterValue::ValueString(s) => {
                write!(f, "\"{}\"", s.as_str())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub (crate) struct FilterCondition {
    key: String, // a unique key to identify the leaves
    attribute: String,
    operator: ComparisonOperator,
    value: FilterValue,
}

#[derive(Debug)]
pub(crate) enum FilterExpressionAST {
    Condition (FilterCondition),
    Logical {
        operator: LogicalOperator,
        leaves: Vec<Box<FilterExpressionAST>>,
    },
}

pub(crate) fn analyse_expression(
    expression: &str,
) -> Result<Box<FilterExpressionAST>, TokenParseError> {
    match lex3(expression) {
        Ok(mut tokens) => {
            normalize_lexeme(&mut tokens);
            parse_tokens(&mut tokens)
        }
        Err(e) => {
            // TODO convert the LexerError e in to a TokenParseError
            Err(TokenParseError::ValueExpected((
                e.char_position as usize,
                None,
            )))
        }
    }
}

pub(crate) fn to_sql_form(
    filter_expression: &FilterExpressionAST,
) -> Result<String, TokenParseError> {
    let mut content: String = String::from("");
    match filter_expression {
        FilterExpressionAST::Condition(FilterCondition {key,attribute, operator, value}) => {
            let sql_op = match operator {
                ComparisonOperator::EQ => "=",
                ComparisonOperator::NEQ => "<>",
                ComparisonOperator::GT => ">",
                ComparisonOperator::LT => "<",
                ComparisonOperator::GTE => ">=",
                ComparisonOperator::LTE => "<=",
                ComparisonOperator::LIKE => "LIKE",
            };

            let s = format!("({} {} {})", attribute, sql_op, value);
            content.push_str(&s);
        }
        FilterExpressionAST::Logical { operator, leaves } => {
            content.push_str("(");

            for (i, l) in leaves.iter().enumerate() {
                let r_leaf_content = to_sql_form(l);
                if let Ok(leaf) = r_leaf_content {
                    content.push_str(&leaf);
                }
                if i < leaves.len() - 1 {
                    content.push_str(&format!(" {:?} ", &operator));
                }
            }
            content.push_str(")");
        }

    }
    Ok(content)
}

fn vectorize_conditions(filter_expression: &FilterExpressionAST) -> Result<Vec<FilterCondition>, GenerationError> {
    let mut filter_conditions: Vec<FilterCondition> = vec![];
    match filter_expression {
        FilterExpressionAST::Condition(filter_condition) => {
            filter_conditions.push((*filter_condition).clone());
        }
        FilterExpressionAST::Logical { operator, leaves } => {
            for (i, l) in leaves.iter().enumerate() {
                if let Ok(leaf) = vectorize_conditions(l) {
                    filter_conditions.extend(leaf);
                }
            }
        }
    }
    Ok(filter_conditions)
}

pub(crate) fn extract_all_conditions(
    filter_expression_ast: &FilterExpressionAST,
) -> Result<HashMap<String,(u32, FilterCondition)>, GenerationError> {
    let mut filter_conditions = vectorize_conditions(filter_expression_ast)?;

    // Put the filter conditions in the hash map
    let mut all_conditions_map : HashMap<String,(u32, FilterCondition)> = HashMap::new();

    for fc in filter_conditions {
        let attribute_count = all_conditions_map
            .values()
            .filter(|(index, filter_condition)| &filter_condition.attribute == &fc.attribute)
            .count() as u32;

        all_conditions_map.insert(fc.key.clone(), (attribute_count, fc));
    }
    Ok(all_conditions_map)
}

/// from the AST, we extract complete filter but replacing the actual filter conditions with  ot_{{tag_name}}.value is not null
/// Be careful, the filter_conditions must have been generated from the same filter_expression AST
pub(crate) fn extract_boolean_filter(filter_expression_ast: &FilterExpressionAST, filter_conditions: &HashMap<String, (u32, FilterCondition)>)
                                     -> Result<String, GenerationError> {
    let mut content: String = String::from("");
    match filter_expression_ast {
        FilterExpressionAST::Condition(FilterCondition {key,attribute, operator, value}) => {
            // Search the key in the hashmap

            match filter_conditions.get(key) {
                None => {
                    panic!("No matching conditions"); // TODO ...
                }
                Some((index, fc)) => {
                    let s = format!(" {}_{}_{}.value is not null ", EXTRA_TABLE_PREFIX, &fc.attribute, index);
                    content.push_str(&s);
                }
            }
        }
        FilterExpressionAST::Logical { operator, leaves } => {
            content.push_str("(");

            for (i, l) in leaves.iter().enumerate() {
                let r_leaf_content = extract_boolean_filter(l, filter_conditions);
                if let Ok(leaf) = r_leaf_content {
                    content.push_str(&leaf);
                }
                if i < leaves.len() - 1 {
                    content.push_str(&format!(" {:?} ", &operator));
                }
            }
            content.push_str(")");
        }

    }
    Ok(content)
}


fn generate_tag_value_filter(filter_condition: &FilterCondition, tag_type: &TagType) -> Result<String, GenerationError> {

    let sql_op = match filter_condition.operator {
        ComparisonOperator::EQ => "=",
        ComparisonOperator::NEQ => "<>",
        ComparisonOperator::GT => ">",
        ComparisonOperator::LT => "<",
        ComparisonOperator::GTE => ">=",
        ComparisonOperator::LTE => "<=",
        ComparisonOperator::LIKE => "LIKE",
    };

    let tag_value_filter = match tag_type {
        TagType::Text => {
            //unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
            format!("unaccent_lower((tv.value_string)::text) {0} unaccent_lower('{1}')", &sql_op, &filter_condition.value)}
        TagType::Bool => {
            // science == true
            format!("tv.value_boolean {0} {1}", &sql_op, &filter_condition.value)}
        TagType::Int => {
            format!("tv.value_integer {0} {1}", &sql_op, &filter_condition.value)}
        TagType::Double => {
            format!("tv.value_double {0} {1}", &sql_op, &filter_condition.value)
        }
        TagType::Date => {todo!();}
        TagType::DateTime => {todo!();}
        TagType::Link => {todo!();}
    };

    Ok(tag_value_filter)
}

enum SearchSqlGenerationMode {
    Live,
    Persisted,
}

#[derive(Debug)]
enum GenerationError {
    TagUnknown(String),
    TagTypeUnknown(String),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub(crate) fn generate_generate_search_sql(filter_expression_ast: &FilterExpressionAST, generation_mode : SearchSqlGenerationMode ) -> Result<String, GenerationError> {
    // get all the final nodes (leaves), for instance, == (lastname, "a%" )
    let filter_conditions = extract_all_conditions(&filter_expression_ast).map_err(tr_fwd!())?;

    // extract all the tags' name from each leaves
    let tags: HashSet<_> =  filter_conditions.iter().map(|(_, (_, filter_condition))| filter_condition.attribute.clone() ).collect();

    // find the properties for all the tags ( type , limit, default value)

    // generate the {{tag_value_filter}} for all tags condition
    let tag_value_filters : Vec<String> = filter_conditions.iter().map(|(_, (_, filter_condition))| generate_tag_value_filter(&filter_condition, &TagType::Text).unwrap() /* TODO */ ).collect();

    if let SearchSqlGenerationMode::Persisted = generation_mode {
        // evaluate the count of items from the tag_value_filter
        ()
        // determine which ones are selective or not

        // store the stats in the database

        // super filters
        // Group the terminal "AND" leaves

        // Compute the super filter for all tag_value_filter
    }

    // generate the boolean_filter
    let boolean_filter = extract_boolean_filter(&filter_expression_ast, &filter_conditions).map_err(tr_fwd!())?;

    // generate the order

    // generate the DOKA search sql
    Ok(String::from(""))
}

#[cfg(test)]
mod tests {

    // cargo test --color=always --bin document-server filter  [ -- --show-output]

    use crate::filter::{analyse_expression, ComparisonOperator, extract_all_conditions, extract_boolean_filter, to_sql_form};
    use crate::filter::filter_ast::{ to_canonical_form, TokenParseError};

    #[test]
    pub fn extract_conditions_1() {
        let input1 = " (country == \"FR\"  AND  (science >= 50) OR (lost_in_hell == \"TRUE\" OR (country == \"LU\" AND science >=50) ) )";
        let tree1 = analyse_expression(input1).unwrap();
        let canonical1 = to_canonical_form(tree1.as_ref()).unwrap();
        let all_conditions = extract_all_conditions(tree1.as_ref()).unwrap();
        println!("all_conditions...{:?}", all_conditions);
        assert_eq!(5, all_conditions.values().len());
        let count_country = all_conditions.iter().filter(|(c, d)| &d.1.attribute == "country").count();
        let count_science = all_conditions.iter().filter(|(c, d)| &d.1.attribute == "science").count();
        let count_lost_in_hell = all_conditions.iter().filter(|(c, d)| &d.1.attribute == "lost_in_hell").count();
        assert_eq!(2, count_country);
        assert_eq!(2, count_science);
        assert_eq!(1, count_lost_in_hell);
    }


    #[test]
    pub fn extract_boolean_filter_1() {
        let input1 = " (country == \"FR\"  AND  (science >= 50) OR (lost_in_hell == \"TRUE\" OR (country == \"LU\" AND science >=50) ) )";
        let tree1 = analyse_expression(input1).unwrap();
        let canonical1 = to_canonical_form(tree1.as_ref()).unwrap();
        let all_conditions = extract_all_conditions(tree1.as_ref()).unwrap();
        let boolean_filter = extract_boolean_filter(tree1.as_ref(), &all_conditions).unwrap();
        println!("boolean filter: {}", &boolean_filter);

        const EXPECTED : &str = "(( ot_country_0.value is not null  AND  ot_science_0.value is not null ) OR ( ot_lost_in_hell_0.value is not null  OR ( ot_country_1.value is not null  AND  ot_science_1.value is not null )))";
        assert_eq!(EXPECTED, &boolean_filter);
    }
}