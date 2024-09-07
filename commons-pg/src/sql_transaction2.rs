use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::anyhow;
use chrono::NaiveDateTime;
use futures::future::Lazy;
use futures::TryStreamExt;
use lazy_static::*;
use log::*;
use mut_static::MutStatic;
use postgres_types::ToSql;
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgArguments, PgPoolOptions};
use sqlx::{
    Arguments, Column, Connection, Encode, Execute, Executor, PgPool, Pool, Postgres, Row,
    Transaction, Type, TypeInfo,
};

use commons_error::*;

use crate::sql_transaction::{naive_datetime_to_system_time, CellValue, SQLDataSet};

lazy_static! {
    static ref SQL_POOL2: OnceLock<SQLPool2> = OnceLock::new();
}

pub async fn init_db_pool2(connect_string: &str, pool_size: u32) -> anyhow::Result<()> {
    if SQL_POOL2.get().is_none() {
        let pool = match SQLPool2::new(connect_string, pool_size)
            .await
            .map_err(err_fwd!("Cannot create the DB pool"))
        {
            Ok(p) => p,
            Err(_) => return Err(anyhow!("_")),
        };
        match SQL_POOL2.set(pool) {
            Ok(_) => {}
            Err(_) => return Err(anyhow!("Impossible to set the pool")),
        }
    }
    Ok(())
}

/// Analyse the template query with named params and compare it to the list of input parameters.
/// Return the actual Sql query with $ parameters and an ordered list of usable parameter.
pub(crate) fn parse_query2<'a>(
    string_template: &str,
    params: &'a HashMap<String, CellValue>,
    _parent_scope: &'a String,
) -> (String, Vec<CellValue>) {
    let mut counter = 1;
    let mut new_sql_string = string_template.to_string();
    let mut v_params: Vec<CellValue> = vec![];

    for p in params {
        let param_name = p.0;
        let param_value = p.1;
        // parse the query params :p_xxx
        let from = format!(":{}", param_name.as_str());
        let to = format!("${}", counter);
        new_sql_string = new_sql_string.replace(&from, to.as_str());
        let owned_cell = (*param_value).clone();
        v_params.push(owned_cell);
        counter = counter + 1;
    }

    (new_sql_string, v_params)
}

pub struct SQLPool2 {
    pool: Pool<Postgres>,
}

impl SQLPool2 {
    pub async fn new(connect_string: &str, pool_size: u32) -> anyhow::Result<Self> {
        // connect_string :  "postgres://doka:doka@localhost:5432/ad_test_03";

        // Configure le pool de connexions
        let pool = PgPoolOptions::new()
            .min_connections(1) // Taille minimale du pool
            .max_connections(pool_size) // Taille maximale du pool
            .idle_timeout(Duration::from_secs(30)) // Timeout pour se connecter
            .idle_timeout(Some(Duration::from_secs(10 * 60))) // Timeout d'inactivité des connexions
            .max_lifetime(Some(Duration::from_secs(2 * 60 * 60))) // Durée de vie maximale d'une connexion
            .connect(connect_string)
            .await?;

        Ok(Self { pool })
    }

    pub async fn pick_connection(&self) -> anyhow::Result<PoolConnection<Postgres>> {
        // Get a single connection from the pool
        let cnx = self.pool.acquire().await?;
        Ok(cnx)
    }
}

pub struct SQLConnection2 {
    pub client: PoolConnection<Postgres>,
}

impl SQLConnection2 {
    pub async fn from_pool() -> anyhow::Result<SQLConnection2> {
        let sql_pool = match SQL_POOL2.get() {
            Some(p) => p,
            None => return Err(anyhow!("Cannot read the pool")),
        };
        let client = sql_pool.pick_connection().await?;
        Ok(SQLConnection2 { client })
    }

    pub async fn begin<'a>(&'a mut self) -> anyhow::Result<SQLTransaction2<'a>> {
        let t = self.client.begin().await?;
        Ok(SQLTransaction2 {
            inner_transaction: t,
        })
    }
}

pub struct SQLTransaction2<'a> {
    // inner_transaction: &'a PgConnection,
    inner_transaction: Transaction<'a, Postgres>,
}

impl<'a> SQLTransaction2<'a> {
    pub fn new(inner_transaction: Transaction<'a, Postgres>) -> Self {
        Self { inner_transaction }
    }

    pub async fn commit(self) -> anyhow::Result<()> {
        Ok(self.inner_transaction.commit().await?)
    }

    pub async fn rollback(self) {
        let _ = self.inner_transaction.rollback().await;
    }
}

pub struct SQLQueryBlock2 {
    pub sql_query: String,
    pub start: u32,
    pub length: Option<u32>,
    pub params: HashMap<String, CellValue>,
}

fn bind_cell_to_query<'q>(
    cell: CellValue,
    query_builder: sqlx::query::Query<'q, Postgres, PgArguments>,
) -> sqlx::query::Query<'q, Postgres, PgArguments> {
    match cell {
        CellValue::String(value) => query_builder.bind(value),
        CellValue::Bool(value) => query_builder.bind(value),
        CellValue::Int(value) => query_builder.bind(value),
        CellValue::Int32(value) => query_builder.bind(value),
        CellValue::Int16(value) => query_builder.bind(value),
        CellValue::Double(value) => query_builder.bind(value),
        CellValue::Date(value) => query_builder.bind(value),
        // TODO implement CellValue::DateTime(valeue: NaiveDateTime)
        CellValue::SystemTime(value) => {
            let opt_naive_datetime = match value {
                None => None,
                Some(sys_datetime) => {
                    // Obtenir la durée écoulée depuis l'époque UNIX
                    let duration_since_epoch = sys_datetime.duration_since(UNIX_EPOCH).unwrap();
                    // Convertir la durée en secondes
                    let seconds = duration_since_epoch.as_secs();
                    // Convertir les secondes en NaiveDateTime
                    let naive_datetime = NaiveDateTime::from_timestamp(
                        seconds as i64,
                        duration_since_epoch.subsec_nanos(),
                    );
                    Some(naive_datetime)
                }
            };
            query_builder.bind(opt_naive_datetime)
        }
    }
}

impl SQLQueryBlock2 {
    /// Main routine to perform a select query
    pub async fn execute(
        &self,
        sql_transaction: &mut SQLTransaction2<'_>,
    ) -> anyhow::Result<SQLDataSet> {
        let null_str = "".to_owned();
        let (mut new_sql_string, v_params) =
            parse_query2(self.sql_query.as_str(), &self.params, &null_str);

        match self.length {
            None => {
                new_sql_string.push_str(format!(" OFFSET {} ", self.start).as_str());
            }
            Some(l) => {
                new_sql_string.push_str(format!(" OFFSET {} LIMIT {}", self.start, l).as_str());
            }
        }

        let mut query_builder = sqlx::query(new_sql_string.as_str());

        let v_params_debug = v_params.clone();
        for param in v_params {
            query_builder = bind_cell_to_query(param, query_builder);
        }

        let mut result_set = query_builder.fetch(&mut *sql_transaction.inner_transaction);

        let mut result: Vec<HashMap<String, CellValue>> = vec![];
        while let Some(row) = result_set.try_next().await.map_err(err_fwd!(
            "SQL query failed : {}, Params : {:?}",
            new_sql_string.as_str(),
            v_params_debug
        ))? {
            let mut my_row: HashMap<String, CellValue> = HashMap::new();

            for col in row.columns() {
                let name = col.name();
                let ty = col.type_info().name().to_lowercase();
                // To handle more types, take a look at the PgType enum
                match ty.as_str() {
                    "int2" => {
                        let db_value: Option<i16> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let option_cell = CellValue::Int16(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "int4" => {
                        let db_value: Option<i32> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let option_cell = CellValue::Int32(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "int8" => {
                        let db_value: Option<i64> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let option_cell = CellValue::Int(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "float8" => {
                        let db_value: Option<f64> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let option_cell = CellValue::Double(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "bool" => {
                        let db_value: Option<bool> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let option_cell = CellValue::Bool(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "varchar" | "bpchar" | "text" => {
                        let db_value: Option<&str> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let option_cell = CellValue::from_opt_str(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "date" => {
                        let db_value: Option<chrono::NaiveDate> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let option_cell = CellValue::from_opt_naivedate(db_value);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    "timestamp" => {
                        // TODO use a NativeDateTime instead of a SystemTime
                        let db_value: Option<NaiveDateTime> = row
                            .try_get(name)
                            .map_err(err_fwd!("Error reading column: {}", name))?;
                        let systime = naive_datetime_to_system_time(db_value);
                        let option_cell = CellValue::from_opt_systemtime(systime);
                        my_row.insert(name.to_owned(), option_cell);
                    }
                    t => {
                        log_error!("Unknown type name [{}]", t);
                    }
                }
            }
            result.push(my_row);
        }

        Ok(SQLDataSet {
            position: 0,
            data: Box::new(result),
        })
    }
}

// For Update and insert
#[derive(Debug)]
pub struct SQLChange2 {
    pub sql_query: String,
    pub params: HashMap<String, CellValue>,
    pub sequence_name: String,
}

impl SQLChange2 {
    pub async fn batch(&self, sql_transaction: &mut SQLTransaction2<'_>) -> anyhow::Result<()> {
        let _ = sql_transaction
            .inner_transaction
            .execute(self.sql_query.as_str())
            .await
            .map_err(err_fwd!(
                "Batch execution failed, sql [{}]",
                self.sql_query.as_str()
            ))?;

        Ok(())
    }

    /// Base routine for update, insert and delete
    async fn change(&self, sql_transaction: &mut SQLTransaction2<'_>) -> anyhow::Result<()> {
        let null_str = "".to_owned();
        let (mut new_sql_string, v_params) =
            parse_query2(self.sql_query.as_str(), &self.params, &null_str);
        let mut query_builder = sqlx::query(new_sql_string.as_str());
        let v_params_debug = v_params.clone();
        for param in v_params {
            query_builder = bind_cell_to_query(param, query_builder);
        }
        let r = query_builder
            .execute(&mut *sql_transaction.inner_transaction)
            .await
            .map_err(err_fwd!(
                "Query failed : {}, Params : {:?}",
                new_sql_string.as_str(),
                v_params_debug
            ))?;

        Ok(())
    }

    /// Return the id of the new row if success
    pub async fn insert(&self, sql_transaction: &mut SQLTransaction2<'_>) -> anyhow::Result<i64> {
        let _ = self.change(sql_transaction).await?;
        let sql = format!("SELECT currval('{}')", self.sequence_name);

        let mut query_builder = sqlx::query(&sql);
        let r = query_builder
            .fetch_one(&mut *sql_transaction.inner_transaction)
            .await?;

        let pk: i64 = r.try_get(0)?;
        log_debug!("Primary key : [{}]", &pk);
        Ok(pk)
    }

    pub async fn insert_no_pk(
        &self,
        sql_transaction: &mut SQLTransaction2<'_>,
    ) -> anyhow::Result<()> {
        let insert_info = self.change(sql_transaction).await?;
        Ok(insert_info)
    }

    pub async fn update(&self, sql_transaction: &mut SQLTransaction2<'_>) -> anyhow::Result<()> {
        let update_info = self.change(sql_transaction).await?;
        Ok(update_info)
    }

    pub async fn delete(&self, sql_transaction: &mut SQLTransaction2<'_>) -> anyhow::Result<()> {
        let delete_info = self.change(sql_transaction).await?;
        Ok(delete_info)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fmt::format;
    use std::path::Path;
    use std::process::exit;
    use std::sync::Once;
    use std::thread;
    use std::time::{Duration, SystemTime};

    use chrono::NaiveDate;
    use sqlx::postgres::PgPoolOptions;
    use sqlx::{query, Acquire, PgConnection, Row, Transaction};
    use tokio::sync::Mutex;
    use tokio::task;
    use tokio::task::JoinHandle;

    use crate::sql_transaction::CellValue;
    use crate::sql_transaction2::{
        init_db_pool2, SQLChange2, SQLConnection2, SQLQueryBlock2, SQLTransaction2,
    };

    /// ```sql
    /// CREATE TABLE public.book (
    ///     id int4 NOT NULL,
    ///     title varchar(200) NOT NULL,
    ///     isbn varchar(60) NULL,
    ///     created_dt date NULL,
    ///     precision_time timestamp NULL
    /// );
    ///
    ///
    /// CREATE SEQUENCE public.book_id_seq
    /// INCREMENT BY 1
    /// MINVALUE 1
    /// MAXVALUE 9223372036854775807
    /// START 1
    /// CACHE 1
    /// NO CYCLE;
    ///
    /// ```

    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| {
            let log_config: String =
                "/mnt/blob/installation_test_03/doka-configs/test_03/doka-test/config/log4rs.yaml"
                    .to_string();
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

    static INITIALISED: Mutex<bool> = Mutex::const_new(false);

    async fn init_pool_once() {
        println!("Penging mutex...");
        let mut initialised = INITIALISED.lock().await;
        println!("mutex locked");
        if *initialised {
            println!("Already initialized");
            return;
        }
        println!("Do it now...");
        let r = init_pool().await;
        // tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        *initialised = true;
        println!("Done");
    }

    async fn init_pool() -> anyhow::Result<()> {
        init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await
    }

    async fn create(trans: &mut PgConnection, title: &str) -> anyhow::Result<()> {
        let query = "INSERT INTO public.book (id, title) VALUES(nextval('book_id_seq'), $1)";
        let r = sqlx::query(query).bind(title).execute(trans).await?;
        Ok(())
    }

    /// Raw connection to PG with SQLX, no lib
    #[tokio::test]
    async fn a10_raw_sqlx_connection() -> anyhow::Result<()> {
        init();
        let url = "postgres://doka:doka@localhost:5432/ad_test_03";

        // Configure le pool de connexions
        let pool = PgPoolOptions::new()
            .min_connections(1) // Taille minimale du pool
            .max_connections(5) // Taille maximale du pool
            .idle_timeout(Duration::from_secs(30)) // Timeout pour se connecter
            .idle_timeout(Some(Duration::from_secs(10))) // Timeout d'inactivité des connexions
            .max_lifetime(Some(Duration::from_secs(300))) // Durée de vie maximale d'une connexion
            .connect(url)
            .await?;

        // Acquérir une connexion individuelle à partir du pool
        let mut cnx = pool.acquire().await?;

        let mut trans: Transaction<'_, sqlx::Postgres> = cnx.begin().await?;

        let pg_trans = &mut *trans;

        let row = query("SELECT 1+1 as sum").fetch_one(&mut *pg_trans).await?;
        // Extract the value from the row
        let sum: i32 = row.get("sum");

        println!("The sum is: {}", sum);

        let r = create(pg_trans, "Another book").await?;
        let r = create(pg_trans, "Dune").await?;

        trans.commit().await?;

        Ok(())
    }

    async fn create_book(trans: &mut SQLTransaction2<'_>, title: &str) -> anyhow::Result<()> {
        let query = SQLQueryBlock2 {
            sql_query:
            "INSERT INTO public.book (id, title) VALUES(nextval('book_id_seq'), 'L''aventurier')"
                .to_string(),
            start: 0,
            length: None,
            params: Default::default(),
        };

        let r = query.execute(&mut *trans).await?;

        Ok(())
    }

    /// Use of the SQLConnection and SQLTransaction from our lib
    #[tokio::test]
    async fn a15_cnx_and_trans() -> anyhow::Result<()> {
        init();
        let r = init_pool_once().await;
        // init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;

        create_book(&mut trans, "Super book").await?;
        create_book(&mut trans, "Super book 2").await?;

        trans.commit().await?;

        Ok(())
    }

    // async fn create_book1(trans: &mut SQLTransaction2<'_>, title: &str) -> anyhow::Result<()> {
    //     let query = SQLQueryBlock2 {
    //         sql_query: "INSERT INTO public.book (id, title) VALUES($1, $2)".to_string(),
    //         start: 0,
    //         length: None,
    //         params: Default::default(),
    //     };
    //
    //     let r = query.execute(&mut *trans).await?;
    //
    //     Ok(())
    // }
    //
    // #[tokio::test]
    // async fn a16_cnx_and_trans() -> anyhow::Result<()> {
    //     init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;
    //
    //     let mut cnx = SQLConnection2::from_pool().await?;
    //     let mut trans = cnx.sql_transaction().await?;
    //
    //     create_book1(&mut trans, "Super book").await?;
    //
    //     trans.commit().await?;
    //
    //     Ok(())
    // }

    /// Simple select
    #[tokio::test]
    async fn a20_simple_query() -> anyhow::Result<()> {
        let r = init_pool_once().await;
        // init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;

        let mut params = HashMap::new();
        params.insert("p_id".to_owned(), CellValue::from_raw_int(2000));

        let query = SQLQueryBlock2 {
            sql_query: "SELECT id, title FROM public.book WHERE id = :p_id AND id < :p_id + 10 ORDER BY title"
                .to_string(),
            start: 0,
            length: Some(10),
            params: params,
        };

        let mut sql_result = query.execute(&mut trans).await?;

        trans.commit().await?;

        // if sql_result.next() {
        //     let id = sql_result.get_int("id");
        //     if let Some(val) = id {
        //         assert!(true);
        //     }
        // } else {
        //     assert!(false);
        // }

        Ok(())
    }

    /// Simple inserts from the execute method
    #[tokio::test]
    async fn a22_simple_insert() -> anyhow::Result<()> {
        init();
        let r = init_pool_once().await;
        // init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;

        let mut params = HashMap::new();
        params.insert(
            "p_title".to_owned(),
            CellValue::from_raw_str("Game of Thrones"),
        );
        params.insert("p_isbn".to_owned(), CellValue::from_opt_str(None));

        let dt = NaiveDate::from_ymd(2024, 8, 15);
        params.insert("p_created_dt".to_owned(), CellValue::from_raw_naivedate(dt));
        params.insert(
            "p_precision_time".to_owned(),
            CellValue::from_raw_systemtime(SystemTime::now()),
        );

        let query = SQLQueryBlock2 {
            sql_query:
                "INSERT INTO public.book (id, title, isbn, created_dt, precision_time) VALUES(nextval('book_id_seq'),\
             :p_title, :p_isbn, :p_created_dt, :p_precision_time)"
                    .to_string(),
            start: 0,
            length: Some(10),
            params: params,
        };

        let mut sql_result = query.execute(&mut trans).await?;

        trans.commit().await?;

        Ok(())
    }

    /// Insert a row using the sequence for the id column
    #[tokio::test]
    async fn a30_insert_with_sequence() -> anyhow::Result<()> {
        init();
        let r = init_pool_once().await;
        // init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;

        let mut params = HashMap::new();
        params.insert(
            "p_title".to_owned(),
            CellValue::from_raw_str("Game of Thrones"),
        );
        params.insert("p_isbn".to_owned(), CellValue::from_opt_str(None));

        let dt = NaiveDate::from_ymd(2024, 8, 15);
        params.insert("p_created_dt".to_owned(), CellValue::from_raw_naivedate(dt));
        params.insert(
            "p_precision_time".to_owned(),
            CellValue::from_raw_systemtime(SystemTime::now()),
        );

        let query = SQLChange2 {
            sql_query:
            "INSERT INTO public.book (id, title, isbn, created_dt, precision_time) VALUES(nextval('book_id_seq'),\
             :p_title, :p_isbn, :p_created_dt, :p_precision_time)"
                .to_string(),
            params: params,
            sequence_name: "book_id_seq".to_string(),
        };

        let pk_id = query.insert(&mut trans).await?;

        trans.commit().await?;
        println!("The pk is: {}", pk_id);
        assert_eq!(true, pk_id > 0);

        Ok(())
    }

    /// Select with filter
    #[tokio::test]
    async fn a40_query_with_filter() -> anyhow::Result<()> {
        init();
        let r = init_pool_once().await;
        // init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;

        let mut params = HashMap::new();
        params.insert("p_id".to_owned(), CellValue::from_raw_int(2000));

        let query = SQLQueryBlock2 {
            sql_query: "SELECT id, title, isbn, created_dt, precision_time  FROM public.book  WHERE id < :p_id + 10 ORDER BY title"
                .to_string(),
            start: 0,
            length: None,
            params: params,
        };

        let mut sql_result = query.execute(&mut trans).await?;
        trans.commit().await?;

        while sql_result.next() {
            let id = sql_result.get_int_32("id");
            let title = sql_result.get_string("title");
            let isbn = sql_result.get_string("isbn");
            let created_dt = sql_result.get_naivedate("created_dt");
            let precision_time = sql_result.get_timestamp("precision_time");

            if let Some(val) = id {
                println!(
                    "ID = {} title={:?} created_dt = {:?} precision_time = {:?}",
                    val, title, created_dt, precision_time
                );
                assert!(true);
            } else {
                assert!(false);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn a50_query_with_filter_offset_limit() -> anyhow::Result<()> {
        let r = init_pool_once().await;
        // init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;

        let mut params = HashMap::new();
        params.insert("p_id".to_owned(), CellValue::from_raw_int(2000));

        let query = SQLQueryBlock2 {
            sql_query: "SELECT id, title, isbn, created_dt, precision_time  FROM public.book  \
                                WHERE id < :p_id + 10 ORDER BY id"
                .to_string(),
            start: 3,
            length: Some(5),
            params: params,
        };

        let mut sql_result = query.execute(&mut trans).await?;
        trans.commit().await?;

        assert_eq!(5, sql_result.len());

        Ok(())
    }

    /// An incorrect SQL query
    #[tokio::test]
    async fn a50_query_syntax_error() -> anyhow::Result<()> {
        init();
        let r = init_pool_once().await;
        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;
        let mut params = HashMap::new();
        params.insert("p_title".to_owned(), CellValue::from_raw_str("Game"));

        let query = SQLQueryBlock2 {
            sql_query: "SELECT idd, title, isbn, created_dt, precision_time FROM public.book WHERE title LIKE ':p_title%' "
                .to_string(),
            start: 0,
            length: None,
            params: params,
        };

        match query.execute(&mut trans).await {
            Ok(sql_result) => {
                assert!(false);
            }
            Err(e) => {
                println!("ERROR : {}", e);
            }
        }
        let _ = trans.commit().await?;
        Ok(())
    }

    async fn task_for_parallel(thread_number: i32) -> anyhow::Result<()> {
        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.begin().await?;

        let mut params = HashMap::new();
        params.insert(
            "p_title".to_owned(),
            CellValue::from_raw_str(format!("{} All games", thread_number).as_str()),
        );
        params.insert("p_isbn".to_owned(), CellValue::from_opt_str(None));

        let dt = NaiveDate::from_ymd(2024, 8, 15);
        params.insert("p_created_dt".to_owned(), CellValue::from_raw_naivedate(dt));
        params.insert(
            "p_precision_time".to_owned(),
            CellValue::from_raw_systemtime(SystemTime::now()),
        );

        let query = SQLChange2 {
            sql_query:
            "INSERT INTO public.book (id, title, isbn, created_dt, precision_time) VALUES(nextval('book_id_seq'),\
             :p_title, :p_isbn, :p_created_dt, :p_precision_time)"
                .to_string(),
            params: params,
            sequence_name: "book_id_seq".to_string(),
        };

        let pk_id = query.insert(&mut trans).await?;

        trans.commit().await?;
        println!("The pk is: {}", pk_id);
        Ok(())
    }

    #[tokio::test]
    async fn p10_insert_with_sequence_multi() -> anyhow::Result<()> {
        init();
        let r = init_pool_once().await;

        let mut handles = vec![];
        for i in 1..=5 {
            let thread_number = i.clone();

            let handle: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
                // Do some async work
                let r = task_for_parallel(thread_number).await;
                Ok(())
            });
            handles.push(handle);
        }

        // Attendre que toutes les tâches se terminent
        for handle in handles {
            let _ = handle.await;
        }

        println!("Toutes les tâches sont terminées.");

        Ok(())
    }
}
