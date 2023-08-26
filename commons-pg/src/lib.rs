
use postgres::{NoTls, Transaction};
use postgres::types::{ToSql};
use std::collections::HashMap;
use std::ops::{Deref};
use std::time::{Duration, SystemTime};
use chrono::{Date, DateTime, NaiveDate, Utc};
use commons_error::*;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use r2d2_postgres::r2d2::{Pool, PooledConnection};
use std::borrow::BorrowMut;
use lazy_static::*;
use log::*;
use mut_static::{MutStatic};

lazy_static! {
    static ref SQL_POOL: MutStatic<SQLPool> = MutStatic::new();
}

// TODO forward the error
pub fn init_db_pool(connect_string: &str, pool_size: u32) {
    let pool = SQLPool::new(connect_string, pool_size)
        .map_err(err_fwd!("Cannot create the static pool")).unwrap();
    let _ = SQL_POOL.set(pool).map_err(err_fwd!("Cannot create the static pool")).unwrap();
}


pub struct SQLPool {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl SQLPool {
    pub fn new(connect_string: &str, pool_size: u32) -> anyhow::Result<Self> {
        let manager = PostgresConnectionManager::new(
            connect_string.parse()?,
            NoTls,
        );

        let pool = r2d2::Pool::builder()
            .max_size(pool_size)
            .connection_timeout(Duration::from_secs(2*3600))
            //.idle_timeout(Some(Duration::from_secs(3600)))
            .build(manager)
            .map_err(err_fwd!("Cannot create the PG connection pool for db [{}]", connect_string))?;

        Ok(Self { pool })
    }


    pub fn pick_connection(&self) -> anyhow::Result<PooledConnection<PostgresConnectionManager<NoTls>>> {
        // Pick a connection from the pool and get the transaction
        let mut my_pool = self.pool.clone();
        let pool = my_pool.borrow_mut();
        let client = pool.get()
            .map_err(err_fwd!("Client from the Connection pool failed"))?;
        Ok(client)
    }
}

pub struct SQLConnection {
    client: PooledConnection<PostgresConnectionManager<NoTls>>,
}

impl SQLConnection {

    pub fn new() -> anyhow::Result<SQLConnection> {
        let pool = SQL_POOL.read().map_err(err_fwd!("*")).unwrap();

        // TODO try to handle to error :(

        // let pool = match SQL_POOL.read().map_err(err_fwd!("*")) {
        //     Ok(x) => { x },
        //     Err(e) => {
        //         log_error!("{:?}", e);
        //         // return Err(anyhow::Error(""));
        //     }
        // };

        let c = pool.pick_connection()
            .map_err(err_fwd!("Connection pickup failed"))?;
        Ok(SQLConnection {
            client: c
        })
    }

    pub fn from_sql_pool(sql_pool: &SQLPool) -> anyhow::Result<SQLConnection> {
        let client = sql_pool.pick_connection()
            .map_err(err_fwd!("Connection pickup failed"))?;
        Ok(SQLConnection {
            client
        })
    }

    pub fn sql_transaction(&'_ mut self) -> anyhow::Result<SQLTransaction<'_>> {
        let t = self.client.transaction().map_err(err_fwd!("Open transaction failed"))?;
        Ok(SQLTransaction {
            inner_transaction: t,
        })
    }
}

pub struct SQLTransaction<'a> {
    inner_transaction: Transaction<'a>,
}

impl<'a> SQLTransaction<'a> {
    pub fn new(inner_transaction: Transaction<'a>) -> Self {
        Self { inner_transaction }
    }

    pub fn commit(self) -> anyhow::Result<()> {
        Ok(self.inner_transaction.commit()?)
    }

    pub fn rollback(self) {
        let _ = self.inner_transaction.rollback();
    }
}


#[derive(Clone, Debug)]
pub struct SQLDataSet {
    // position is 0 when not started, 1 for the first row, and so on.
    position: usize,
    data: Box<Vec<HashMap<String, CellValue>>>,
}

impl SQLDataSet {
    pub fn next(&mut self) -> bool {
        if self.position < self.data.len() {
            self.position += 1;
            return true;
        }
        false
    }

    pub fn restart(&mut self) {
        self.position = 0;
    }


    pub fn len(&self) -> usize {
        self.data.len()
    }


    pub fn get_int(&self, col_name: &str) -> Option<i64> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }

        let row: &HashMap<String, CellValue> = self.data.deref().get(self.position - 1).unwrap();
        let cell = row.get(col_name).unwrap();

        cell.inner_value_int()
    }

    pub fn get_int_32(&self, col_name: &str) -> Option<i32> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }

        let row: &HashMap<String, CellValue> = self.data.deref().get(self.position - 1).unwrap();
        let cell = row.get(col_name).unwrap();

        cell.inner_value_int_32()
    }

    pub fn get_int_16(&self, col_name: &str) -> Option<i16> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }

        let row: &HashMap<String, CellValue> = self.data.deref().get(self.position - 1).unwrap();
        let cell = row.get(col_name).unwrap();

        cell.inner_value_int_16()
    }


    pub fn get_double(&self, col_name: &str) -> Option<f64> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }

        let row: &HashMap<String, CellValue> = self.data.deref().get(self.position - 1).unwrap();
        let cell = row.get(col_name).unwrap();

        cell.inner_value_double()
    }

    pub fn get_bool(&self, col_name: &str) -> Option<bool> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }

        let row: &HashMap<String, CellValue> = self.data.deref().get(self.position - 1)?;
        let cell = row.get(col_name)?;

        cell.inner_value_bool()
    }

    pub fn get_string(&self, col_name: &str) -> Option<String> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }
        let cell = self.data.deref().get(self.position - 1)?.get(col_name)?;
        cell.inner_value_string()
    }

    pub fn get_timestamp(&self, col_name: &str) -> Option<SystemTime> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }
        let row: &HashMap<String, CellValue> = self.data.deref().get(self.position - 1)?;
        let cell = row.get(col_name)?;
        cell.inner_value_systemtime()
    }

    pub fn get_timestamp_as_datetime(&self, col_name: &str) -> Option<DateTime<Utc>> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }

        let opt_dt = self.get_timestamp(col_name).map(|st| Self::system_time_to_date_time(&st));
        opt_dt
    }

    pub fn get_naivedate_as_date(&self, col_name: &str) -> Option<Date<Utc>> {
        if self.position < 1 || self.position > self.data.len() {
            return None;
        }
        let row: &HashMap<String, CellValue> = self.data.deref().get(self.position - 1)?;
        let cell = row.get(col_name).unwrap();
        let opt_d = cell.inner_value_naivedate().map(|nd| {Self::naivedate_to_date(&nd)});

        opt_d
    }


    fn naivedate_to_date(nd: &NaiveDate) -> Date<Utc> {
        Date::from_utc(*nd, Utc)
    }

    fn system_time_to_date_time(t: &SystemTime) -> DateTime<Utc> {
        let dt: DateTime<Utc> = t.clone().into();
        dt
    }

    fn _date_time_to_system_time(dt: &DateTime<Utc>) -> SystemTime {
        let my_dt = dt.clone();
        SystemTime::from(my_dt)
    }
}


pub fn iso_to_datetime(dt_str: &str) -> anyhow::Result<DateTime<Utc>> {
    let dt = DateTime::parse_from_rfc3339(dt_str)?.with_timezone(&Utc);
    anyhow::Result::Ok(dt)
}

pub fn iso_to_date(d_str: &str) -> anyhow::Result<Date<Utc>> {
    let dt_s = format!("{}T00:00:00Z", d_str);
    let dt = DateTime::parse_from_rfc3339(&dt_s)?.with_timezone(&Utc).date();
    anyhow::Result::Ok(dt)
}


pub fn date_time_to_iso(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

pub fn date_to_iso(d: &Date<Utc>) -> String {
    d.format("%Y-%m-%d").to_string()
}

#[derive(Clone, Debug)]
pub enum CellValue {
    String(Option<String>),
    Bool(Option<bool>),
    Int(Option<i64>),
    Int32(Option<i32>),
    Int16(Option<i16>),
    Double(Option<f64>),
    Date(Option<NaiveDate>),
    SystemTime(Option<SystemTime>),
}

impl CellValue {
    pub fn inner_value_int(&self) -> Option<i64> {
        if let CellValue::Int(val) = self {
            *val
        } else {
            None
        }
    }

    pub fn inner_value_int_32(&self) -> Option<i32> {
        if let CellValue::Int32(val) = self {
            *val
        } else {
            None
        }
    }

    pub fn inner_value_int_16(&self) -> Option<i16> {
        if let CellValue::Int16(val) = self {
            *val
        } else {
            None
        }
    }

    pub fn inner_value_double(&self) -> Option<f64> {
        if let CellValue::Double(val) = self {
            *val
        } else {
            None
        }
    }


    pub fn inner_value_bool(&self) -> Option<bool> {
        if let CellValue::Bool(val) = self {
            *val
        } else {
            None
        }

        // if let CellValue::Bool(val) = self {
        //     Some(*val)
        // } else {
        //     None
        // }
    }

    pub fn inner_value_string(&self) -> Option<String> {
        if let CellValue::String(val) = self {
            val.clone()
        } else {
            None
        }
    }

    pub fn inner_value_systemtime(&self) -> Option<SystemTime> {
        if let CellValue::SystemTime(val) = self {
            val.clone()
        } else {
            None
        }
    }

    pub fn inner_value_naivedate(&self) -> Option<NaiveDate> {
        if let CellValue::Date(opt_val) = self {
            opt_val.clone()
        } else {
            None
        }
    }

    pub fn from_raw_int(i: i64) -> Self {
        CellValue::Int(Some(i))
    }

    pub fn from_raw_int_32(i: i32) -> Self {
        CellValue::Int32(Some(i))
    }

    pub fn from_raw_int_16(i: i16) -> Self {
        CellValue::Int16(Some(i))
    }

    pub fn from_raw_double(f: f64) -> Self {
        CellValue::Double(Some(f))
    }

    pub fn from_raw_bool(b: bool) -> Self {
        CellValue::Bool(Some(b))
    }

    pub fn from_raw_systemtime(st: SystemTime) -> Self {
        CellValue::SystemTime(Some(st))
    }

    pub fn from_raw_naivedate(nd: NaiveDate) -> Self {
        CellValue::Date(Some(nd))
    }

    // pub fn from_float( option_val : Option<f64> ) -> Self {
    //     match option_val {
    //         None => {
    //             CellValue::NullFloat
    //         }
    //         Some(val) => {
    //             CellValue::Float(val)
    //         }
    //     }
    // }


    // pub fn from_bool( option_val : Option<bool> ) -> Self {
    //     CellValue::Bool(option_val)
    //     // match option_val {
    //     //     None => {
    //     //         CellValue::NullStr
    //     //     }
    //     //     Some(val) => {
    //     //         CellValue::Bool(val as bool)
    //     //     }
    //     // }
    // }

    pub fn from_raw_str(text: &str) -> Self {
        CellValue::from_raw_string(text.to_owned())
    }

    pub fn from_raw_string(text: String) -> Self {
        CellValue::String(Some(text))
    }

    pub fn from_opt_str(option_val: Option<&str>) -> Self {
        match option_val {
            None => {
                CellValue::String(None)
            }
            Some(val) => {
                CellValue::from_raw_string(val.to_owned())
            }
        }
    }

    pub fn from_opt_systemtime(option_val: Option<SystemTime>) -> Self {
        match option_val {
            None => {
                CellValue::SystemTime(None)
            }
            Some(val) => {
                CellValue::from_raw_systemtime(val)
            }
        }
    }

    pub fn from_opt_naivedate(option_val: Option<NaiveDate>) -> Self {

        // TODO Check this solution
        // let opt_nd =  option_val.map(|x| CellValue::from_raw_systemtime(val));
        // CellValue::Date(opt_nd)

        match option_val {
            None => {
                CellValue::Date(None)
            }
            Some(val) => {
                CellValue::from_raw_naivedate(val)
            }
        }
    }
}

pub struct SQLQueryBlock {
    pub sql_query: String,
    pub start: u32,
    pub length: Option<u32>,
    pub params: HashMap<String, CellValue>,
}

impl SQLQueryBlock {
    pub fn execute(&self, sql_transaction: &mut SQLTransaction) -> anyhow::Result<SQLDataSet> {

        // assign a number to all of the params
        // p_name => 0, p_id => 1

        let null_str = "".to_owned();
        let (mut new_sql_string, v_params) = parse_query(self.sql_query.as_str(), &self.params, &null_str);

        match self.length {
            None => {
                new_sql_string.push_str(format!(" OFFSET {} ", self.start).as_str());
            }
            Some(l) => {
                new_sql_string.push_str(format!(" OFFSET {} LIMIT {}", self.start, l).as_str());
            }
        }

        let result_set = sql_transaction.inner_transaction.query(new_sql_string.as_str(), v_params.as_slice())
            .map_err(err_fwd!("Sql query failed, sql [{}]", new_sql_string.as_str()))?;

        let mut result: Vec<HashMap<String, CellValue>> = vec![];

        for row in result_set {
            let column = row.columns();
            let mut my_row: HashMap<String, CellValue> = HashMap::new();

            for col in column {
                let name = col.name();
                let ty = col.type_();

                // TODO manage more types (Boolean, decimal, time only)
                match ty.name() {
                    "int2" => {
                        let db_value: Option<i16> = row.get(name);
                        let option_cell = CellValue::Int16(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "int4" => {
                        let db_value: Option<i32> = row.get(name);
                        let option_cell = CellValue::Int32(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "int8" => {
                        let db_value: Option<i64> = row.get(name);
                        let option_cell = CellValue::Int(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "float8" => {
                        let db_value: Option<f64> = row.get(name);
                        let option_cell = CellValue::Double(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "bool" => {
                        let db_value: Option<bool> = row.get(name);
                        let option_cell = CellValue::Bool(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "varchar" | "bpchar" | "text" => {
                        let db_value: Option<&str> = row.get(name);
                        let option_cell = CellValue::from_opt_str(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "date" => {
                        let db_value: Option<chrono::NaiveDate> = row.get(name);
                        let option_cell = CellValue::from_opt_naivedate(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "timestamp" => {
                        let db_value: Option<SystemTime> = row.get(name);
                        let option_cell = CellValue::from_opt_systemtime(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    // "tsvector" => {
                    //     let db_value = row.vzip();
                    //
                    //     // log_info!("db value {:?}", &db_value);
                    //     //let option_cell = CellValue::from_opt_str(db_value);
                    //     //my_row.insert(name.to_owned(), option_cell);
                    // }
                    _ => {
                        log_error!("Unknown type name [{}]", ty.name());
                    }
                }
            }
            result.push(my_row);
        }

        Ok(SQLDataSet { position: 0, data: Box::new(result) })
    }
}

/// Analyse the template query with named params and compare it to the list of input parameters.
/// Return the actual Sql query with $ parameters and an ordered list of usable parameter.
fn parse_query<'a>(string_template: &str, params: &'a HashMap<String, CellValue>, _parent_scope: &'a String) -> (String, Vec<&'a (dyn ToSql + Sync)>) {
    let mut counter = 1;
    let mut new_sql_string = string_template.to_string();
    let mut v_params: Vec<&'a (dyn ToSql + Sync)> = vec![];

    for p in params {
        let param_name = p.0;
        let param_value = p.1;

        // parse the query params :p_xxx
        let from = format!(":{}", param_name.as_str());
        let to = format!("${}", counter);

        new_sql_string = new_sql_string.replace(&from, to.as_str());

        // Store the param value
        match param_value {
            CellValue::Int(i) => {
                v_params.push(i);
            }
            CellValue::Int32(i) => {
                v_params.push(i);
            }
            CellValue::Int16(i) => {
                v_params.push(i);
            }
            CellValue::Double(f) => {
                v_params.push(f);
            }
            CellValue::Bool(b) => {
                v_params.push(b);
            }
            CellValue::String(s) => {
                v_params.push(s);
            }
            CellValue::Date(st) => {
                v_params.push(st);
            }
            CellValue::SystemTime(st) => {
                v_params.push(st);
            }
        }

        counter = counter + 1;
    }

    (new_sql_string, v_params)
}

// For Update and insert
#[derive(Debug)]
pub struct SQLChange {
    pub sql_query: String,
    pub params: HashMap<String, CellValue>,
    pub sequence_name: String,
}


impl SQLChange {
    pub fn batch(&self, sql_transaction: &mut SQLTransaction) -> anyhow::Result<()> {
        let _ = sql_transaction.inner_transaction.batch_execute(
            self.sql_query.as_str(),
        ).map_err(err_fwd!("Batch execution failed, sql [{}]", self.sql_query.as_str()))?;

        Ok(())
    }


    fn execute(&self, sql_transaction: &mut SQLTransaction) -> anyhow::Result<u64> {
        let null_str = "".to_owned();
        let (new_sql_string, v_params) = parse_query(self.sql_query.as_str(), &self.params, &null_str);

        log_debug!("New sql query : [{}]", &new_sql_string);
        let change_query_info = sql_transaction.inner_transaction.execute(
            new_sql_string.as_str(),
            v_params.as_slice(),
        ).map_err(err_fwd!("Query execution failed, sql [{}]", new_sql_string.as_str()))?;

        log_debug!("change query info: [{}]", &change_query_info);
        Ok(change_query_info)
    }


    // Return the id of the new row if success
    pub fn insert(&self, sql_transaction: &mut SQLTransaction) -> anyhow::Result<i64> {
        let _ = self.execute(sql_transaction)?;

        let sql = format!("SELECT currval('{}')", self.sequence_name);
        let pk_info = sql_transaction.inner_transaction.query_one(sql.as_str(), &[])
            .map_err(err_fwd!("Query execution failed, sql [{}]", &sql))?;

        let pk: i64 = pk_info.get(0);
        log_debug!("Primary key : [{}]", &pk);

        Ok(pk)
    }

    pub fn insert_no_pk(&self, sql_transaction: &mut SQLTransaction) -> anyhow::Result<()> {
        let _ = self.execute(sql_transaction)?;
        Ok(())
    }

    pub fn update(&self, sql_transaction: &mut SQLTransaction) -> anyhow::Result<u64> {
        let update_info = self.execute(sql_transaction)?;
        Ok(update_info)
    }

    pub fn delete(&self, sql_transaction: &mut SQLTransaction) -> anyhow::Result<u64> {
        let delete_info = self.execute(sql_transaction)?;
        Ok(delete_info)
    }
}


#[cfg(test)]
mod tests {
    use crate::{SQLQueryBlock, CellValue, SQLChange, SQLPool, SQLConnection, init_db_pool};
    use std::collections::HashMap;
    use std::fs::File;
    use commons_error::*;
    use std::path::Path;
    use std::process::exit;
    use std::sync::Once;
    use commons_error::*;

    static INIT: Once = Once::new();

    fn init() {
        INIT.call_once(|| {
            let log_config: String = "E:/doka-configs/dev/ppm/config/log4rs.yaml".to_string();
            let log_config_path = Path::new(&log_config);

            match log4rs::init_file(&log_config_path, Default::default()) {
                Err(e) => {
                    eprintln!("{:?} {:?}", &log_config_path, e);
                    exit(-59);
                }
                Ok(_) => {}
            }
        });
    }


    #[test]
    fn a10_faulty_connection() {
        init();

        let r_sql_pool = SQLPool::new("host=pg13 port=5432 dbname=p2_prod_2 user=denis password=wrong_pass.", 1)
            .map_err(err_fwd!("Fail the pool"));

        assert!(r_sql_pool.is_err());
    }


    #[test]
    fn a20_simple_query() -> anyhow::Result<()> {
        init();
        init_db_pool("host=pg13 port=5432 dbname=p2_prod_2 user=denis password=Oratece4.", 2);

        let mut cnx = SQLConnection::new().map_err(err_fwd!("New Sql connection failed"))?;
        let mut trans = cnx.sql_transaction().map_err(err_fwd!("Error transaction"))?;

        let query = SQLQueryBlock {
            sql_query: "SELECT id, customer_name, ciphered_password FROM public.keys ORDER BY customer_name".to_string(),
            start: 0,
            length: Some(10),
            params: HashMap::new(),
        };

        let mut sql_result = query.execute(&mut trans).map_err(err_fwd!("Query failed"))?;

        trans.commit()?;

        if sql_result.next() {
            let id = sql_result.get_int("id");
            if let Some(val) = id {
                assert!(true);
            }
        } else {
            assert!(false);
        }


        Ok(())
    }


    #[test]
    fn a30_param_request() {
        init();

        let sql_string = r#" SELECT id, name, type, created, last_modified, category_id, tag_country
                                    FROM public.item_1
                                    WHERE name like :p_name AND ( :p_name IS NOT NULL )
                                    AND id > :p_id AND  :p_id < 400 "#;

        init_db_pool("host=pg13 port=5432 dbname=p2_prod_2 user=denis password=Oratece4.", 2);

        let mut cnx = SQLConnection::new()
            .map_err(err_fwd!("Connection issue")).unwrap();

        let mut trans = cnx.sql_transaction().map_err(err_fwd!("Transaction issue")).unwrap();

        let mut params: HashMap<String, CellValue> = HashMap::new();
        params.insert("p_name".to_owned(), CellValue::from_raw_string("A%".to_owned()));
        params.insert("p_id".to_owned(), CellValue::from_raw_int(180));

        let query = SQLQueryBlock {
            sql_query: sql_string.to_string(),
            start: 0,
            length: Some(10),
            params,
        };

        let mut data_set = query.execute(&mut trans).unwrap();

        if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        }

        while data_set.next() {
            let id = data_set.get_int("id");
            let name = data_set.get_string("name");
            let the_type = data_set.get_string("type");
            let created = data_set.get_timestamp("created");
            let last_modified = data_set.get_timestamp("last_modified");

            let category_id = data_set.get_int("category_id");
            let tag_country = data_set.get_string("tag_country");

        }

        assert!(data_set.len() > 0);
        assert_eq!(data_set.position, data_set.len());
    }

    #[test]
    fn a40_insert_row() {
        init();

        init_db_pool("host=pg13 port=5432 dbname=p2_prod_2 user=denis password=Oratece4.", 2);

        let mut cnx = SQLConnection::new().map_err(err_fwd!("Connection issue")).unwrap();
        let mut trans = cnx.sql_transaction().map_err(err_fwd!("Transaction issue")).unwrap();

        let mut params: HashMap<String, CellValue> = HashMap::new();
        params.insert("p_customer_id".to_owned(), CellValue::from_raw_int(26));
        params.insert("p_customer_key".to_owned(), CellValue::from_raw_string("The Encrypted Key".to_string()));


        let query = SQLChange {
            sql_query: "INSERT INTO keys (customer_name, ciphered_password) VALUES (:p_customer_id, :p_customer_key)".to_string(),
            params,
            sequence_name: "keys_id_seq".to_string(),
        };

        let id = query.insert(&mut trans).unwrap();
        if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        }

        assert!(id > 10)
    }


    #[test]
    fn update_row() {
        init();

        init_db_pool("host=pg13 port=5432 dbname=p2_prod_2 user=denis password=Oratece4.", 2);

        let mut cnx = SQLConnection::new().unwrap();
        let mut trans = cnx.sql_transaction().unwrap();

        let mut params: HashMap<String, CellValue> = HashMap::new();
        params.insert("p_customer_id".to_owned(), CellValue::from_raw_int(10));
        params.insert("p_customer_key".to_owned(), CellValue::from_raw_string("N/A".to_string()));

        let query = SQLChange {
            sql_query: "UPDATE public.keys SET ciphered_password = :p_customer_key WHERE id > :p_customer_id".to_string(),
            params,
            sequence_name: "".to_string(),
        };

        match query.update(&mut trans) {
            Ok(id) => {
                println!("{:?}", id);
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }

        if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        }
    }


    #[test]
    fn test_anyhow_4() {
        init();

        // let var = 125;
        // let txt = "sample text";
        // let _res = open_file_anyhow_4().map_err(err_fwd!("Second error by anyhow [{}] [{}]", &var, &txt) );

        let filename = "bar.txt";
        let _f = File::open(filename).map_err(
            err_fwd!("First error managed by anyhow, filename=[{}]", filename)
        );
    }


    #[test]
    fn test_pg() {
        use postgres::{Client, NoTls};

        init();

        let mut client = Client::connect("host=postgresql95-c1 port=5433 dbname=p2_prod_2 user=denis password=Oratece4.", NoTls)
            .unwrap();

        //     client.batch_execute("
        // CREATE TABLE person (
        //     id      SERIAL PRIMARY KEY,
        //     name    TEXT NOT NULL,
        //     data    BYTEA
        // )
        //     ")?;

        // let name = "Ferris";
        // let data = None::<&[u8]>;
        // client.execute(
        //     "INSERT INTO person (name, data) VALUES ($1, $2)",
        //     &[&name, &data],
        // )?;

        for row in client.query("SELECT customer_id, customer_key FROM public.keys ORDER BY customer_id", &[]).unwrap() {
            let id: i64 = row.get(0);
            let name: &str = row.get(1);

            println!("found person: {} {}", id, name);
        }
    }
}
