use log::error;
use commons_error::*;

use dkconfig::properties::get_prop_value;
use dkdto::EntrySession;
use doka_cli::request_client::{SessionManagerClient};
use crate::property_name::{ SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY};


pub fn fetch_entry_session(sid : &str) -> anyhow::Result<EntrySession> {
    let sm_host = get_prop_value(SESSION_MANAGER_HOSTNAME_PROPERTY).map_err(err_fwd!(""))?;
    let sm_port : u16 = get_prop_value(SESSION_MANAGER_PORT_PROPERTY)?.parse().map_err(err_fwd!(""))?;
    let smc = SessionManagerClient::new(&sm_host, sm_port);
    // For now the token is the sid itself
    let response = smc.get_session(sid, sid);

    if response.status.error_code != 0 || response.sessions.is_empty() {
        log_error!("Session Manager failed with status [{:?}]", &response.status);
        return Err(anyhow::anyhow!("_"));
    }

    let ref_entry_session : &EntrySession = response.sessions.get(0).ok_or(anyhow::anyhow!("Cannot find the session"))?;
    let entry_session = ref_entry_session.clone();

    Ok(entry_session)
}

