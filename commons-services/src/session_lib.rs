use log::error;

use commons_error::*;
use common_config::properties::get_prop_value;
use common_config::property_name::{SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY};
use dkdto::api_error::ApiError;
use dkdto::error_codes::{INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN};
use dkdto::web_types::EntrySession;
use doka_cli::async_request_client::SessionManagerClientAsync;
use doka_cli::request_client::TokenType;

use crate::token_lib::SessionToken;
use crate::x_request_id::Follower;

pub async fn fetch_entry_session(sid: &str) -> anyhow::Result<EntrySession> {
    let sm_host = get_prop_value(SESSION_MANAGER_HOSTNAME_PROPERTY).map_err(tr_fwd!())?;
    let sm_port: u16 = get_prop_value(SESSION_MANAGER_PORT_PROPERTY)?.parse().map_err(tr_fwd!())?;
    let smc = SessionManagerClientAsync::new(&sm_host, sm_port);
    // For now the token is the sid itself
    match smc.get_session(sid, sid).await {
        Ok(session_reply) => {
            let ref_entry_session: &EntrySession =
                session_reply.sessions.get(0).ok_or(anyhow::anyhow!("Cannot find the session"))?;
            let entry_session = ref_entry_session.clone();
            Ok(entry_session)
        }
        Err(e) => {
            log_error!("Session Manager failed with status [{:?}]", e);
            Err(anyhow::anyhow!("{} - {}", e.http_error_code, e.message))
        }
    }
}

pub async fn valid_sid_get_session(
    session_token: &SessionToken,
    follower: &mut Follower,
) -> Result<EntrySession, &'static ApiError<'static>> {
    if !session_token.is_valid() {
        log_error!("💣 Invalid session token, token=[{:?}], follower=[{}]", &session_token, &follower);
        return Err(&*INVALID_TOKEN); // no clone, no alloc
    }

    follower.token_type = TokenType::Sid(session_token.0.clone());

    let entry_session = match fetch_entry_session(&follower.token_type.value()).await {
        Ok(es) => es,
        Err(e) => {
            log_error!("💣 Session Manager failed, follower=[{}], err={e}", &follower);
            return Err(&*INTERNAL_TECHNICAL_ERROR); // no clone, no alloc
        }
    };

    Ok(entry_session)
}
