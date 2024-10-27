use std::collections::HashMap;
use std::env;
use std::fs::{File, read_to_string, remove_file};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::exit;

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

pub fn read_doka_env( var_name: &str ) -> String {
    let mut doka_env: Option<String> = None;
    let args: Vec<String> = env::args().collect();
    let mut index = 1;
    while index < args.len() {
        let v = &args[index];
        match v.as_str() {
            "--doka-env" => {
                if let Some(k) = args.get(index + 1) {
                    doka_env = Some(k.clone());
                }
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }

    if doka_env.is_none() {
        doka_env = match env::var(var_name) {
            Ok(env) => Some(env),
            Err(e) => {
                eprintln!("💣 Cannot find the DOKA_ENV system variable, {}", e);
                exit(99);
            },
        };
    }

    doka_env.unwrap()
}

// Read the configuration file from the project code and the environment variable
pub fn read_config( project_code : &str, doka_env_folder : &str ) -> HashMap<String, String> {

    let config_path = Path::new(&doka_env_folder).join(project_code).join("config/application.properties");

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
