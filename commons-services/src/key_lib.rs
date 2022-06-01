use log::error;

use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use doka_cli::request_client::{KeyManagerClient};
use commons_error::*;
use crate::COMMON_EDIBLE_KEY_PROPERTY;
use crate::property_name::{KEY_MANAGER_HOSTNAME_PROPERTY, KEY_MANAGER_PORT_PROPERTY};

///
/// Find the customer key if any
///
pub fn fetch_customer_key(customer_code : &str, sid : &str) -> anyhow::Result<String> {

    let key_not_found = anyhow::anyhow!("Cannot find the customer key");

    // Get the crypto key
    let km_host = get_prop_value(KEY_MANAGER_HOSTNAME_PROPERTY).map_err(err_fwd!(""))?;
    let km_port : u16 = get_prop_value(KEY_MANAGER_PORT_PROPERTY)?.parse().map_err(err_fwd!(""))?;
    let kmc = KeyManagerClient::new( &km_host, km_port );
    let response = kmc.get_key(customer_code, &sid);

    if response.status.error_code != 0 {
        log_error!("Key Manager failed with status [{:?}]", response.status);
        return Err(key_not_found);
    }

    let customer_key = response.keys.get(customer_code).ok_or(key_not_found)?.ciphered_key.as_str();

    // The key we receive from the Key manager is master-encrypted
    let cek = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY).map_err(err_fwd!(""))?;

    let clear_customer_key = DkEncrypt::decrypt_str(&customer_key, &cek)
                                        .map_err(err_fwd!("Cannot decrypt the customer key"))?;

    Ok(clear_customer_key)
}

