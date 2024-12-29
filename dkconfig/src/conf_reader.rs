use std::collections::HashMap;
use std::env;
use std::fs::{File, read_to_string, remove_file};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::exit;

use anyhow::{anyhow, Context};
use java_properties::read;
use serde_derive::Deserialize;
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

/// Read the doka env value
/// It's the path where we can find the properties for the service
pub fn read_doka_env(var_name: &str) -> Option<String> {
    let mut doka_env: Option<String> = None;
    let args: Vec<String> = env::args().collect();
    let mut index = 1;

    // Parse command-line arguments to look for "--doka-env"
    while index < args.len() {
        let v = &args[index];
        match v.as_str() {
            "--doka-env" => {
                if let Some(k) = args.get(index + 1) {
                    doka_env = Some(k.clone());
                }
                index += 2; // Skip the value after "--doka-env"
            }
            _ => {
                index += 1;
            }
        }
    }

    // If no "--doka-env" argument is found, check the environment variable
    if doka_env.is_none() {
        doka_env = match env::var(var_name) {
            Ok(env) => Some(env),
            Err(e) => {
                eprintln!("💣 Cannot find the DOKA_ENV system variable: {}", e);
                None
            },
        };
    }

    doka_env
}

/// Read the configuration file from the project code and the environment variable
///  * If the doka_env_folder is defined, we take the property file from it.
///  * If not, we read the .doka-config.json file from the user's base folder (or the argument --doka-config)
///       and where the services are defined
pub fn read_config(project_code: &str, doka_env_folder: &Option<String>) -> HashMap<String, String> {
    // TODO repalce the arg  doka-env with the path to the .doka-config.json !!!!

    // V2
    let (property_file, constants, properties) = if let Some(folder) = doka_env_folder {
        // V1
        println!("😵‍💫 Be careful, V1 mode!!!!");
        (Path::new(folder).join(project_code).join("config/application.properties"), None, None)
    } else {
        // V2
        let config_file =  dirs::home_dir()
            .expect("Failed to determine home directory")
            .join(".doka-config.json");

        let cluster_configs = match read_cluster_configs(&config_file) {
            Ok(props) => props,
            Err(_) => {
                eprintln!("Failed to read config from path: {:?}", config_file);
                exit(89);
            }
        };

        let constants = cluster_configs.clusters.get(0).unwrap().constants.map.clone();
        let property_file = cluster_configs.clusters.get(0).unwrap().services.get(0).unwrap().property_file.clone();
        // TODO manage errors and take the property of the service "PROJECT_CODE"
        let properties = cluster_configs.clusters.get(0).unwrap().services.get(0).unwrap().properties.map.clone();
        let resolved_property_file = replace_value_with_constants(&property_file, &Some(constants.clone()));
        (Path::new(&resolved_property_file).to_path_buf(), Some(constants), Some(properties))
    };


    let Ok(props) = read_config_from_path(&property_file, &constants, &properties) else {
        exit(100);
    };

    props
}

/// Struct matching the JSON structure for cluster configurations

#[derive(Deserialize, Debug)]
pub struct ClusterConfigs {
    pub clusters: Vec<ClusterConfig>
}

#[derive(Deserialize, Debug)]
pub struct ClusterConfig {
    pub description: Option<String>,
    pub name: String,
    pub constants: Constants,
    pub services: Vec<Service>,
}

#[derive(Deserialize, Debug)]
pub struct Constants {
    #[serde(flatten)]
    pub map: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
pub struct FlatProps {
    #[serde(flatten)]
    pub map: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
pub struct Service {
    pub description: Option<String>,
    pub name: String,
    pub property_file: String,
    pub properties: FlatProps,
}

/// Reads the cluster configuration from a JSON file
pub fn read_cluster_configs(file_path: &Path) -> anyhow::Result<ClusterConfigs> {
    println!("Reading the properties from the file: {}", file_path.to_str().unwrap_or("Not found"));


    // Open the file
    let mut file = File::open(file_path).map_err(|e| {
        eprintln!("💣 Cannot open the configuration file: {:?}", e);
        anyhow!("Cannot open the configuration file: {:?}", file_path)
    })?;

    // Read the file contents into a string
    let mut json_data = String::new();
    file.read_to_string(&mut json_data).map_err(|e| {
        eprintln!("💣 Cannot read the configuration file: {:?}", e);
        anyhow!("Cannot read the configuration file: {:?}", file_path)
    })?;

    // Parse the JSON string into a ClusterConfigs struct
    let configs: ClusterConfigs = serde_json::from_str(&json_data).map_err(|e| {
        eprintln!("💣 Failed to parse JSON file: {:?}", e);
        anyhow!("Failed to parse JSON file into ClusterConfigs: {:?}", file_path)
    })?;

    println!("Successfully read configuration: {:?}", configs);

    Ok(configs)

}


/// Read the configuration file from a direct path
/// It will first read the props from the application.properties files, then override the values with those in the .doka-config.json
/// and eventually process the interpolation with the constant value.
pub fn read_config_from_path(property_file: &PathBuf, constants:  &Option<HashMap<String, String>>, properties:  &Option<HashMap<String, String>>) -> anyhow::Result<HashMap<String, String>> {

    println!("Read the properties from the file : {}", property_file.to_str().unwrap_or("Not found"));

    let props = match File::open(&property_file) {
        Ok(f) => {
            read(BufReader::new(f)).unwrap_or_else(|e| {
                eprintln!("💣 Cannot read the configuration file, e={}", e);
                HashMap::new()
            })
        },
        Err(e) => {
            eprintln!("💣 Cannot open the property file, e={}", e);
            HashMap::new()
        }
    };

    println!("Configuration file : props={:?}", &props);
    println!("New properties : {:?}", &properties);
    // Override the value with the one from the doka-config file
    let  overridden_props = match properties {
        None => {props}
        Some(p) => {
            // let mut combined = props
            //     .into_iter()
            //     .map(|(key, value)| {
            //         let new_value = p.get(&key).unwrap_or(&value);
            //         (key, new_value.clone())
            //     })
            //     .collect();



            let mut combined_props = props;

            for (key, value) in p {
                combined_props.insert(key.clone(), value.clone());
            }
            combined_props
        }
    };


    // Loop over all the properties and replace the constants in each property
    let resolved_props: HashMap<String, String> = overridden_props
        .into_iter()
        .map(|(key, value)| (key, replace_value_with_constants(&value, constants)))
        .collect();

    println!("Resolved properties: {:?}", resolved_props);

    Ok(resolved_props)
}


/// Replaces a single property's value by substituting constants.
fn replace_value_with_constants(
    value: &str,
    constants: &Option<HashMap<String, String>>,
) -> String {
    let mut resolved_value = value.to_string();

    if let Some(constants_map) = constants {
        for (const_key, const_value) in constants_map {
            let placeholder = format!("${{{}}}", const_key); // Placeholder format: ${KEY}
            if resolved_value.contains(&placeholder) {
                resolved_value = resolved_value.replace(&placeholder, const_value);
            }
        }
    }

    resolved_value
}