use std::collections::HashMap;
use std::time::SystemTime;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde_json::to_string as to_json_string;

use commons_error::*;
use commons_pg::sql_transaction::{date_time_to_iso, naivedate_to_iso, CellValue, SQLDataSet};
use commons_pg::sql_transaction_async::{SQLChangeAsync, SQLQueryBlockAsync, SQLTransactionAsync};
use commons_services::x_request_id::Follower;
use dkdto::web_types::{BuildQueryConditionStat, BuildQueryRequest, EnumTagValue, ItemElement, TagType, TagValueElement};

use crate::engine::generator::TagDefinition;

pub(crate) struct SearchDao {
    follower: Follower,
}

impl SearchDao {
    pub(crate) fn new(follower: Follower) -> Self {
        Self { follower }
    }

    pub(crate) async fn search_item_from_query(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        sql_query: &str,
        start_page: Option<u32>,
        page_size: Option<u32>,
        select_tags: &[String],
        projected_definitions: &[TagDefinition],
    ) -> anyhow::Result<Vec<ItemElement>> {
        let params = HashMap::new();

        let query = SQLQueryBlockAsync {
            sql_query: sql_query.to_string(),
            start: start_page.unwrap_or(0) * page_size.unwrap_or(0),
            length: page_size,
            params,
        };

        let mut sql_result: SQLDataSet =
            query.execute(trans).await.map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

        let mut items = vec![];
        while sql_result.next() {
            let id: i64 = sql_result.get_int("id").ok_or(anyhow!("Wring id"))?;
            let name: String = sql_result.get_string("name").unwrap_or("".to_owned());
            let o_file_ref: Option<String> = sql_result.get_string("file_ref");
            let created_gmt = sql_result
                .get_timestamp_as_datetime("created_gmt")
                .ok_or(anyhow::anyhow!("Wrong created gmt"))
                .map_err(tr_fwd!())?;

            let last_modified_gmt =
                sql_result.get_timestamp_as_datetime("last_modified_gmt").as_ref().map(|x| date_time_to_iso(x));

            let properties = build_projected_properties(&sql_result, id, projected_definitions, select_tags);

            items.push(ItemElement {
                item_id: id,
                name,
                file_ref: o_file_ref,
                created: date_time_to_iso(&created_gmt),
                last_modified: last_modified_gmt,
                properties: Some(properties),
            });
        }

        Ok(items)
    }

    pub(crate) async fn execute_count_query(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        sql_query: &str,
    ) -> anyhow::Result<usize> {
        let query = SQLQueryBlockAsync { sql_query: sql_query.to_string(), start: 0, length: Some(1), params: HashMap::new() };

        let mut sql_result: SQLDataSet =
            query.execute(trans).await.map_err(err_fwd!("Count query failed, [{}]", &query.sql_query))?;

        if !sql_result.next() {
            return Ok(0);
        }

        let item_count: i64 = sql_result.get_int("value").ok_or(anyhow!("Wrong value count"))?;
        Ok(item_count as usize)
    }

    pub(crate) async fn persist_compiled_query(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        build_query_request: &BuildQueryRequest,
        compiled_sql: &str,
        matching_item_count: usize,
        condition_stats: &[BuildQueryConditionStat],
        customer_code: &str,
    ) -> anyhow::Result<i64> {
        let extra_properties_json = build_query_request
            .extra_properties
            .as_ref()
            .map(|properties| to_json_string(properties))
            .transpose()
            .map_err(err_fwd!("Cannot serialize extra_properties, follower=[{}]", &self.follower))?;

        let now = SystemTime::now();
        let compiled_query_id = match self.find_compiled_query_id(trans, &build_query_request.query_name, customer_code).await? {
            Some(query_id) => {
                self.update_compiled_query(
                    trans,
                    query_id,
                    build_query_request,
                    compiled_sql,
                    matching_item_count,
                    extra_properties_json.clone(),
                    now,
                    customer_code,
                )
                .await?;
                query_id
            }
            None => {
                self.insert_compiled_query(
                    trans,
                    build_query_request,
                    compiled_sql,
                    matching_item_count,
                    extra_properties_json,
                    now,
                    customer_code,
                )
                .await?
            }
        };

        self.delete_compiled_query_conditions(trans, compiled_query_id, customer_code).await?;
        self.insert_compiled_query_conditions(trans, compiled_query_id, condition_stats, now, customer_code).await?;

        Ok(compiled_query_id)
    }

    async fn find_compiled_query_id(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        query_name: &str,
        customer_code: &str,
    ) -> anyhow::Result<Option<i64>> {
        let sql_query = format!(
            r#"SELECT id
FROM cs_{}.compiled_query
WHERE query_name = :p_query_name"#,
            customer_code
        );

        let mut params = HashMap::new();
        params.insert("p_query_name".to_string(), CellValue::from_raw_string(query_name.to_string()));

        let query = SQLQueryBlockAsync { sql_query, start: 0, length: Some(1), params };
        let mut sql_result = query.execute(trans).await.map_err(err_fwd!(
            "Cannot find compiled query id, query_name=[{}], follower=[{}]",
            query_name,
            &self.follower
        ))?;

        Ok(if sql_result.next() { sql_result.get_int("id") } else { None })
    }

    async fn insert_compiled_query(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        build_query_request: &BuildQueryRequest,
        compiled_sql: &str,
        matching_item_count: usize,
        extra_properties_json: Option<String>,
        now: SystemTime,
        customer_code: &str,
    ) -> anyhow::Result<i64> {
        let sql_query = format!(
            r#"INSERT INTO cs_{}.compiled_query
                (query_name, description, filters, order_tag, extra_properties_json, compiled_sql, matching_item_count, created_gmt, last_modified_gmt)
              VALUES
                (:p_query_name, :p_description, :p_filters, :p_order_tag, :p_extra_properties_json, :p_compiled_sql, :p_matching_item_count, :p_created_gmt, :p_last_modified_gmt)"#,
            customer_code
        );

        let mut params = HashMap::new();
        params.insert("p_query_name".to_string(), CellValue::from_raw_string(build_query_request.query_name.clone()));
        params.insert("p_description".to_string(), CellValue::from_raw_string(build_query_request.description.clone()));
        params.insert(
            "p_filters".to_string(),
            CellValue::from_raw_string(build_query_request.filters.clone().unwrap_or_else(|| "()".to_string())),
        );
        params.insert("p_order_tag".to_string(), CellValue::String(build_query_request.order_tag.clone()));
        params.insert("p_extra_properties_json".to_string(), CellValue::String(extra_properties_json));
        params.insert("p_compiled_sql".to_string(), CellValue::from_raw_string(compiled_sql.to_string()));
        params.insert("p_matching_item_count".to_string(), CellValue::from_raw_int(matching_item_count as i64));
        params.insert("p_created_gmt".to_string(), CellValue::from_raw_systemtime(now));
        params.insert("p_last_modified_gmt".to_string(), CellValue::from_raw_systemtime(now));

        let sql_insert = SQLChangeAsync {
            sql_query,
            params,
            sequence_name: format!("cs_{}.compiled_query_id_seq", customer_code),
        };

        sql_insert
            .insert(trans)
            .await
            .map_err(err_fwd!("Cannot insert compiled query, follower=[{}]", &self.follower))
    }

    async fn update_compiled_query(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        compiled_query_id: i64,
        build_query_request: &BuildQueryRequest,
        compiled_sql: &str,
        matching_item_count: usize,
        extra_properties_json: Option<String>,
        now: SystemTime,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let sql_query = format!(
            r#"UPDATE cs_{}.compiled_query
SET description = :p_description,
    filters = :p_filters,
    order_tag = :p_order_tag,
    extra_properties_json = :p_extra_properties_json,
    compiled_sql = :p_compiled_sql,
    matching_item_count = :p_matching_item_count,
    last_modified_gmt = :p_last_modified_gmt
WHERE id = :p_compiled_query_id"#,
            customer_code
        );

        let mut params = HashMap::new();
        params.insert("p_description".to_string(), CellValue::from_raw_string(build_query_request.description.clone()));
        params.insert(
            "p_filters".to_string(),
            CellValue::from_raw_string(build_query_request.filters.clone().unwrap_or_else(|| "()".to_string())),
        );
        params.insert("p_order_tag".to_string(), CellValue::String(build_query_request.order_tag.clone()));
        params.insert("p_extra_properties_json".to_string(), CellValue::String(extra_properties_json));
        params.insert("p_compiled_sql".to_string(), CellValue::from_raw_string(compiled_sql.to_string()));
        params.insert("p_matching_item_count".to_string(), CellValue::from_raw_int(matching_item_count as i64));
        params.insert("p_last_modified_gmt".to_string(), CellValue::from_raw_systemtime(now));
        params.insert("p_compiled_query_id".to_string(), CellValue::from_raw_int(compiled_query_id));

        let query = SQLChangeAsync { sql_query, params, sequence_name: "".to_string() };
        query
            .update(trans)
            .await
            .map_err(err_fwd!("Cannot update compiled query, follower=[{}]", &self.follower))
    }

    async fn delete_compiled_query_conditions(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        compiled_query_id: i64,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let sql_query = format!(
            r#"DELETE FROM cs_{}.compiled_query_condition
WHERE compiled_query_id = :p_compiled_query_id"#,
            customer_code
        );

        let mut params = HashMap::new();
        params.insert("p_compiled_query_id".to_string(), CellValue::from_raw_int(compiled_query_id));

        let query = SQLChangeAsync { sql_query, params, sequence_name: "".to_string() };
        query
            .delete(trans)
            .await
            .map_err(err_fwd!("Cannot delete compiled query conditions, follower=[{}]", &self.follower))
    }

    async fn insert_compiled_query_conditions(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        compiled_query_id: i64,
        condition_stats: &[BuildQueryConditionStat],
        now: SystemTime,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let sql_query = format!(
            r#"INSERT INTO cs_{}.compiled_query_condition
                (compiled_query_id, condition_key, attribute_name, operator, value_text, occurrence, count_sql, matching_item_count, is_ic, superfilter_sql, created_gmt, last_modified_gmt)
              VALUES
                (:p_compiled_query_id, :p_condition_key, :p_attribute_name, :p_operator, :p_value_text, :p_occurrence, :p_count_sql, :p_matching_item_count, :p_is_ic, :p_superfilter_sql, :p_created_gmt, :p_last_modified_gmt)"#,
            customer_code
        );

        let sequence_name = format!("cs_{}.compiled_query_condition_id_seq", customer_code);

        for condition_stat in condition_stats {
            let mut params = HashMap::new();
            params.insert("p_compiled_query_id".to_string(), CellValue::from_raw_int(compiled_query_id));
            params.insert("p_condition_key".to_string(), CellValue::from_raw_string(condition_stat.condition_key.clone()));
            params.insert("p_attribute_name".to_string(), CellValue::from_raw_string(condition_stat.attribute.clone()));
            params.insert("p_operator".to_string(), CellValue::from_raw_string(condition_stat.operator.clone()));
            params.insert("p_value_text".to_string(), CellValue::from_raw_string(condition_stat.value.clone()));
            params.insert("p_occurrence".to_string(), CellValue::from_raw_int_32(condition_stat.occurrence as i32));
            params.insert("p_count_sql".to_string(), CellValue::from_raw_string(condition_stat.count_query.clone()));
            params.insert(
                "p_matching_item_count".to_string(),
                CellValue::from_raw_int(condition_stat.matching_item_count as i64),
            );
            params.insert("p_is_ic".to_string(), CellValue::Bool(None));
            params.insert("p_superfilter_sql".to_string(), CellValue::String(None));
            params.insert("p_created_gmt".to_string(), CellValue::from_raw_systemtime(now));
            params.insert("p_last_modified_gmt".to_string(), CellValue::from_raw_systemtime(now));

            let sql_insert = SQLChangeAsync { sql_query: sql_query.clone(), params, sequence_name: sequence_name.clone() };
            sql_insert
                .insert(trans)
                .await
                .map_err(err_fwd!("Cannot insert compiled query condition, follower=[{}]", &self.follower))?;
        }

        Ok(())
    }
}

fn enum_tag_value_from_cell(tag_type: &TagType, cell: Option<&CellValue>) -> EnumTagValue {
    match tag_type {
        TagType::Text => EnumTagValue::Text(cell.and_then(CellValue::inner_value_string)),
        TagType::Link => EnumTagValue::Link(cell.and_then(CellValue::inner_value_string)),
        TagType::Bool => EnumTagValue::Boolean(cell.and_then(CellValue::inner_value_bool)),
        TagType::Int => EnumTagValue::Integer(cell.and_then(CellValue::inner_value_int)),
        TagType::Double => EnumTagValue::Double(cell.and_then(CellValue::inner_value_double)),
        TagType::Date => {
            let opt_iso_d_str = cell.and_then(CellValue::inner_value_naivedate).as_ref().map(naivedate_to_iso);
            EnumTagValue::SimpleDate(opt_iso_d_str)
        }
        TagType::DateTime => {
            let opt_iso_dt_str = cell
                .and_then(CellValue::inner_value_systemtime)
                .map(|system_time| {
                    let dt: DateTime<Utc> = system_time.into();
                    dt
                })
                .as_ref()
                .map(date_time_to_iso);
            EnumTagValue::DateTime(opt_iso_dt_str)
        }
    }
}

fn build_projected_properties(
    sql_result: &SQLDataSet,
    item_id: i64,
    definitions: &[TagDefinition],
    select_tags: &[String],
) -> Vec<TagValueElement> {
    let mut properties = vec![];

    for tag_name in select_tags {
        let Some(definition) = definitions.iter().find(|definition| definition.tag_names == *tag_name) else {
            continue;
        };

        let value = enum_tag_value_from_cell(&definition.tag_type, sql_result.get_cell(tag_name));

        properties.push(TagValueElement { tag_value_id: 0, item_id, tag_id: 0, tag_name: tag_name.clone(), value });
    }

    properties
}
