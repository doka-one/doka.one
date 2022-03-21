use commons_pg::{SQLConnection, SQLTransaction};
use commons_error::*;

///
/// Start a new database transaction
///
pub fn open_transaction( r_cnx: &'_ mut anyhow::Result<SQLConnection>) -> anyhow::Result<SQLTransaction<'_>> {
    let cnx = match r_cnx.as_mut().map_err(err_fwd!("Fail opening db connection")) {
        Ok(x) => {x}
        Err(_) => {
            return Err(anyhow::anyhow!("_"));
        }
    };
    let trans = cnx.sql_transaction().map_err(err_fwd!("Fail starting a transaction"))?;
    Ok(trans)
}