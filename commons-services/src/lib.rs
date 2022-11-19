

use std::path::Path;
use std::process::exit;
use dkconfig::conf_reader::cek_read_once;
use dkconfig::properties::{get_prop_value, set_prop_value};
use log::error;
use commons_error::*;
use crate::property_name::{COMMON_EDIBLE_KEY_FILE_PROPERTY, COMMON_EDIBLE_KEY_PROPERTY};

pub mod session_lib;
pub mod token_lib;
pub mod database_lib;
pub mod key_lib;
pub mod property_name;
pub mod x_request_id;


pub fn read_cek_and_store() {
    let Ok(cek_file) = get_prop_value(COMMON_EDIBLE_KEY_FILE_PROPERTY) else {
        log_error!("Cannot read CEK");
        exit(-28);
    };
    let cek = match cek_read_once(Path::new(&cek_file), false) {
        Ok(s) => {s}
        Err(e) => {
            log_error!("{:?} {:?}", &cek_file, e);
            exit(-29);
        }
    };
    set_prop_value(COMMON_EDIBLE_KEY_PROPERTY, &cek);
}

