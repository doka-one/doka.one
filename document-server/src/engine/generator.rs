use crate::filter::filter_ast::{ComparisonOperator, FilterCondition, FilterExpressionAST};
use commons_error::tr_fwd;
use commons_error::*;
use dkdto::{TagType, WebType};
use log::*;
use std::collections::{HashMap, HashSet};
use std::fmt;
use axum::async_trait;

use once_cell::sync::Lazy;
use commons_pg::sql_transaction::SQLDataSet;
use commons_pg::sql_transaction_async::{SQLConnectionAsync, SQLQueryBlockAsync};
use commons_services::x_request_id::Follower;
use dkdto::error_codes::INTERNAL_DATABASE_ERROR;

const EXTRA_TABLE_PREFIX: &str = "ot";

static LEGAL_OPERATORS_BY_TAG_TYPE: Lazy<HashMap<TagType, Vec<ComparisonOperator>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(TagType::Bool, vec![ComparisonOperator::EQ, ComparisonOperator::NEQ]);
    map.insert(
        TagType::Int,
        vec![
            ComparisonOperator::EQ,
            ComparisonOperator::NEQ,
            ComparisonOperator::GT,
            ComparisonOperator::GTE,
            ComparisonOperator::LT,
            ComparisonOperator::LTE,
        ],
    );
    map.insert(
        TagType::Double,
        vec![
            ComparisonOperator::EQ,
            ComparisonOperator::NEQ,
            ComparisonOperator::GT,
            ComparisonOperator::GTE,
            ComparisonOperator::LT,
            ComparisonOperator::LTE,
        ],
    );
    map.insert(TagType::Text, vec![ComparisonOperator::EQ, ComparisonOperator::NEQ, ComparisonOperator::LIKE]);
    map
});

pub(crate) enum SearchSqlGenerationMode {
    Live,
    Persisted,
}

#[derive(Debug)]
pub(crate) struct TagDefinition {
    tag_names: String,
    tag_type: TagType,
}

/// Extract all the filter conditions from the filter_expression AST
fn extract_all_conditions(
    filter_expression_ast: &FilterExpressionAST,
) -> Result<HashMap<String, (u32, FilterCondition)>, GenerationError> {
    let filter_conditions = vectorize_conditions(filter_expression_ast)?;

    // Put the filter conditions in the hash map
    let mut all_conditions_map: HashMap<String, (u32, FilterCondition)> = HashMap::new();

    for fc in filter_conditions {
        let attribute_count = all_conditions_map
            .values()
            .filter(|(index, filter_condition)| &filter_condition.attribute == &fc.attribute)
            .count() as u32;

        all_conditions_map.insert(fc.key.clone(), (attribute_count, fc));
    }
    Ok(all_conditions_map)
}

/// Vectorize the filter_expression AST into a vector of filter conditions
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

/// From the AST, we extract complete filter but replacing the actual filter conditions with  ot_{{tag_name}}.value is not null
/// Be careful, the filter_conditions must have been generated from the same filter_expression AST
pub(crate) fn build_query_filter(
    filter_expression_ast: &FilterExpressionAST,
    filter_conditions: &HashMap<String, (u32, FilterCondition)>,
) -> Result<String, GenerationError> {
    let mut content: String = String::from("");
    match filter_expression_ast {
        FilterExpressionAST::Condition(FilterCondition { key, attribute, operator, value }) => {
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
                let r_leaf_content = build_query_filter(l, filter_conditions);
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

#[derive(Debug)]
pub(crate) enum GenerationError {
    TagUnknown(String),
    TagTypeUnknown(String),
    TagSearchError(String),
    TagIncompatibleType(String),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

use std::sync::Arc;
use anyhow::Result;

// Make sure Follower implements Debug (or Display).
// #[derive(Debug)]
// pub struct Follower { /* ... */ }

pub(crate) struct TagDefinitionBuilder {
    follower: Follower,
}

impl TagDefinitionBuilder {
    pub fn new(follower: Follower) -> Self {
        Self { follower }
    }
}

#[async_trait]
trait TagDefinitionInterface {
    /// Prefer slices over &Vec<T>
    async fn get_tag_definition(&self, tag_name: &[String]) -> Result<Vec<TagDefinition>>;
}

#[async_trait]
impl TagDefinitionInterface for TagDefinitionBuilder {
    async fn get_tag_definition(&self, _tag_name: &[String]) -> Result<Vec<TagDefinition>> {
        // Example: include follower in every error path
        let mut cnx = SQLConnectionAsync::from_pool()
            .await
            .map_err(err_fwd!("New DB connection failed; follower={:?}", self.follower))?;

        let mut trans = cnx.begin()
            .await
            .map_err(err_fwd!("Transaction issue; follower={:?}", self.follower))?;

        let mut params = HashMap::new();
        // params.insert("p_customer_code".to_owned(), p_customer_code);

        let sql_query = r#"SELECT 1 FROM dokaadmin.customer WHERE code = :p_customer_code"#.to_owned();

        let query = SQLQueryBlockAsync {
            sql_query,
            params,
            start: 0,
            length: Some(1),
        };

        let _sql_result: SQLDataSet = query.execute(&mut trans)
            .await
            .map_err(err_fwd!(
                "Query failed [{}]; follower={:?}",
                &query.sql_query,
                self.follower
            ))?;

        // If your transaction commit is async, keep `.await`; if not, remove it.
        trans.commit().await?;

        Ok(vec![])
    }
}


///
fn build_tag_value_filter(filter_condition: &FilterCondition, tag_type: &TagType) -> Result<String, GenerationError> {
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
            format!(
                "unaccent_lower((tv.value_string)::text) {0} unaccent_lower('{1}')",
                &sql_op, &filter_condition.value
            )
        }
        TagType::Bool => {
            // science = true
            let value = filter_condition.value.to_string().to_lowercase();
            dbg!(&value);
            if filter_condition.operator == ComparisonOperator::EQ && value == "true" {
                "tv.value_boolean".to_string()
            } else {
                "NOT tv.value_boolean".to_string()
            }
        }
        TagType::Int => {
            format!("tv.value_integer {0} {1}", &sql_op, &filter_condition.value)
        }
        TagType::Double => {
            format!("tv.value_double {0} {1}", &sql_op, &filter_condition.value)
        }
        TagType::Date => {
            todo!();
        }
        TagType::DateTime => {
            todo!();
        }
        TagType::Link => {
            todo!();
        }
    };

    Ok(tag_value_filter)
}

/// Verify if all the tags are defined, compare tags and definitions by looping on tags and finding the definition
fn verify_filter_conditions(
    filter_conditions: &HashMap<String, (u32, FilterCondition)>,
    definitions: &Vec<TagDefinition>,
) -> Result<(), GenerationError> {
    for (_, (_, filter_condition)) in filter_conditions.iter() {
        if let Some(definition) = definitions.iter().find(|def| &def.tag_names == &filter_condition.attribute) {
            /** TODO we must also check the value format depending on the tag type here */
            // Check if the operator is valid for the tag type
            if let Some(valid_operators) = LEGAL_OPERATORS_BY_TAG_TYPE.get(&definition.tag_type) {
                if !valid_operators.contains(&filter_condition.operator) {
                    return Err(GenerationError::TagIncompatibleType(format!(
                        "Tag : {}, Invalid operator {:?} for tag type {:?}",
                        &filter_condition.attribute, &filter_condition.operator, &definition.tag_type
                    )));
                }
            } else {
                return Err(GenerationError::TagIncompatibleType(format!(
                    "Tag : {}, No valid operators defined for tag type {:?}",
                    &filter_condition.attribute, &definition.tag_type
                )));
            }
        } else {
            return Err(GenerationError::TagIncompatibleType(format!(
                "Tag attribute '{}' not found in definitions",
                &filter_condition.attribute
            )));
        }
    }
    Ok(())
}

fn build_order_column(order_tags: &[&str], map_of_tags_with_occurrence: &HashMap<String, Vec<String>>) -> Vec<String> {
    order_tags
        .iter()
        .filter_map(|tag| map_of_tags_with_occurrence.get(&tag.to_string()))
        .map(|occurrences| {
            let mut coalesce_expr = occurrences.iter().map(|o| format!("{}.value", o)).collect::<Vec<_>>();
            if coalesce_expr.len() == 1 {
                coalesce_expr.pop().unwrap()
            } else {
                let mut expr = coalesce_expr.pop().unwrap();
                while let Some(next) = coalesce_expr.pop() {
                    expr = format!("COALESCE({}, {})", next, expr);
                }
                expr
            }
        })
        .collect()
}

fn build_tag_column_with_alias(
    select_tags: &[&str],
    map_of_tags_with_occurrence: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    select_tags
        .iter()
        .filter_map(|tag| map_of_tags_with_occurrence.get(&tag.to_string()).map(|occurrences| (tag, occurrences)))
        .map(|(tag, occurrences)| {
            if occurrences.len() == 1 {
                format!("{}.value AS {}", occurrences[0], tag)
            } else if occurrences.len() == 2 {
                format!("COALESCE({}.value, {}.value) AS {}", occurrences[0], occurrences[1], tag)
            } else {
                // For 3 or more, nest COALESCE as requested
                let mut expr = format!("COALESCE({}.value, {}.value)", occurrences[0], occurrences[1]);
                for o in &occurrences[2..] {
                    expr = format!("COALESCE({}, {}.value)", expr, o);
                }
                format!("{} AS {}", expr, tag)
            }
        })
        .collect()
}

/// 🔑 Generate the SQL query from the filter AST
///    
///     REF_TAG : DOKA_SEARCH_SQL
pub(crate) async fn generate_search_sql<T: TagDefinitionInterface>(
    filter_expression_ast: &FilterExpressionAST,
    tag_definition_builder: &T,
    select_tags: &[&str],
    order_tags: &[&str],
    generation_mode: SearchSqlGenerationMode,
) -> Result<String, GenerationError> {
    // Get all the final nodes (leaves), for instance, == (lastname, "a%" )
    let filter_conditions = extract_all_conditions(&filter_expression_ast).map_err(tr_fwd!())?;

    dbg!(&filter_conditions);

    // Extract all the tags name from each leaves
    let tags: HashSet<_> =
        filter_conditions.iter().map(|(_, (_, filter_condition))| filter_condition.attribute.clone()).collect();

    dbg!(&tags);

    // Find the tag_definitions for all the tags (type, limit, default value)
    let tags_list: Vec<String> = tags.iter().cloned().collect();

    let definitions = match tag_definition_builder
        .get_tag_definition(&tags_list).await
        .map_err(tr_fwd!())
    {
        Ok(definitions) => definitions,
        Err(e) => {
            // TODO tracer and session id ?
            log_error!("Error while getting tag definitions: {:?}", e);
            return Err(GenerationError::TagSearchError("Error in tag search".to_string()));
        }
    };

    dbg!(&definitions);

    // Verify if all the tags are defined, compare tags and definitions by looping on tags and finding the definition
    // if a tag is not in definition, throw an error
    for tag in tags.iter() {
        if !definitions.iter().any(|def| &def.tag_names == tag) {
            return Err(GenerationError::TagUnknown(tag.clone()));
        }
    }

    // Verify if the filter conditions are compatible with the tag type
    if let Err(e) = verify_filter_conditions(&filter_conditions, &definitions) {
        // TODO tracer and session id ?
        log_error!("Error while verifying filter conditions: {:?}", e);
        return Err(e);
    }

    let mut map_of_tags_with_occurrence: HashMap<String, Vec<String>> = HashMap::new();

    // Generate the {{tag_value_filter}} for all tags condition
    let mut list_of_query_tags: Vec<String> = vec![];
    for (_, (occurrence, fc)) in filter_conditions.iter() {
        let tag_type = definitions.iter().find(|def| def.tag_names == fc.attribute).map(|def| &def.tag_type).unwrap();

        let tag_value_filter = build_tag_value_filter(fc, tag_type).map_err(|e| {
            log_error!("Error while building tag value filter: {:?}", e);
            e
        })?;

        log_debug!("tag_value_filter: {}", &tag_value_filter);

        let query_tag =
            build_query_tag(fc, tag_type, *occurrence, &tag_value_filter).map_err(tr_fwd!()).map_err(|e| {
                log_error!("Error while building query filter: {:?}", e);
                GenerationError::TagSearchError("Error in tag search".to_string())
            })?;

        let tag_occurrence = format!("ot_{}_{}", &fc.attribute, occurrence);

        dbg!(&tag_occurrence);

        // Add the tag_occurrence to a list associated with the tag name through a hash map
        map_of_tags_with_occurrence.entry(fc.attribute.clone()).or_insert_with(Vec::new).push(tag_occurrence);

        dbg!(&map_of_tags_with_occurrence);

        log_info!("query_tags: {}", &query_tag);

        list_of_query_tags.push(query_tag);
    }

    // build the query filter aka boolean filter
    let query_filter = build_query_filter(&filter_expression_ast, &filter_conditions).map_err(tr_fwd!())?;

    dbg!(&query_filter);

    // build the order columns
    let order_columns = build_order_column(order_tags, &map_of_tags_with_occurrence).join(", ");

    dbg!(&order_columns);

    // build the tag_columns
    let tag_columns = build_tag_column_with_alias(select_tags, &map_of_tags_with_occurrence).join(", ");

    dbg!(&tag_columns);

    if let SearchSqlGenerationMode::Persisted = generation_mode {
        // Evaluate the count of items from the tag_value_filter
        ()
        // determine which ones are selective or not

        // store the stats in the database

        // super filters
        // Group the terminal "AND" leaves

        // Compute the super filter for all tag_value_filter
    }

    // Build the final SQL

    let mut final_sql = String::from("SELECT i.id,");
    final_sql.push_str(&tag_columns);

    final_sql.push_str(" FROM item i ");

    final_sql.push_str(&list_of_query_tags.join(" "));

    final_sql.push_str(" WHERE ");

    final_sql.push_str(query_filter.as_str());

    final_sql.push_str(" ORDER BY ");

    final_sql.push_str(order_columns.as_str());

    // generate the DOKA search sql
    Ok(final_sql.to_string())
}

const QUERY_FILTER_TEMPLATE: &str = r#"LEFT OUTER JOIN (
                                        SELECT tv.item_id, tv.{{value_column_name}} as value
                                        FROM tag_definition td
                                        JOIN tag_value tv ON tv.tag_id = td.id 
                                        AND td."name" = '{{tag_name}}' {{tag_value_filter}} {{tag_super_filter}}
                                    ) ot_{{tag_name}}_{{occurence}} ON ot_{{tag_name}}_{{occurence}}.item_id = i.id"#;

fn build_query_tag(
    filter_condition: &FilterCondition,
    tag_type: &TagType,
    occurence: u32,
    tag_value_filter: &str,
) -> anyhow::Result<String> {
    let query_filter = QUERY_FILTER_TEMPLATE
        .replace("{{value_column_name}}", tag_type.value_column_name())
        .replace("{{tag_name}}", &filter_condition.attribute)
        .replace("{{tag_value_filter}}", &format!("AND {}", tag_value_filter))
        .replace("{{tag_super_filter}}", "") // Add super filter logic if needed
        .replace("{{occurence}}", &occurence.to_string());
    Ok(query_filter)
}

#[cfg(test)]
mod tests {

    // cargo test --color=always --bin document-server engine  [ -- --show-output]

    use crate::engine::generator::{
        build_query_filter, extract_all_conditions, generate_search_sql, verify_filter_conditions, GenerationError,
        SearchSqlGenerationMode, TagDefinition, TagDefinitionInterface,
    };
    use crate::filter::analyse_expression;
    use crate::filter::filter_ast::{
        to_canonical_form, ComparisonOperator, FilterCondition, FilterExpressionAST, FilterValue,
    };
    use crate::parser_log;
    use commons_error::*;
    use dkdto::TagType;
    use log::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Once};
    use sqlparser::parser::Parser;
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::ast::{Statement, SetExpr, TableFactor, TableWithJoins, Query, ObjectName};
    use std::collections::HashSet;
    use axum::async_trait;
    use commons_services::x_request_id::{Follower, XRequestID};
    use doka_cli::request_client::TokenType;
    use crate::filter::filter_lexer::FilterError;

    static INIT_LOGGER: Once = Once::new();

    pub(crate) fn init_logger() {
        INIT_LOGGER.call_once(|| {
            if let Err(e) = log4rs::init_file(
                //"/home/denis/Projects/wks-doka-one/doka.one/document-server/log4rs.yaml",
                 r#"C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\log4rs.yaml"#,
                Default::default(),
            ) {
                panic!("{:?}", e);
            }
        });
    }

    struct TagDefinitionBuilderMock {}

    #[async_trait]
    impl TagDefinitionInterface for TagDefinitionBuilderMock {
        async fn get_tag_definition(&self, tag_name: &[String]) -> anyhow::Result<Vec<TagDefinition>> {
            // Write a list of tag definitions
            let tag_definitions = vec![
                TagDefinition { tag_names: "country".to_string(), tag_type: TagType::Text },
                TagDefinition { tag_names: "science".to_string(), tag_type: TagType::Int },
                TagDefinition { tag_names: "is_open".to_string(), tag_type: TagType::Bool },
            ];
            Ok(tag_definitions)
        }
    }


    /// Parse the SQL and return a list of base tables used in the query.
    ///
    /// Example:
    ///   SELECT ... FROM item i
    ///   JOIN tag_value tv ON ...
    /// returns ["item", "tag_value"]
    pub fn validate_my_engine_query(sql: &str) -> Result<Vec<String>, String> {
        let dialect = PostgreSqlDialect {};
        let r_statements = Parser::parse_sql(&dialect, sql)
            .map_err(|e| e.to_string());

        assert_eq!(false, r_statements.is_err());

        let statements = r_statements.unwrap();

        let mut tables = HashSet::new();
        for stmt in statements {
            collect_tables_from_statement(&stmt, &mut tables);
        }

        let mut list: Vec<String> = tables.into_iter().collect();
        list.sort();

        // ✅ Assert that we only got the expected tables
        assert_eq!(
            list,
            vec!["item".to_string(), "tag_definition".to_string(), "tag_value".to_string()]
        );

        Ok(list)
    }

    fn collect_tables_from_statement(stmt: &Statement, tables: &mut HashSet<String>) {
        match stmt {
            Statement::Query(q) => collect_tables_from_query(q, tables),
            // Add more if you want to support INSERT, UPDATE, DELETE, etc.
            _ => {}
        }
    }

    fn collect_tables_from_query(query: &Query, tables: &mut HashSet<String>) {
        match &*query.body {
            SetExpr::Select(s) => {
                for twj in &s.from {
                    collect_tables_from_table_with_joins(twj, tables);
                }
            }
            SetExpr::Query(q) => collect_tables_from_query(q, tables),
            _ => {}
        }
        // Also recurse into subqueries in ORDER BY, WITH, etc., if needed
    }

    fn collect_tables_from_table_with_joins(twj: &TableWithJoins, tables: &mut HashSet<String>) {
        collect_table_factor(&twj.relation, tables);
        for j in &twj.joins {
            collect_table_factor(&j.relation, tables);
        }
    }

    fn collect_table_factor(tf: &TableFactor, tables: &mut HashSet<String>) {
        match tf {
            TableFactor::Table { name, .. } => {
                let table = objectname_last_ident(name);
                tables.insert(table);
            }
            TableFactor::Derived { subquery, .. } => {
                collect_tables_from_query(subquery, tables);
            }
            _ => {}
        }
    }

    fn objectname_last_ident(name: &ObjectName) -> String {
        name.0
            .last()
            .map(|id| id.to_string())
            .unwrap_or_default()
    }


    #[tokio::test]
    pub async fn test_generate_search_sql_6_conditions() {
        init_logger();
        let input = " (country == \"US\" OR country == \"FR\"  AND  (science >= 50) OR (is_open == FALSE OR (country == \"LU\" AND science >=50) ) )";
        let filter_expression_ast = analyse_expression(input).unwrap();

        let tag_definition_builder = TagDefinitionBuilderMock {};

        let query = generate_search_sql(
            &filter_expression_ast,
            &tag_definition_builder,
            &vec!["country", "science", "is_open"],
            &vec!["country", "science", "is_open"],
            SearchSqlGenerationMode::Live,
        ).await;
        let q = &query.unwrap();
        // validate and assert table names
        let _r = validate_my_engine_query(q);
        log_info!("FINAL QUERY : {}", q);
    }

    struct TagDefinitionBuilderMock2 {}
    #[async_trait]
    impl TagDefinitionInterface for TagDefinitionBuilderMock2 {
        async fn get_tag_definition(&self, tag_name: &[String]) -> anyhow::Result<Vec<TagDefinition>> {
            // Write a list of tag definitions
            let tag_definitions = vec![
                TagDefinition { tag_names: "lastname".to_string(), tag_type: TagType::Text },
                TagDefinition { tag_names: "postal_code".to_string(), tag_type: TagType::Int },
            ];
            Ok(tag_definitions)
        }
    }

    ///
    /// -- CASE 7 lastname LIKE 'ab%' OR  (postal_code = 30099 AND lastname LIKE '%h%')
    ///
    #[tokio::test]
    pub async fn test_generate_search_sql_3_conditions() {
        init_logger();
        let input = r#"lastname LIKE "%ab%" OR (postal_code == 30099  AND  lastname LIKE "%h%")"#;

        let filter_expression_ast = analyse_expression(input).unwrap();

        let tag_definition_builder = TagDefinitionBuilderMock2 {};
        let query = generate_search_sql(
            &filter_expression_ast,
            &tag_definition_builder,
            &vec!["lastname", "postal_code"],
            &vec!["lastname", "postal_code"],
            SearchSqlGenerationMode::Live,
        ).await;

        let q = &query.unwrap();
        // validate and assert table names
        let _r = validate_my_engine_query(q);
        log_info!("FINAL QUERY : {}", q);
    }

    #[test]
    fn test_verify_filter_conditions() {
        // Initialize valid tag definitions
        let definitions = vec![
            TagDefinition { tag_names: "country".to_string(), tag_type: TagType::Text },
            TagDefinition { tag_names: "age".to_string(), tag_type: TagType::Int },
            TagDefinition { tag_names: "is_active".to_string(), tag_type: TagType::Bool },
        ];

        // Create valid filter conditions
        let mut filter_conditions = HashMap::new();
        filter_conditions.insert(
            "1".to_string(),
            (
                0,
                FilterCondition {
                    key: "1".to_string(),
                    attribute: "country".to_string(),
                    operator: ComparisonOperator::EQ,
                    value: FilterValue::ValueString("FR".to_string()),
                },
            ),
        );
        filter_conditions.insert(
            "2".to_string(),
            (
                0,
                FilterCondition {
                    key: "2".to_string(),
                    attribute: "age".to_string(),
                    operator: ComparisonOperator::GT,
                    value: FilterValue::ValueInt(18),
                },
            ),
        );

        // Verify valid conditions
        assert!(verify_filter_conditions(&filter_conditions, &definitions).is_ok());

        // Add an invalid filter condition (unsupported operator for Bool type)
        filter_conditions.insert(
            "3".to_string(),
            (
                0,
                FilterCondition {
                    key: "3".to_string(),
                    attribute: "is_active".to_string(),
                    operator: ComparisonOperator::GT, // Invalid for Bool
                    value: FilterValue::ValueBool(true),
                },
            ),
        );

        // Verify invalid conditions
        let result = verify_filter_conditions(&filter_conditions, &definitions);
        assert!(result.is_err());
        if let Err(GenerationError::TagIncompatibleType(err_msg)) = result {
            // dbg!(&err_msg);
            assert!(err_msg.contains("is_active"));
        } else {
            panic!("Expected TagIncompatibleType error");
        }
    }

    #[test]
    pub fn extract_conditions_1() {
        init_logger();
        let input1 = " (country == \"FR\"  AND  (science >= 50) OR (is_open == \"TRUE\" OR (country == \"LU\" AND science >=50) ) )";
        let tree1 = analyse_expression(input1).unwrap();
        // let _canonical1 = to_canonical_form(tree1.as_ref()).unwrap();
        let all_conditions = extract_all_conditions(tree1.as_ref()).unwrap();
        parser_log!("all_conditions...{:?}", all_conditions; 0);
        assert_eq!(5, all_conditions.values().len());
        let count_country = all_conditions.iter().filter(|(c, d)| &d.1.attribute == "country").count();
        let count_science = all_conditions.iter().filter(|(c, d)| &d.1.attribute == "science").count();
        let count_is_open = all_conditions.iter().filter(|(c, d)| &d.1.attribute == "is_open").count();
        assert_eq!(2, count_country);
        assert_eq!(2, count_science);
        assert_eq!(1, count_is_open);
    }

    #[test]
    pub fn extract_boolean_filter_1() {
        init_logger();
        let input1 = " (country == \"FR\"  AND  (science >= 50) OR (is_open == \"TRUE\" OR (country == \"LU\" AND science >=50) ) )";
        let tree1 = analyse_expression(input1).unwrap();
        // let canonical1 = to_canonical_form(tree1.as_ref()).unwrap();
        let all_conditions = extract_all_conditions(tree1.as_ref()).unwrap();
        let boolean_filter = build_query_filter(tree1.as_ref(), &all_conditions).unwrap();
        log_debug!("boolean filter: {}", &boolean_filter);

        const EXPECTED : &str = "(( ot_country_0.value is not null  AND  ot_science_0.value is not null ) OR ( ot_is_open_0.value is not null  OR ( ot_country_1.value is not null  AND  ot_science_1.value is not null )))";
        assert_eq!(EXPECTED, &boolean_filter);
    }
}
