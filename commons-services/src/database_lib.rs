use crate::try_or_return;
use crate::x_request_id::Follower;
use anyhow::anyhow;
use commons_error::*;
use commons_pg::sql_transaction::{SQLConnection, SQLTransaction};
use commons_pg::sql_transaction2::{SQLConnection2, SQLTransaction2};
use dkdto::error_codes::INTERNAL_DATABASE_ERROR;
use dkdto::{WebResponse, WebTypeBuilder};
use tokio::sync::oneshot;

///
/// Start a new database transaction
///
pub fn open_transaction(
    r_cnx: &'_ mut anyhow::Result<SQLConnection>,
) -> anyhow::Result<SQLTransaction<'_>> {
    let cnx = match r_cnx
        .as_mut()
        .map_err(err_fwd!("Fail opening db connection"))
    {
        Ok(x) => x,
        Err(_) => {
            return Err(anyhow::anyhow!("_"));
        }
    };
    let trans = cnx
        .sql_transaction()
        .map_err(err_fwd!("Fail starting a transaction"))?;
    Ok(trans)
}

// pub async fn open_transaction2<'a>() -> anyhow::Result<SQLTransaction2<'a>> {
//     let mut cnx = SQLConnection2::from_pool()
//         .await
//         .map_err(err_fwd!("💣 Open connection error"))?;
//
//     let mut trans = cnx
//         .sql_transaction()
//         .await
//         .map_err(err_fwd!("💣 Open transaction error"))?;
//
//     Ok(trans)
// }

pub async fn run_blocking_spawn<R, F>(f: F, follower: &Follower) -> WebResponse<R>
where
    R: Send + 'static,
    F: FnOnce() -> WebResponse<R> + Send + 'static,
{
    // Create a oneshot channel for one-way communication
    let (tx, rx) = oneshot::channel();
    let g = move || {
        let r = f();
        // Send the user object back to the main thread
        let _ = tx.send(r);
    };

    tokio::task::spawn_blocking(g);

    rx.await
        .map_err(err_fwd!(
            "💣 Thread receive data error, follower=[{}]",
            &follower
        ))
        .unwrap()
}
