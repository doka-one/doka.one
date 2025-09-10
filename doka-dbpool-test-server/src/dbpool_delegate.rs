use std::collections::HashMap;

use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection};
use dkdto::error_codes::INTERNAL_DATABASE_ERROR;
use dkdto::web_types::{WebType, WebTypeBuilder};
use log::*;
use rocket::http::Status;

pub(crate) struct DbPoolDelegate {}

impl DbPoolDelegate {
    pub fn new() -> Self {
        Self {}
    }

    ///
    /// 🔑 Grab...
    ///
    /// It's usually ...
    ///
    pub fn grab_ctx(&self, duration: u32) -> WebType<String> {
        log_info!("🚀 Start grab ctx api, duration=[{}]", duration);

        let Ok(mut cnx) = SQLConnection::new().map_err(err_fwd!("💣 Connection issue, duration=[{}]", duration))
        else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        };

        let r_trans = cnx.sql_transaction().map_err(err_fwd!("💣 Transaction issue, duration=[{}]", duration));
        let Ok(mut trans) = r_trans else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        };

        let sql_insert = r#"INSERT INTO public.connection_history
                            (timer, description, status)
                            VALUES (now(), :p_description, :p_status)"#;

        let mut params: HashMap<String, CellValue> = HashMap::new();
        params.insert("p_description".to_owned(), CellValue::from_raw_str(&format!("DESC_{}", duration)));
        params.insert("p_status".to_owned(), CellValue::from_raw_str("OPEN"));

        let query = SQLChange {
            sql_query: sql_insert.to_string(),
            params,
            sequence_name: "public.connection_history_id_seq".to_string(),
        };

        let Ok(session_db_id) =
            query.insert(&mut trans).map_err(err_fwd!("💣 Cannot insert the session, duration=[{}]", duration))
        else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        };

        if trans.commit().map_err(err_fwd!("💣 Commit failed, duration=[{}]", duration)).is_err() {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        }

        log_info!("😎 Connection was grabbed with success, session_db_id=[{}], duration=[{}]", session_db_id, duration);

        log_info!("🏁 End grab ctx, duration=[{}]", duration);
        WebType::from_item(Status::Accepted.code, "OK".to_string())
    }
}
