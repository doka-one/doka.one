use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use chrono::{DateTime, NaiveDateTime, Utc};
use lazy_static::*;

use mut_static::MutStatic;
use postgres_types::ToSql;
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgArguments, PgPoolOptions};
use sqlx::{Arguments, Connection, Encode, Pool, Postgres, Transaction, Type};

use commons_error::*;

use crate::sql_transaction::{parse_query, CellValue};

lazy_static! {
    static ref SQL_POOL2: MutStatic<SQLPool2> = MutStatic::new();
}

pub async fn init_db_pool2(connect_string: &str, pool_size: u32) -> anyhow::Result<()> {
    let pool = match SQLPool2::new(connect_string, pool_size)
        .await
        .map_err(err_fwd!("Cannot create the DB pool"))
    {
        Ok(p) => p,
        Err(_) => return Err(anyhow!("_")),
    };
    match SQL_POOL2
        .set(pool)
        .map_err(err_fwd!("Cannot create the static pool"))
    {
        Ok(_) => {}
        Err(_) => return Err(anyhow!("_")),
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
            .min_connections(pool_size) // Taille minimale du pool
            .max_connections(pool_size + 5) // Taille maximale du pool
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
        let sql_pool = match SQL_POOL2.read().map_err(err_fwd!("*")) {
            Ok(p) => p,
            Err(_) => return Err(anyhow!("_")),
        };
        let client = sql_pool.pick_connection().await?;
        Ok(SQLConnection2 { client })
    }

    pub async fn sql_transaction<'a>(&'a mut self) -> anyhow::Result<SQLTransaction2<'a>> {
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

// type PgParamsType<'a> = &'a (dyn sqlx::types::Type<Postgres> + sqlx::Encode<'a, Postgres>);

//pub type PgParam<'a> = &'a (dyn Type<Postgres> + Encode<'a, Postgres> + Sync);

// enum PgParam {
//     Int(i32),
//     Text(String),
// }
//
// impl PgParam {
//     fn bind_to_query<'q>(
//         self,
//         query_builder: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
//     ) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
//         match self {
//             PgParam::Int(value) => query_builder.bind(value),
//             PgParam::Text(value) => query_builder.bind(value),
//         }
//     }
// }

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
    pub async fn execute(
        &self,
        sql_transaction: &mut SQLTransaction2<'_>,
    ) -> anyhow::Result<() /*SQLDataSet*/> {
        let null_str = "".to_owned();
        let (mut new_sql_string, v_params) =
            parse_query2(self.sql_query.as_str(), &self.params, &null_str);

        let mut query_builder = sqlx::query(new_sql_string.as_str());

        for param in v_params {
            query_builder = bind_cell_to_query(param, query_builder);
        }

        let r = query_builder
            .execute(&mut *sql_transaction.inner_transaction)
            .await?;

        // let result_set = sql_transaction
        //     .inner_transaction
        //     .query(new_sql_string.as_str(), v_params.as_slice())
        //     .map_err(err_fwd!(
        //         "Sql query failed, sql [{}]",
        //         new_sql_string.as_str()
        //     ))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    use sqlx::postgres::PgPoolOptions;
    use sqlx::{query, Acquire, PgConnection, Row, Transaction};

    use crate::sql_transaction::CellValue;
    use crate::sql_transaction2::{init_db_pool2, SQLConnection2, SQLQueryBlock2, SQLTransaction2};

    async fn create(trans: &mut PgConnection, title: &str) -> anyhow::Result<()> {
        let query = "INSERT INTO public.book (id, title) VALUES(nextval('book_id_seq'), $1)";
        let r = sqlx::query(query).bind(title).execute(trans).await?;
        Ok(())
    }

    #[tokio::test]
    async fn a10_raw_sqlx_connection() -> anyhow::Result<()> {
        let url = "postgres://doka:doka@localhost:5432/ad_test_03";

        // Configure le pool de connexions
        let pool = PgPoolOptions::new()
            .max_connections(5) // Taille maximale du pool
            .min_connections(1) // Taille minimale du pool
            .idle_timeout(Duration::from_secs(30)) // Timeout pour se connecter
            .idle_timeout(Some(Duration::from_secs(10))) // Timeout d'inactivité des connexions
            .max_lifetime(Some(Duration::from_secs(300))) // Durée de vie maximale d'une connexion
            .connect(url)
            .await?;

        // Acquérir une connexion individuelle à partir du pool
        let mut cnx = pool.acquire().await?;

        // let cnx = sqlx::postgres::PgPool::connect(url).await?;

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

    #[tokio::test]
    async fn a15_cnx_and_trans() -> anyhow::Result<()> {
        init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.sql_transaction().await?;

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

    #[tokio::test]
    async fn a20_simple_query() -> anyhow::Result<()> {
        init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.sql_transaction().await?;

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

    #[tokio::test]
    async fn a22_simple_insert() -> anyhow::Result<()> {
        init_db_pool2("postgres://doka:doka@localhost:5432/ad_test_03", 3).await?;

        let mut cnx = SQLConnection2::from_pool().await?;
        let mut trans = cnx.sql_transaction().await?;

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
}
