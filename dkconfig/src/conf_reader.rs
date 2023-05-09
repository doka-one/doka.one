
use std::env;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::fs::{File, read_to_string, remove_file};
use std::io::BufReader;
use std::collections::HashMap;
use anyhow::anyhow;
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


// Read the configuration file from the project code and the environment variable
pub fn read_config( project_code : &str, var_name : &str ) -> HashMap<String, String> {
    let doka_env = match env::var(var_name) {
        Ok(env) => env,
        Err(e) => {
            eprintln!("💣 Cannot find the DOKA_ENV system variable, {}", e);
            exit(99);
        },
    };

    let config_path = Path::new(&doka_env).join(project_code).join("config/application.properties");

    let Ok(props) = read_config_from_path(&config_path) else {
        exit(89);
    };

    props
}

// Read the configuration file from a direct path
pub fn read_config_from_path( config_path: &PathBuf ) -> anyhow::Result<HashMap<String, String>> {

    let f = match File::open(&config_path) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("💣 Cannot open the configuration file, e={}", e);
            return Err(anyhow!("Cannot open the configuration file: [{:#?}]", &config_path));
        }
    };

    let props = match read(BufReader::new(f)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("💣 Cannot read the configuration file, e={}", e);
            return Err(anyhow!("Cannot read the configuration file"));
        }
    };

    println!("Configuration file : props={:?}", &props);

    Ok(props)
}
