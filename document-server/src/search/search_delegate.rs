use axum::http::StatusCode;
use log::{error, info};
use serde::de::DeserializeOwned;

use commons_error::*;
use commons_pg::sql_transaction_async::SQLConnectionAsync;
use commons_services::session_lib::valid_sid_get_session;
use commons_services::token_lib::SessionToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkdto::api_error::ApiError;
use dkdto::error_codes::INTERNAL_DATABASE_ERROR;
use dkdto::web_types::{
    BuildQueryConditionStat, BuildQueryReply, BuildQueryRequest, ContextMessage, GetItemReply, IntoWebTypeWithContext,
    SimpleMessage, WebType, WebTypeBuilder, WebTypeWithContext,
};
use doka_cli::request_client::TokenType;

use crate::engine::generator::{
    GenerationError, SearchSqlGenerationMode, TagDefinition, TagDefinitionBuilder, TagDefinitionInterface,
    compile_search_query, generate_search_sql,
};
use crate::filter::analyse_expression;
use crate::filter::filter_ast::FilterExpressionAST;
use crate::filter::filter_lexer::FilterError;
use crate::search::dao::SearchDao;

pub(crate) struct SearchDelegate {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl SearchDelegate {
    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower { x_request_id: x_request_id.new_if_null(), token_type: TokenType::None },
        }
    }

    pub async fn search_item(
        mut self,
        start_page: Option<u32>,
        page_size: Option<u32>,
        filter_expression: Option<String>,
        order_tags: Option<Vec<String>>,
    ) -> WebTypeWithContext<GetItemReply> {
        log_info!(
            "🚀 Start search_item api, start_page=[{:?}], page_size=[{:?}], follower=[{}]",
            start_page,
            page_size,
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error_ctx()
        );

        log_info!("😎 We fetched the session, follower=[{}]", &self.follower);

        let filter_expression_ast: Box<FilterExpressionAST> =
            try_or_return!(analyse_expression(filter_expression.as_deref().unwrap_or("()")), |e: FilterError| {
                let c = ContextMessage { message: e.human_error_message(), context: vec![e.char_position.to_string()] };
                WebTypeWithContext::from_simple(StatusCode::BAD_REQUEST.as_u16(), c)
            });

        let tag_definition_builder = TagDefinitionBuilder::new(self.session_token.clone(), self.follower.clone());
        let select_tags = build_select_tags(&filter_expression_ast, &order_tags.clone().unwrap_or_default());

        let sql_query = try_or_return!(
            generate_search_sql(
                &filter_expression_ast,
                &tag_definition_builder,
                &select_tags,
                &order_tags.unwrap_or(vec![]),
                SearchSqlGenerationMode::Live,
                &entry_session.customer_code,
            )
            .await,
            |e: GenerationError| {
                log_error!("💣 Fail to generate sql search query, [{:?}],follower=[{}]", e, &self.follower);
                let msg = SimpleMessage::from(e.to_string());
                WebType::from_simple(StatusCode::BAD_REQUEST.as_u16(), msg).into_with_context()
            }
        );

        let Ok(mut cnx) = SQLConnectionAsync::from_pool()
            .await
            .map_err(err_fwd!("💣 New Db connection failed, follower=[{}]", &self.follower))
        else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!("💣 Transaction issue, follower=[{}]", &self.follower))
        else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        };

        let projected_definitions: Vec<TagDefinition> = try_or_return!(
            tag_definition_builder
                .get_tag_definition(&select_tags, &entry_session.customer_code)
                .await
                .map_err(tr_fwd!()),
            |_e| WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context()
        );

        let dao = SearchDao::new(self.follower.clone());
        let Ok(items) = dao
            .search_item_from_query(&mut trans, &sql_query, Some(0), Some(10), &select_tags, &projected_definitions)
            .await
        else {
            log_error!("💣 Cannot find item by id, follower=[{}]", &self.follower);
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        };

        log_info!("😎 We found the items, item count=[{}], follower=[{}]", items.len(), &self.follower);

        if trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        }

        log_info!("🏁 End search_item, follower=[{}]", &self.follower);

        WebTypeWithContext::from_item(StatusCode::OK.as_u16(), GetItemReply { items })
    }

    pub async fn build_query(mut self, build_query_request: BuildQueryRequest) -> WebTypeWithContext<BuildQueryReply> {
        let filter_expression = build_query_request.filters.clone();
        let order_tags = build_query_request.order_tag.clone().map(|order_tag| vec![order_tag.clone()]);

        log_info!(
            "🚀 Start build_query api, query_name=[{}], follower=[{}]",
            &build_query_request.query_name,
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error_ctx()
        );

        let filter_expression_ast: Box<FilterExpressionAST> =
            try_or_return!(analyse_expression(filter_expression.as_deref().unwrap_or("()")), |e: FilterError| {
                let c = ContextMessage { message: e.human_error_message(), context: vec![e.char_position.to_string()] };
                WebTypeWithContext::from_simple(StatusCode::BAD_REQUEST.as_u16(), c)
            });

        let tag_definition_builder = TagDefinitionBuilder::new(self.session_token.clone(), self.follower.clone());
        let mut select_tags = build_select_tags(&filter_expression_ast, &order_tags.clone().unwrap_or_default());
        extend_select_tags(&mut select_tags, build_query_request.extra_properties.as_deref().unwrap_or_default());

        let compiled_query = try_or_return!(
            compile_search_query(
                &filter_expression_ast,
                &tag_definition_builder,
                &select_tags,
                &order_tags.clone().unwrap_or_default(),
                SearchSqlGenerationMode::Persisted,
                &entry_session.customer_code,
            )
            .await,
            |e: GenerationError| {
                log_error!("💣 Fail to generate sql build-query, [{:?}], follower=[{}]", e, &self.follower);
                let msg = SimpleMessage::from(e.to_string());
                WebType::from_simple(StatusCode::BAD_REQUEST.as_u16(), msg).into_with_context()
            }
        );

        let Ok(mut cnx) = SQLConnectionAsync::from_pool()
            .await
            .map_err(err_fwd!("💣 New Db connection failed, follower=[{}]", &self.follower))
        else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!("💣 Transaction issue, follower=[{}]", &self.follower))
        else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        };

        let projected_definitions: Vec<TagDefinition> = try_or_return!(
            tag_definition_builder
                .get_tag_definition(&select_tags, &entry_session.customer_code)
                .await
                .map_err(tr_fwd!()),
            |_e| WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context()
        );

        let dao = SearchDao::new(self.follower.clone());
        let Ok(items) = dao
            .search_item_from_query(
                &mut trans,
                &compiled_query.main_sql,
                None,
                None,
                &select_tags,
                &projected_definitions,
            )
            .await
        else {
            log_error!("💣 Cannot execute compiled search request, follower=[{}]", &self.follower);
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        };

        let mut condition_stats = vec![];
        for condition_stat_query in &compiled_query.condition_stats {
            let Ok(matching_item_count) = dao.execute_count_query(&mut trans, &condition_stat_query.count_sql).await
            else {
                log_error!("💣 Cannot execute IC count query, follower=[{}]", &self.follower);
                return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
            };

            condition_stats.push(BuildQueryConditionStat {
                condition_key: condition_stat_query.condition_key.clone(),
                attribute: condition_stat_query.attribute.clone(),
                operator: condition_stat_query.operator.clone(),
                value: condition_stat_query.value_repr.clone(),
                occurrence: condition_stat_query.occurrence,
                count_query: condition_stat_query.count_sql.clone(),
                matching_item_count,
            });
        }

        if dao
            .persist_compiled_query(
                &mut trans,
                &build_query_request,
                &compiled_query.main_sql,
                items.len(),
                &condition_stats,
                &entry_session.customer_code,
            )
            .await
            .map_err(err_fwd!("💣 Cannot persist compiled query, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        }

        if trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR).into_with_context();
        }

        log_info!("🏁 End build_query, follower=[{}]", &self.follower);

        WebTypeWithContext::from_item(
            StatusCode::OK.as_u16(),
            BuildQueryReply {
                query_name: build_query_request.query_name,
                description: build_query_request.description,
                compiled: true,
                stored: true,
                filters: filter_expression,
                order_tag: build_query_request.order_tag,
                extra_properties: build_query_request.extra_properties,
                matching_item_count: items.len(),
                condition_stats,
            },
        )
    }

    fn web_type_error<T>() -> impl Fn(&ApiError<'static>) -> WebType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            WebType::from_api_error(e)
        }
    }

    fn web_type_error_ctx<T>() -> impl Fn(&ApiError<'static>) -> WebTypeWithContext<T>
    where
        T: DeserializeOwned,
    {
        let f = Self::web_type_error::<T>();
        move |e| f(e).into_with_context()
    }
}

fn collect_attributes(ast: &FilterExpressionAST, attributes: &mut Vec<String>) {
    match ast {
        FilterExpressionAST::Condition(filter_condition) => {
            if !attributes.contains(&filter_condition.attribute) {
                attributes.push(filter_condition.attribute.clone());
            }
        }
        FilterExpressionAST::Logical { leaves, .. } => {
            for leaf in leaves {
                collect_attributes(leaf, attributes);
            }
        }
    }
}

fn build_select_tags(ast: &FilterExpressionAST, order_tags: &[String]) -> Vec<String> {
    let mut select_tags = vec![];
    collect_attributes(ast, &mut select_tags);
    extend_select_tags(&mut select_tags, order_tags);
    select_tags
}

fn extend_select_tags(select_tags: &mut Vec<String>, extra_tags: &[String]) {
    for tag in extra_tags {
        if !select_tags.contains(tag) {
            select_tags.push(tag.clone());
        }
    }
}
