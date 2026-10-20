use crate::filter::filter_ast::{ComparisonOperator, FilterCondition, FilterExpressionAST, FilterValue};
use crate::filter::to_sql_literal;
use crate::filter::type_resolver::{FilterResolutionError, collect_attribute_names, resolve_filter_value_types};
use axum::async_trait;
use commons_error::tr_fwd;
use commons_error::*;
use commons_pg::sql_transaction_async::SQLConnectionAsync;
use commons_services::x_request_id::Follower;
use dkdto::web_types::TagType;
use log::*;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

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
    map.insert(
        TagType::Date,
        vec![
            ComparisonOperator::EQ,
            ComparisonOperator::NEQ,
            ComparisonOperator::GT,
            ComparisonOperator::GTE,
            ComparisonOperator::LT,
            ComparisonOperator::LTE,
        ],
    );
    map
});

pub(crate) enum SearchSqlGenerationMode {
    Live,
    Persisted,
}

#[derive(Debug, Clone)]
pub(crate) struct ConditionStatQuery {
    pub(crate) condition_key: String,
    pub(crate) attribute: String,
    pub(crate) operator: String,
    pub(crate) value_repr: String,
    pub(crate) occurrence: u32,
    pub(crate) count_sql: String,
    pub(crate) benchmark_sql: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledSearchQuery {
    pub(crate) main_sql: String,
    pub(crate) condition_stats: Vec<ConditionStatQuery>,
}

#[derive(Debug)]
pub(crate) struct TagDefinition {
    pub(crate) tag_names: String,
    pub(crate) tag_type: TagType,
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
            .filter(|(_, filter_condition)| &filter_condition.attribute == &fc.attribute)
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
        FilterExpressionAST::Logical { operator: _, leaves } => {
            for l in leaves {
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
        FilterExpressionAST::Condition(FilterCondition { key, attribute: _, operator: _, value: _ }) => {
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
            // F0004 rule 3: an empty Logical is the sentinel "match-all"
            // AST built by the delegate when no effective filter is
            // provided. Emit `TRUE` so the surrounding `WHERE\n    <fragment>`
            // template produces valid SQL.
            if leaves.is_empty() {
                content.push_str("TRUE");
            } else {
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
    }
    Ok(content)
}

#[derive(Debug)]
pub(crate) enum GenerationError {
    TagUnknown(String),
    TagTypeUnknown(String),
    TagSearchError(String),
    TagIncompatibleType(String),
    InvalidDateLiteral { tag: String, value: String },
    IncompatibleValueShape { tag: String, tag_type: TagType, got: &'static str },
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

use crate::tag::TagDelegate;
use anyhow::Result;
use commons_services::token_lib::SessionToken;

#[derive(Debug)]
pub(crate) struct TagDefinitionBuilder {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl TagDefinitionBuilder {
    pub fn new(session_token: SessionToken, follower: Follower) -> Self {
        Self { session_token, follower }
    }
}

#[async_trait]
pub(crate) trait TagDefinitionInterface {
    /// Returns the definition of the given tags for the given customer
    async fn get_tag_definition(&self, tag_names: &[String], customer_code: &str) -> Result<Vec<TagDefinition>>;
}

#[async_trait]
impl TagDefinitionInterface for TagDefinitionBuilder {
    async fn get_tag_definition(&self, tag_names: &[String], customer_code: &str) -> Result<Vec<TagDefinition>> {
        let mut cnx = SQLConnectionAsync::from_pool()
            .await
            .map_err(err_fwd!("New DB connection failed; follower={}", &self.follower))?;

        let mut trans = cnx.begin().await.map_err(err_fwd!("Transaction issue; follower={}", &self.follower))?;

        let tag_delegate = TagDelegate::new(self.session_token.clone(), self.follower.x_request_id.clone());

        let r_defs = tag_delegate
            .search_tags_by_names(&mut trans, tag_names, customer_code)
            .await
            .map_err(err_fwd!("search_tags_by_names failed; follower={}", &self.follower));

        trans.commit().await?;

        // Convert to TagDefiniton and return the list
        r_defs.map(|defs| {
            defs.into_iter()
                .map(|def| TagDefinition {
                    tag_names: def.name,
                    tag_type: TagType::from_str(&def.tag_type).unwrap_or(TagType::Text),
                })
                .collect()
        })
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
            // F0003: route the value literal through `to_sql_literal` (F0002),
            // then wrap the quoted body in `unaccent_lower(...)`, keeping any
            // trailing ` ESCAPE '\'` clause OUTSIDE the wrapping — the
            // ESCAPE clause attaches to the LIKE, not to the function call.
            //
            // Examples:
            //   to_sql_literal -> `'den%'`              => unaccent_lower('den%')
            //   to_sql_literal -> `'50\%' ESCAPE '\'`   => unaccent_lower('50\%') ESCAPE '\'
            let literal = to_sql_literal(&filter_condition.value, &filter_condition.operator);
            let (wrapped_body, escape_suffix) = match literal.find(" ESCAPE '") {
                Some(pos) => (&literal[..pos], &literal[pos..]),
                None => (literal.as_str(), ""),
            };
            format!(
                "unaccent_lower((tv.value_string)::text) {0} unaccent_lower({1}){2}",
                &sql_op, wrapped_body, escape_suffix
            )
        }
        TagType::Bool => match (&filter_condition.operator, &filter_condition.value) {
            (ComparisonOperator::EQ, FilterValue::ValueBool(true)) => "tv.value_boolean = TRUE".to_string(),
            (ComparisonOperator::EQ, FilterValue::ValueBool(false)) => "tv.value_boolean = FALSE".to_string(),
            (ComparisonOperator::NEQ, FilterValue::ValueBool(true)) => "tv.value_boolean <> TRUE".to_string(),
            (ComparisonOperator::NEQ, FilterValue::ValueBool(false)) => "tv.value_boolean <> FALSE".to_string(),
            _ => {
                return Err(GenerationError::TagIncompatibleType(format!(
                    "Tag : {}, Invalid boolean comparison {:?} with value {}",
                    &filter_condition.attribute, &filter_condition.operator, &filter_condition.value
                )));
            }
        },
        TagType::Int => {
            format!("tv.value_integer {0} {1}", &sql_op, &filter_condition.value)
        }
        TagType::Double => {
            format!("tv.value_double {0} {1}", &sql_op, &filter_condition.value)
        }
        TagType::Date => match &filter_condition.value {
            FilterValue::ValueDate(d) => format!("tv.value_date {} DATE '{}'", sql_op, d),
            other => {
                return Err(GenerationError::TagIncompatibleType(format!(
                    "Tag : {}, expected date literal, got {:?}",
                    filter_condition.attribute, other
                )));
            }
        },
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
            /* TODO we must also check the value format depending on the tag type here */
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

fn build_order_column(
    order_tags: &[String],
    map_of_tags_with_occurrence: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    order_tags
        .iter()
        .filter_map(|tag| map_of_tags_with_occurrence.get(tag))
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

/// Not used for now
///
/// Allow to add some tag column in the search query
fn build_tag_column_with_alias(
    select_tags: &[String],
    map_of_tags_with_occurrence: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    select_tags
        .iter()
        .filter_map(|tag| map_of_tags_with_occurrence.get(tag).map(|occurrences| (tag, occurrences)))
        .map(|(tag, occurrences)| {
            if occurrences.len() == 1 {
                format!("{}.value AS {}", occurrences[0], tag)
            } else if occurrences.len() == 2 {
                format!("COALESCE({}.value, {}.value) AS {}", occurrences[0], occurrences[1], tag)
            } else {
                let mut expr = format!("COALESCE({}.value, {}.value)", occurrences[0], occurrences[1]);
                for o in &occurrences[2..] {
                    expr = format!("COALESCE({}, {}.value)", expr, o);
                }
                format!("{} AS {}", expr, tag)
            }
        })
        .collect()
}

/// Resolve phase of the search-query pipeline.
///
/// Steps: `collect_attribute_names` → load `TagDefinition`s for filter +
/// order tags → verify every referenced tag exists → `resolve_filter_value_types`
/// (rewrites `ValueString` → `ValueDate` for `Date` tags).
///
/// Returns the typed AST and the owned definitions vector. Callers feed
/// the pair into `generate_sql_from_resolved`.
///
/// REF_TAG : DOKA_SEARCH_SQL
pub(crate) async fn resolve_filter<T: TagDefinitionInterface>(
    filter_expression_ast: FilterExpressionAST,
    tag_definition_builder: &T,
    order_tags: &[String],
    customer_code: &str,
) -> Result<(FilterExpressionAST, Vec<TagDefinition>), GenerationError> {
    let mut tags: HashSet<String> = collect_attribute_names(&filter_expression_ast);
    tags.extend(order_tags.iter().cloned());

    dbg!(&tags);

    // Find the tag_definitions for the tags used in the filter and in the order by.
    // F0004: when both the filter AST and order_tags are empty (the "match-all"
    // case with no ordering), skip the lookup — `search_tags_by_names` builds
    // an invalid `WHERE name IN ` when given an empty list.
    let tags_list: Vec<String> = tags.iter().cloned().collect();

    let definitions = if tags_list.is_empty() {
        vec![]
    } else {
        match tag_definition_builder.get_tag_definition(&tags_list, customer_code).await.map_err(tr_fwd!()) {
            Ok(definitions) => definitions,
            Err(e) => {
                log_error!("Error while getting tag definitions: {:?}", e);
                return Err(GenerationError::TagSearchError("Error in tag search".to_string()));
            }
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

    // F0005: type-resolve the AST (rewrites ValueString → ValueDate for Date tags)
    // before any further analysis. From here on, the AST is typed.
    let resolved = resolve_filter_value_types(filter_expression_ast, definitions).map_err(|e| match e {
        FilterResolutionError::InvalidDateLiteral { tag, value } => GenerationError::InvalidDateLiteral { tag, value },
        FilterResolutionError::IncompatibleValueShape { tag, tag_type, got } => {
            GenerationError::IncompatibleValueShape { tag, tag_type, got }
        }
    })?;

    Ok((resolved.ast, resolved.definitions))
}

/// 🔑 SQL-generation phase of the search-query pipeline.
///
/// Consumes the typed AST and definitions produced by `resolve_filter`,
/// builds the per-condition `LEFT OUTER JOIN`s (plus synthetic joins for
/// order-only tags), the boolean filter, the ORDER BY columns, and
/// assembles the final `SELECT` together with per-condition stat queries.
///
///     REF_TAG : DOKA_SEARCH_SQL
pub(crate) async fn generate_sql_from_resolved(
    filter_expression_ast: FilterExpressionAST,
    definitions: Vec<TagDefinition>,
    select_tags: &[String],
    order_tags: &[String],
    _generation_mode: SearchSqlGenerationMode,
    customer_code: &str,
) -> Result<CompiledSearchQuery, GenerationError> {
    // Extract all conditions from the resolved (typed) AST.
    let filter_conditions = extract_all_conditions(&filter_expression_ast).map_err(tr_fwd!())?;

    dbg!(&filter_conditions);

    // Verify if the filter conditions are compatible with the tag type
    if let Err(e) = verify_filter_conditions(&filter_conditions, &definitions) {
        log_error!("Error while verifying filter conditions: {:?}", e);
        return Err(e);
    }

    let mut map_of_tags_with_occurrence: HashMap<String, Vec<String>> = HashMap::new();

    // Generate the {{tag_value_filter}} for all tags condition
    let mut list_of_query_tags: Vec<String> = vec![];
    let mut condition_stats: Vec<ConditionStatQuery> = vec![];
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

        // Add the stats related to the filter condition
        condition_stats.push(build_condition_stat_query(fc, tag_type, *occurrence, &tag_value_filter));
    }

    // F0004 rule 4: order_tags that are not already covered by a filter
    // condition need a synthetic LEFT OUTER JOIN with `AND TRUE`, so the
    // `ot_<tag>_0.value` reference in the ORDER BY clause resolves.
    for order_tag in order_tags.iter() {
        if map_of_tags_with_occurrence.contains_key(order_tag) {
            continue;
        }
        let tag_type = definitions.iter().find(|def| &def.tag_names == order_tag).map(|def| &def.tag_type).unwrap();

        let synthetic_join = QUERY_FILTER_TEMPLATE
            .replace("{{value_column_name}}", tag_type.value_column_name())
            .replace("{{tag_name}}", order_tag)
            .replace("{{tag_value_filter}}", "AND TRUE")
            .replace("{{tag_super_filter}}", "")
            .replace("{{occurence}}", "0");

        let tag_occurrence = format!("ot_{}_{}", order_tag, 0);
        map_of_tags_with_occurrence.entry(order_tag.clone()).or_insert_with(Vec::new).push(tag_occurrence);
        list_of_query_tags.push(synthetic_join);
    }

    // build the query filter aka boolean filter
    let query_filter = build_query_filter(&filter_expression_ast, &filter_conditions).map_err(tr_fwd!())?;

    dbg!(&query_filter);

    // build the order columns
    let order_columns = build_order_column(order_tags, &map_of_tags_with_occurrence).join(",\n    ");

    dbg!(&order_columns);

    let tag_columns = build_tag_column_with_alias(select_tags, &map_of_tags_with_occurrence).join(",\n    ");

    // Build the final SQL

    let mut final_sql = String::from(
        r#"SELECT i.id,
    name,
    file_ref,
    created_gmt,
    last_modified_gmt"#,
    );

    if !tag_columns.is_empty() {
        final_sql.push_str(",\n    ");
        final_sql.push_str(&tag_columns);
    }

    final_sql.push_str(" FROM {customer_schema}.item i ");

    final_sql.push_str("\n");
    final_sql.push_str(&list_of_query_tags.join("\n"));

    final_sql.push_str("\n");
    final_sql.push_str(" WHERE ");

    final_sql.push_str("\n    ");
    final_sql.push_str(query_filter.as_str());

    if !order_columns.is_empty() {
        final_sql.push_str("\n");
        final_sql.push_str(" ORDER BY ");

        final_sql.push_str("\n    ");
        final_sql.push_str(order_columns.as_str());
        final_sql.push_str("\n");
    }

    let sql_query = final_sql.to_string().replace("{customer_schema}", format!("cs_{}", customer_code).as_str());
    for condition_stat in &mut condition_stats {
        condition_stat.count_sql =
            condition_stat.count_sql.replace("{customer_schema}", format!("cs_{}", customer_code).as_str());
        condition_stat.benchmark_sql =
            condition_stat.benchmark_sql.replace("{customer_schema}", format!("cs_{}", customer_code).as_str());
    }

    Ok(CompiledSearchQuery { main_sql: sql_query, condition_stats })
}

pub(crate) async fn generate_search_sql<T: TagDefinitionInterface>(
    filter_expression_ast: FilterExpressionAST,
    tag_definition_builder: &T,
    select_tags: &[String],
    order_tags: &Vec<String>,
    generation_mode: SearchSqlGenerationMode,
    customer_code: &str,
) -> Result<String, GenerationError> {
    let (resolved_ast, definitions) =
        resolve_filter(filter_expression_ast, tag_definition_builder, order_tags, customer_code).await?;
    generate_sql_from_resolved(resolved_ast, definitions, select_tags, order_tags, generation_mode, customer_code)
        .await
        .map(|compiled| compiled.main_sql)
}

/// tag_value_filter and tag_super_filter are side by side to avoid a blank line
/// in the case of there isn't any tag_super_filter
const QUERY_FILTER_TEMPLATE: &str = r#"LEFT OUTER JOIN (
    SELECT tv.item_id, tv.{{value_column_name}} as value
    FROM {customer_schema}.tag_definition td
    JOIN {customer_schema}.tag_value tv ON
        tv.tag_id = td.id
        AND td."name" = '{{tag_name}}'
        {{tag_value_filter}}{{tag_super_filter}}
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

fn build_condition_stat_query(
    filter_condition: &FilterCondition,
    tag_type: &TagType,
    occurrence: u32,
    tag_value_filter: &str,
) -> ConditionStatQuery {
    let count_sql = format!(
        r#"SELECT count(*) as value
FROM {{customer_schema}}.tag_definition td
JOIN {{customer_schema}}.tag_value tv ON
    tv.tag_id = td.id
    AND td."name" = '{}'
    AND {}"#,
        &filter_condition.attribute, tag_value_filter
    );

    let benchmark_sql = format!(
        r#"SELECT tv.{} as value
FROM {{customer_schema}}.tag_definition td
JOIN {{customer_schema}}.tag_value tv ON
    tv.tag_id = td.id
    AND td."name" = '{}'
    AND {}"#,
        tag_type.value_column_name(),
        &filter_condition.attribute,
        tag_value_filter
    );

    ConditionStatQuery {
        condition_key: filter_condition.key.clone(),
        attribute: filter_condition.attribute.clone(),
        operator: format!("{:?}", filter_condition.operator),
        value_repr: filter_condition.value.to_string(),
        occurrence,
        count_sql,
        benchmark_sql,
    }
}

#[cfg(test)]
mod tests {

    // cargo test --color=always --bin document-server engine  [ -- --show-output]

    use crate::engine::generator::{
        GenerationError, SearchSqlGenerationMode, TagDefinition, TagDefinitionInterface, build_query_filter,
        build_tag_value_filter, extract_all_conditions, generate_search_sql, generate_sql_from_resolved,
        resolve_filter, verify_filter_conditions,
    };
    use crate::filter::analyse_expression;
    use crate::filter::filter_ast::{ComparisonOperator, FilterCondition, FilterValue};
    use crate::parser_log;
    use axum::async_trait;
    use commons_error::*;
    use dkdto::web_types::TagType;
    use log::*;
    use sqlparser::ast::{ObjectName, Query, SetExpr, Statement, TableFactor, TableWithJoins};
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::sync::Once;

    static INIT_LOGGER: Once = Once::new();

    pub(crate) fn init_logger() {
        INIT_LOGGER.call_once(|| {
            if let Err(e) = log4rs::init_file(
                "/home/denis/Projects/wks-doka-one/doka.one/document-server/log4rs.yaml",
                // r#"C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\log4rs.yaml"#,
                Default::default(),
            ) {
                eprintln!("logger init skipped: {:?}", e);
            }
        });
    }

    struct TagDefinitionBuilderMock {}

    #[async_trait]
    impl TagDefinitionInterface for TagDefinitionBuilderMock {
        async fn get_tag_definition(
            &self,
            _tag_name: &[String],
            _customer_code: &str,
        ) -> anyhow::Result<Vec<TagDefinition>> {
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
        let r_statements = Parser::parse_sql(&dialect, sql).map_err(|e| e.to_string());

        assert_eq!(false, r_statements.is_err());

        let statements = r_statements.unwrap();

        let mut tables = HashSet::new();
        for stmt in statements {
            collect_tables_from_statement(&stmt, &mut tables);
        }

        let mut list: Vec<String> = tables.into_iter().collect();
        list.sort();

        // ✅ Assert that we only got the expected tables
        assert_eq!(list, vec!["item".to_string(), "tag_definition".to_string(), "tag_value".to_string()]);

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
        name.0.last().map(|id| id.to_string()).unwrap_or_default()
    }

    #[tokio::test]
    pub async fn test_generate_search_sql_6_conditions() {
        init_logger();
        let input = " (country == \"US\" OR country == \"FR\"  AND  (science >= 50) OR (is_open == FALSE OR (country == \"LU\" AND science >=50) ) )";
        let filter_expression_ast = analyse_expression(input).unwrap();

        let tag_definition_builder = TagDefinitionBuilderMock {};

        let query = generate_search_sql(
            *filter_expression_ast,
            &tag_definition_builder,
            &vec!["country".to_string(), "science".to_string(), "is_open".to_string()],
            &vec!["country".to_string(), "science".to_string(), "is_open".to_string()],
            SearchSqlGenerationMode::Live,
            "cs_123456",
        )
        .await;
        let q = &query.unwrap();
        // validate and assert table names
        let _r = validate_my_engine_query(q);
        log_info!("FINAL QUERY : {}", q);
    }

    struct TagDefinitionBuilderMock2 {}
    #[async_trait]
    impl TagDefinitionInterface for TagDefinitionBuilderMock2 {
        async fn get_tag_definition(
            &self,
            _tag_name: &[String],
            _customer_code: &str,
        ) -> anyhow::Result<Vec<TagDefinition>> {
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
            *filter_expression_ast,
            &tag_definition_builder,
            &vec!["".to_string()],
            &vec!["lastname".to_string(), "postal_code".to_string()],
            SearchSqlGenerationMode::Live,
            "cs_123456",
        )
        .await;

        let q = &query.unwrap();

        // validate and assert table names
        let _r = validate_my_engine_query(q);
    }

    #[tokio::test]
    pub async fn test_compile_search_query_builds_condition_stats() {
        init_logger();
        let input = r#"lastname LIKE "%ab%" OR (postal_code == 30099  AND  lastname LIKE "%h%")"#;
        let filter_expression_ast = analyse_expression(input).unwrap();
        let tag_definition_builder = TagDefinitionBuilderMock2 {};

        let (resolved_ast, definitions) = resolve_filter(
            *filter_expression_ast,
            &tag_definition_builder,
            &vec!["lastname".to_string(), "postal_code".to_string()],
            "123456",
        )
        .await
        .unwrap();

        let compiled = generate_sql_from_resolved(
            resolved_ast,
            definitions,
            &vec!["lastname".to_string()],
            &vec!["lastname".to_string(), "postal_code".to_string()],
            SearchSqlGenerationMode::Persisted,
            "123456",
        )
        .await
        .unwrap();

        assert!(compiled.main_sql.contains("FROM cs_123456.item i"));
        assert_eq!(3, compiled.condition_stats.len());
        assert_eq!(2, compiled.condition_stats.iter().filter(|stat| stat.attribute == "lastname").count());
        assert_eq!(1, compiled.condition_stats.iter().filter(|stat| stat.attribute == "postal_code").count());
        assert!(compiled.condition_stats.iter().all(|stat| stat.count_sql.contains("SELECT count(*) as value")
            && stat.count_sql.contains("cs_123456.tag_value")));
    }

    #[tokio::test]
    pub async fn test_compile_search_query_single_leaf_builds_one_count_query() {
        init_logger();
        let filter_expression_ast = analyse_expression(r#"(country == "FR")"#).unwrap();
        let tag_definition_builder = TagDefinitionBuilderMock {};

        let (resolved_ast, definitions) =
            resolve_filter(*filter_expression_ast, &tag_definition_builder, &vec![], "123456").await.unwrap();

        let compiled = generate_sql_from_resolved(
            resolved_ast,
            definitions,
            &vec!["country".to_string()],
            &vec![],
            SearchSqlGenerationMode::Persisted,
            "123456",
        )
        .await
        .unwrap();

        assert_eq!(1, compiled.condition_stats.len());
        assert_eq!("country", compiled.condition_stats[0].attribute);
        assert!(compiled.condition_stats[0].count_sql.contains(r#"td."name" = 'country'"#));
        assert!(compiled.condition_stats[0].count_sql.contains("unaccent_lower('FR')"));
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
        let count_country = all_conditions.iter().filter(|(_, d)| &d.1.attribute == "country").count();
        let count_science = all_conditions.iter().filter(|(_, d)| &d.1.attribute == "science").count();
        let count_is_open = all_conditions.iter().filter(|(_, d)| &d.1.attribute == "is_open").count();
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

        const EXPECTED: &str = "(( ot_country_0.value is not null  AND  ot_science_0.value is not null ) OR ( ot_is_open_0.value is not null  OR ( ot_country_1.value is not null  AND  ot_science_1.value is not null )))";
        assert_eq!(EXPECTED, &boolean_filter);
    }

    #[test]
    pub fn test_build_tag_value_filter_bool_false_eq() {
        let filter_condition = FilterCondition {
            key: "1".to_string(),
            attribute: "is_open".to_string(),
            operator: ComparisonOperator::EQ,
            value: FilterValue::ValueBool(false),
        };

        let sql = build_tag_value_filter(&filter_condition, &TagType::Bool).unwrap();

        assert_eq!("tv.value_boolean = FALSE", sql);
    }

    #[test]
    pub fn test_build_tag_value_filter_bool_true_eq() {
        let filter_condition = FilterCondition {
            key: "1".to_string(),
            attribute: "is_open".to_string(),
            operator: ComparisonOperator::EQ,
            value: FilterValue::ValueBool(true),
        };

        let sql = build_tag_value_filter(&filter_condition, &TagType::Bool).unwrap();

        assert_eq!("tv.value_boolean = TRUE", sql);
    }

    #[test]
    pub fn test_build_tag_value_filter_bool_true_neq() {
        let filter_condition = FilterCondition {
            key: "1".to_string(),
            attribute: "is_open".to_string(),
            operator: ComparisonOperator::NEQ,
            value: FilterValue::ValueBool(true),
        };

        let sql = build_tag_value_filter(&filter_condition, &TagType::Bool).unwrap();

        assert_eq!("tv.value_boolean <> TRUE", sql);
    }

    #[test]
    pub fn test_build_tag_value_filter_bool_false_neq() {
        let filter_condition = FilterCondition {
            key: "1".to_string(),
            attribute: "is_open".to_string(),
            operator: ComparisonOperator::NEQ,
            value: FilterValue::ValueBool(false),
        };

        let sql = build_tag_value_filter(&filter_condition, &TagType::Bool).unwrap();

        assert_eq!("tv.value_boolean <> FALSE", sql);
    }

    // === UT-F0005 : Date filter support ===

    // TC-F0005-007 — `build_tag_value_filter` emits `DATE 'YYYY-MM-DD'` for EQ
    #[test]
    pub fn ut_f0005_007_build_tag_value_filter_date_eq() {
        use chrono::NaiveDate;
        let fc = FilterCondition {
            key: "1".to_string(),
            attribute: "birthdate".to_string(),
            operator: ComparisonOperator::EQ,
            value: FilterValue::ValueDate(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()),
        };

        let sql = build_tag_value_filter(&fc, &TagType::Date).unwrap();

        assert_eq!("tv.value_date = DATE '2025-12-31'", sql);
    }

    // TC-F0005-008 — All ordered operators emit the right symbol
    #[test]
    pub fn ut_f0005_008_build_tag_value_filter_date_all_operators() {
        use chrono::NaiveDate;
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let cases = [
            (ComparisonOperator::NEQ, "<>"),
            (ComparisonOperator::GT, ">"),
            (ComparisonOperator::GTE, ">="),
            (ComparisonOperator::LT, "<"),
            (ComparisonOperator::LTE, "<="),
        ];

        for (op, sym) in cases {
            let fc = FilterCondition {
                key: "1".to_string(),
                attribute: "birthdate".to_string(),
                operator: op.clone(),
                value: FilterValue::ValueDate(date),
            };
            let sql = build_tag_value_filter(&fc, &TagType::Date).unwrap();
            assert_eq!(format!("tv.value_date {} DATE '2025-12-31'", sym), sql, "op={:?}", op);
        }
    }

    // TC-F0005-009 — Defensive arm rejects non-`ValueDate` values
    #[test]
    pub fn ut_f0005_009_build_tag_value_filter_date_defensive() {
        let fc = FilterCondition {
            key: "1".to_string(),
            attribute: "birthdate".to_string(),
            operator: ComparisonOperator::EQ,
            value: FilterValue::ValueString("2025-12-31".to_string()),
        };

        match build_tag_value_filter(&fc, &TagType::Date) {
            Err(GenerationError::TagIncompatibleType(msg)) => {
                assert!(msg.contains("birthdate"), "expected attribute name in error, got: {}", msg);
            }
            other => panic!("Expected TagIncompatibleType, got {:?}", other),
        }
    }

    // TC-F0005-010 — `LIKE` is rejected on a Date tag at the operator-check stage
    #[test]
    pub fn ut_f0005_010_verify_filter_conditions_rejects_like_on_date() {
        use crate::filter::filter_lexer::PatternPart;
        let definitions = vec![TagDefinition { tag_names: "birthdate".to_string(), tag_type: TagType::Date }];

        let mut filter_conditions = HashMap::new();
        filter_conditions.insert(
            "1".to_string(),
            (
                0,
                FilterCondition {
                    key: "1".to_string(),
                    attribute: "birthdate".to_string(),
                    operator: ComparisonOperator::LIKE,
                    value: FilterValue::ValuePattern(vec![PatternPart::Literal("x".to_string())]),
                },
            ),
        );

        match verify_filter_conditions(&filter_conditions, &definitions) {
            Err(GenerationError::TagIncompatibleType(_)) => {}
            other => panic!("Expected TagIncompatibleType, got {:?}", other),
        }
    }
}
