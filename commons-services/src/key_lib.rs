use log::error;

use commons_error::*;
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use doka_cli::async_request_client::KeyManagerClientAsync;

use crate::COMMON_EDIBLE_KEY_PROPERTY;
use crate::property_name::{KEY_MANAGER_HOSTNAME_PROPERTY, KEY_MANAGER_PORT_PROPERTY};
use crate::x_request_id::Follower;

///
/// Find the customer key if any
///
pub async fn fetch_customer_key(
    customer_code: &str,
    follower: &Follower,
) -> anyhow::Result<String> {
    let sid = &follower.token_type.value();
    let key_not_found = anyhow::anyhow!("Cannot find the customer key");

    // Get the crypto key
    let km_host = get_prop_value(KEY_MANAGER_HOSTNAME_PROPERTY).map_err(tr_fwd!())?;
    let km_port: u16 = get_prop_value(KEY_MANAGER_PORT_PROPERTY)?
        .parse()
        .map_err(tr_fwd!())?;
    let kmc = KeyManagerClientAsync::new(&km_host, km_port);

    match kmc.get_key(customer_code, &sid).await {
        Ok(customer_key_reply) => {
            let customer_key = customer_key_reply
                .keys
                .get(customer_code)
                .ok_or(key_not_found)?
                .ciphered_key
                .as_str();

            // The key we receive from the Key manager is master-encrypted
            let cek = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY).map_err(tr_fwd!())?;

            let clear_customer_key = DkEncrypt::decrypt_str(&customer_key, &cek)
                .map_err(err_fwd!("Cannot decrypt the customer key"))?;

            Ok(clear_customer_key)
        }
        Err(e) => {
            log_error!(
                "Key Manager failed with status [{}], follower=[{}]",
                e.message,
                &follower
            );
            return Err(key_not_found);
        }
    }
}
