use log::error;
use commons_error::*;

use dkconfig::properties::get_prop_value;
use dkdto::{EntrySession};
use doka_cli::request_client::{SessionManagerClient};
use crate::property_name::{ SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY};


pub fn fetch_entry_session(sid : &str) -> anyhow::Result<EntrySession> {
    let sm_host = get_prop_value(SESSION_MANAGER_HOSTNAME_PROPERTY).map_err(tr_fwd!())?;
    let sm_port : u16 = get_prop_value(SESSION_MANAGER_PORT_PROPERTY)?.parse().map_err(tr_fwd!())?;
    let smc = SessionManagerClient::new(&sm_host, sm_port);
    // For now the token is the sid itself
    match smc.get_session(sid, sid) {
        Ok(session_reply) => {
            let ref_entry_session : &EntrySession = session_reply.sessions.get(0).ok_or(anyhow::anyhow!("Cannot find the session"))?;
            let entry_session = ref_entry_session.clone();
            Ok(entry_session)
        }
        Err(e) => {
            log_error!("Session Manager failed with status [{:?}]", e);
            return Err(anyhow::anyhow!("{} - {}", e.http_error_code, e.message));
        }
    }



}

