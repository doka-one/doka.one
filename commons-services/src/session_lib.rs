use log::error;
use commons_error::*;

use dkconfig::properties::get_prop_value;
use dkdto::{EntrySession, ErrorSet};
use dkdto::error_codes::{INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN};
use doka_cli::request_client::{SessionManagerClient, TokenType};
use crate::property_name::{ SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY};
use crate::token_lib::SessionToken;
use crate::x_request_id::Follower;


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


pub fn valid_sid_get_session(session_token: &SessionToken, follower: &mut Follower) -> Result<String, ErrorSet<'static>> {
    // Check if the token is valid
    if !session_token.is_valid() {
        log_error!("💣 Invalid session token, token=[{:?}], follower=[{}]", &session_token, &follower);
        return Err(INVALID_TOKEN);
    }

    follower.token_type = TokenType::Sid(session_token.0.clone());

    // Read the session information
    let Ok(entry_session) = fetch_entry_session(&follower.token_type.value()).map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &follower)) else {
        return Err(INTERNAL_TECHNICAL_ERROR);
    };
    let customer_code = entry_session.customer_code.as_str();
    Ok(customer_code.to_owned())
}
