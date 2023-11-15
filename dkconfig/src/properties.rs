use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::RwLock;
use anyhow::anyhow;
use lazy_static::*;
use commons_error::*;

lazy_static! {
    static ref PROPS : RwLock<HashMap<u32, &'static mut HashMap<String,String>> > = RwLock::new(
        {
            let mut m = HashMap::new();
            let props : HashMap<String,String> = HashMap::new();
            m.insert(0, Box::leak(Box::new( props )));
            m
        });
}

// "app.customerfile"
pub fn get_prop_value(prop_name : &str) -> anyhow::Result<String> {
    // https://doc.rust-lang.org/std/sync/struct.RwLock.html
    let v = PROPS.read().unwrap().deref().get(&0).ok_or(anyhow!("Shared map not found: [{}]", prop_name))?.deref()
        .get(prop_name).ok_or(anyhow!("Prop not found: [{}]", prop_name))?.trim().to_owned();
    Ok(v)
}

// TODO propagate the possible errors
pub fn set_prop_values(props : HashMap<String, String>) {
    // https://doc.rust-lang.org/std/sync/struct.RwLock.html

    let mut w = PROPS.write().unwrap();
    let item = w.get_mut(&0).unwrap();
    *item = Box::leak(Box::new(props ));
}

//
pub fn set_prop_value(prop_name : &str, value : &str ) {
    if let Ok(write_guard) = PROPS.write().as_mut() {
        // the returned write_guard implements `Deref` giving us easy access to the target value

        let map = write_guard.deref_mut();
        if  let Some( item ) = map.get_mut(&0) {
            item.insert(prop_name.to_string(), value.to_string());
        }
    }
}

///
/// Return the connect string and the pool size
///
pub fn get_prop_pg_connect_string() -> anyhow::Result<(String,u32)> {
    let db_hostname = get_prop_value("db.hostname").map_err(tr_fwd!())?;
    let db_port = get_prop_value("db.port").map_err(tr_fwd!())?;
    let db_name = get_prop_value("db.name").map_err(tr_fwd!())?;
    let db_user = get_prop_value("db.user").map_err(tr_fwd!())?;
    let db_password = get_prop_value("db.password").map_err(tr_fwd!())?;
    let db_pool_size = get_prop_value("db.pool_size")?.parse::<u32>().map_err(err_fwd!("Cannot read the pool size"))?;
    let cs = format!("host={} port={} dbname={} user={} password={}", db_hostname, db_port, db_name, db_user,db_password);
    Ok((cs, db_pool_size))
}