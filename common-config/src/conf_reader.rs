use std::collections::HashMap;
use std::env;
use std::fs::{read_to_string, remove_file, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::exit;

use anyhow::anyhow;
use java_properties::read;
use commons_error::*;
use serde_derive::Deserialize;

//
pub fn cek_read_once(cek_file: &Path, is_edible: bool) -> anyhow::Result<String> {
    let cek = read_to_string(&cek_file)
        .map_err(err_fwd!("Cannot open CEK file, filename=[{}]", cek_file.to_str().unwrap().to_owned()))?;

    if is_edible {
        remove_file(&cek_file)
            .map_err(err_fwd!("Unknown CEK file error, filename=[{}]", cek_file.to_str().unwrap().to_owned()))?;
    }

    Ok(cek)
}

/// Read the cluster profile value from the environment variable
/// Used to read the DOKA_CLUSTER_PROFILE variable
pub fn read_cluster_profile_from_env(var_name: &str) -> Option<String> {
    match env::var(var_name) {
        Ok(env) => Some(env),
        Err(e) => {
            eprintln!("🚫 Cannot find the {} system variable: {}", var_name, e);
            None
        }
    }
}

/// Read the doka env value
/// It's the path where we can find the properties for the service
pub fn read_env(var_name: &str) -> Option<String> {
    let mut doka_env: Option<String> = None;
    let args: Vec<String> = env::args().collect();
    let mut index = 1;

    // Parse command-line arguments to look for "--env"
    while index < args.len() {
        let v = &args[index];
        match v.as_str() {
            "--env" => {
                if let Some(k) = args.get(index + 1) {
                    doka_env = Some(k.clone());
                }
                index += 2; // Skip the value after "--env"
            }
            _ => {
                index += 1;
            }
        }
    }

    // If no "--env" argument is found, check the environment variable
    if doka_env.is_none() {
        doka_env = match env::var(var_name) {
            Ok(env) => Some(env),
            Err(e) => {
                eprintln!("🚫 Cannot find the {} system variable: {}", var_name, e);
                None
            }
        };
    }

    doka_env
}

/// Read the cluster profile value from the command line
/// For example, "--cluster-profile dev_02"
pub fn read_cluster_profile(cluster_var_name: &Option<String>) -> Option<String> {
    let mut cluster_profile: Option<String> = None;
    let args: Vec<String> = env::args().collect();
    let mut index = 1;

    // Parse command-line arguments to look for "--cluster-profile"
    while index < args.len() {
        let v = &args[index];
        match v.as_str() {
            "--cluster-profile" => {
                if let Some(k) = args.get(index + 1) {
                    cluster_profile = Some(k.clone());
                }
                index += 2; // Skip the value after "--cluster-profile"
            }
            _ => {
                index += 1;
            }
        }
    }

    if cluster_profile.is_none() {
        cluster_profile = read_cluster_profile_from_env(cluster_var_name.as_ref().unwrap());
    }

    cluster_profile
}

/// Read the configuration file from the project code and the environment variable
///  * If the o_config_file is defined, we take the property file from it.
///  * If not, we read the .xxx-config.json file from the user's base folder (or the argument --config-file)
///       and where the services are defined
pub fn read_config( project_code: &str, o_config_file: &Option<String>, cluster_var_name: &Option<String>,) -> HashMap<String, String> {
    let config_file = match o_config_file {
        None => dirs::home_dir().expect("Failed to determine home directory").join(".doka-config.json"),
        Some(config_file) => {
            println!("🔧 Using the config file from the environment variable : {}", config_file);
            Path::new(config_file).to_path_buf()
        }
    };

    let (property_file, constants, properties) = if config_file.exists() {
        let cluster_configs = match read_cluster_configs(&config_file) {
            Ok(props) => props,
            Err(_) => {
                eprintln!("💣 Failed to read config from path: {:?}", config_file);
                exit(70);
            }
        };

        let profile_name = read_cluster_profile(&cluster_var_name).unwrap_or_else(|| {
            eprintln!("💣 Cluster profile name not found");
            exit(60);
        });

        let cluster = cluster_configs.clusters.iter().find(|c| c.name == profile_name).unwrap_or_else(|| {
            eprintln!("💣 Cluster with profile name '{}' not found", profile_name);
            exit(10);
        });

        let os = std::env::consts::OS;
        let o_system_constants = match os {
            "windows" => cluster.constants_windows.as_ref(),
            "linux" => cluster.constants_linux.as_ref(),
            _ => None,
        };

        let mut constants = cluster.constants.map.clone();

        // Complete the constants list with the constants for the OS
        if let Some(os_constants) = o_system_constants {
            for (k, v) in &os_constants.map {
                constants.insert(k.clone(), v.clone());
            }
        }

        let service = cluster.services.iter().find(|s| s.name == project_code).unwrap_or_else(|| {
            eprintln!("Service with name '{}' not found", project_code);
            exit(20);
        });

        let property_file = service.property_file.clone();
        let mut properties = service.properties.map.clone();

        // Complete the list of properties with the common properties
        match &cluster.properties {
            None => {}
            Some(cp) => {
                for (k,v) in &cp.map {
                    properties.insert(k.clone(), v.clone());
                }
            }
        }

        let resolved_property_file = replace_value_with_constants(&property_file, &Some(constants.clone()));
        (Path::new(&resolved_property_file).to_path_buf(), Some(constants), Some(properties))
    } else {
        eprintln!("💣 Config file [{}] does not exist", config_file.to_str().unwrap());
        exit(120);
    };
    let Ok(props) = read_config_from_path(&property_file, &constants, &properties) else {
        exit(100);
    };
    props
}

/// Struct matching the JSON structure for cluster configurations

#[derive(Deserialize, Debug)]
pub struct ClusterConfigs {
    pub clusters: Vec<ClusterConfig>,
}

#[derive(Deserialize, Debug)]
pub struct ClusterConfig {
    pub description: Option<String>,
    pub name: String,
    pub constants: Constants,
    pub constants_linux: Option<Constants>,
    pub constants_windows: Option<Constants>,
    pub properties: Option<FlatProps>,
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

    println!("Successfully read configuration");

    Ok(configs)
}

/// Read the configuration file from a direct path
/// It will first read the props from the application.properties files, then override the values with those in the .doka-config.json
/// and eventually process the interpolation with the constant value.
pub fn read_config_from_path(property_file: &PathBuf, constants: &Option<HashMap<String, String>>, properties: &Option<HashMap<String, String>>,
) -> anyhow::Result<HashMap<String, String>> {
    println!("Read the properties from the file : {}", property_file.to_str().unwrap_or("Not found"));

    let props = match File::open(&property_file) {
        Ok(f) => read(BufReader::new(f)).unwrap_or_else(|e| {
            eprintln!("💣 Cannot read the configuration file, e={}", e);
            HashMap::new()
        }),
        Err(e) => {
            eprintln!("Cannot open the property file, let's continue, e={}", e);
            HashMap::new()
        }
    };

    // Override the value with the one from the doka-config file
    let overridden_props = match properties {
        None => props,
        Some(p) => {
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

    println!("Resolved properties: {:?}", resolved_props.keys());

    Ok(resolved_props)
}

/// Replaces a single property's value by substituting constants.
fn replace_value_with_constants(value: &str, constants: &Option<HashMap<String, String>>) -> String {
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
