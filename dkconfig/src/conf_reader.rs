
use std::env;
use std::path::Path;
use std::process::exit;
use std::fs::{File, read_to_string, remove_file};
use std::io::BufReader;
use std::collections::HashMap;
use java_properties::read;
use commons_error::*;

//
pub fn cek_read_once(cek_file : &Path, is_edible: bool) -> anyhow::Result<String> {

    let cek = read_to_string(&cek_file).map_err(
        err_fwd!("Cannot open CEK file, filename=[{}]", cek_file.to_str().unwrap().to_owned())
    )?;

    if is_edible {
        remove_file(&cek_file).map_err(
            err_fwd!("Unknown CEK file error, filename=[{}]", cek_file.to_str().unwrap().to_owned())
        )?;
    }

    Ok(cek)
}


//
pub fn read_config( project_code : &str, var_name : &str ) -> HashMap<String, String> {

    let doka_env = match env::var(var_name) {
        Ok(env) => env,
        Err(e) => {
            eprintln!("💣 Cannot find the DOKA_ENV system variable, {}", e);
            exit(-99);
        },
    };

    let config_path = Path::new(&doka_env).join(project_code).join("config/application.properties");

    let f = match File::open(&config_path) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("💣 Cannot find the configuration file, e={}", e);
            exit(-89);
        }
    };

    let props = match read(BufReader::new(f)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("💣 Cannot read the configuration file, e={}", e);
            exit(-79);
        }
    };

    eprintln!("Configuration file : props={:?}", &props);

    props
}
