// TODO how to move it in a dao sub folder ?

use std::collections::HashMap;
use std::time::SystemTime;
use commons_pg::{CellValue, SQLChange, SQLTransaction};
use commons_error::*;
use log::*;

pub fn create_item(trans : &mut SQLTransaction, item_name: &str, customer_code : &str) -> anyhow::Result<i64> {
    let sql_query = format!( r"INSERT INTO cs_{}.item(name, created_gmt, last_modified_gmt)
    VALUES (:p_name, :p_created, :p_last_modified)", customer_code );

    let sequence_name = format!( "cs_{}.item_id_seq", customer_code );

    let now = SystemTime::now();
    let mut params = HashMap::new();
    params.insert("p_name".to_string(), CellValue::from_raw_string(item_name.to_string()));
    params.insert("p_created".to_string(), CellValue::from_raw_systemtime(now.clone()));
    params.insert("p_last_modified".to_string(), CellValue::from_raw_systemtime(now.clone()));

    let sql_insert = SQLChange {
        sql_query,
        params,
        sequence_name,
    };

    let item_id = sql_insert.insert(trans).map_err(err_fwd!("Insertion of a new item failed"))?;

    log_info!("Created item : item_id=[{}]", item_id);
    Ok(item_id)
}

// pub fn create_item_file(trans : &mut SQLTransaction, item_id: i64, file_ref: &str, customer_code : &str) -> anyhow::Result<()> {
//     let sql_query = format!( r"INSERT INTO cs_{}.item_file(item_id, file_ref)
//     VALUES (:p_item_id, :p_file_ref)", customer_code );
//
//     // let sequence_name = format!( "cs_{}.item_id_seq", customer_code );
//
//     let mut params = HashMap::new();
//     params.insert("p_item_id".to_string(), CellValue::from_raw_int(item_id));
//     params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_owned()));
//
//     let sql_insert = SQLChange {
//         sql_query,
//         params,
//         sequence_name : String::new(),
//     };
//
//     let _= sql_insert.insert(trans).map_err(err_fwd!("Insertion of a new item_file failed"))?;
//
//     log_info!("Created item_file : item_id=[{}], file_ref=[{}]", item_id, file_ref);
//     Ok(())
// }