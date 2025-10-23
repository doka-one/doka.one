use std::collections::HashMap;
use std::env;
use std::process::exit;

use anyhow::anyhow;

use commons_error::*;
use common_config::conf_reader::{read_config, read_config_from_path, read_env};
use common_config::properties::{get_prop_value, set_prop_values};

use crate::command_options::{display_commands, load_commands, parse_args, Command, Params};
use crate::customer_commands::{create_customer, delete_customer, disable_customer};
use crate::file_commands::{file_download, file_info, file_list, file_loading, file_upload};
use crate::item_commands::{create_item, get_item, item_tag_delete, item_tag_update, search_item};
use crate::session_commands::session_login;
use crate::token_commands::{get_target_file, token_generate};

mod command_options;
mod customer_commands;
mod file_commands;
mod item_commands;
mod session_commands;
mod token_commands;

const PARAMETER_ERROR: u16 = 10;
const LOGIN_SESSION_FAILED: u16 = 30;
const DELETE_CUSTOMER_FAILED: u16 = 40;
const DISABLE_CUSTOMER_FAILED: u16 = 50;
const CREATE_CUSTOMER_FAILED: u16 = 60;
const GENERATE_TOKEN_FAILED: u16 = 80;
const CREATE_ITEM_FAILED: u16 = 90;
const GET_ITEM_FAILED: u16 = 100;
const PROP_ITEM_FAILED: u16 = 101;
const FILE_UPLOAD_FAILED: u16 = 110;
const FILE_DOWNLOAD_FAILED: u16 = 120;
const SUCCESS: u16 = 0;

fn read_configuration_file() -> anyhow::Result<()> {
    let doka_env = read_env("DOKA_CLI_ENV");
    let props = read_config(
        "doka-cli",
        &doka_env,
        &Some("DOKA_CLUSTER_PROFILE".to_string()),
    );

    // let config_path = get_target_file("config/application.properties")?;
    // let config_path_str = config_path.to_str().ok_or(anyhow!("Cannot convert path to str"))?;
    // println!("Define the properties from file : {}", config_path_str);
    // let props = read_config_from_path( &config_path )?;

    set_prop_values(props);

    Ok(())
}

fn extract_mandatory_option(
    options: &HashMap<String, Option<String>>,
    key: &str,
) -> anyhow::Result<String> {
    let opt_value = options
        .get(key)
        .ok_or_else(|| anyhow!("💣 Unknown parameter, option=[{}]", key))?;
    let value = opt_value
        .as_ref()
        .ok_or_else(|| anyhow!("💣 Unknown parameter, option=[{}]", key))?;
    Ok(value.to_owned())
}

fn extract_option(
    options: &HashMap<String, Option<String>>,
    key: &str,
) -> anyhow::Result<Option<String>> {
    let opt_value = options.get(key);
    match opt_value {
        None => Ok(None),
        Some(o_value) => Ok(o_value.to_owned()),
    }
}

fn dispatch(params: &Params, commands: &[Command]) -> u16 {
    match (params.object.as_str(), params.action.as_str()) {
        ("help", "help") => {
            display_commands(commands);
            SUCCESS
        }
        ("token", "generate") => {
            let Ok(cek_file) =
                extract_mandatory_option(&params.options, "-c").map_err(eprint_fwd!("Error"))
            else {
                return PARAMETER_ERROR;
            };
            let err = token_generate(&cek_file);
            success_or_err(err, GENERATE_TOKEN_FAILED)
        }
        ("customer", "create") => {
            let Ok((customer_name, email, admin_password)) =
                (|| -> anyhow::Result<(String, String, String)> {
                    Ok((
                        extract_mandatory_option(&params.options, "-n")?,
                        extract_mandatory_option(&params.options, "-e")?,
                        extract_mandatory_option(&params.options, "-ap")?,
                    ))
                })()
                .map_err(eprint_fwd!("Error"))
            else {
                return PARAMETER_ERROR;
            };
            let err = create_customer(&customer_name, &email, &admin_password);
            success_or_err(err, CREATE_CUSTOMER_FAILED)
        }
        ("customer", "disable") => {
            let Ok(customer_code) =
                extract_mandatory_option(&params.options, "-cc").map_err(eprint_fwd!("Error"))
            else {
                return PARAMETER_ERROR;
            };
            let err = disable_customer(&customer_code);
            success_or_err(err, DISABLE_CUSTOMER_FAILED)
        }
        ("customer", "delete") => {
            let Ok(customer_code) =
                extract_mandatory_option(&params.options, "-cc").map_err(eprint_fwd!("Error"))
            else {
                return PARAMETER_ERROR;
            };
            let err = delete_customer(&customer_code);
            success_or_err(err, DELETE_CUSTOMER_FAILED)
        }
        ("session", "login") => {
            let Ok((user_name, user_password)) = (|| -> anyhow::Result<(String, String)> {
                Ok((
                    extract_mandatory_option(&params.options, "-u")?,
                    extract_mandatory_option(&params.options, "-p")?,
                ))
            })()
            .map_err(eprint_fwd!("Error")) else {
                return PARAMETER_ERROR;
            };
            let err = session_login(&user_name, &user_password);
            success_or_err(err, LOGIN_SESSION_FAILED)
        }
        ("item", "create") => {
            let Ok((item_name, o_file_ref, o_path, o_properties)) =
                (|| -> anyhow::Result<(String, Option<String>, Option<String>, Option<String>)> {
                    Ok((
                        extract_mandatory_option(&params.options, "-n")?,
                        extract_option(&params.options, "-fr")?,
                        extract_option(&params.options, "-pt")?,
                        extract_option(&params.options, "-p")?,
                    ))
                })()
                .map_err(eprint_fwd!("Error"))
            else {
                return CREATE_ITEM_FAILED;
            };
            let err = create_item(
                &item_name,
                o_file_ref.as_deref(),
                o_path.as_deref(),
                o_properties.as_deref(),
            );
            success_or_err(err, CREATE_ITEM_FAILED)
        }
        ("item", "search") => {
            let _err = search_item();
            0
        }
        ("item", "get") => {
            let Ok(id) =
                extract_mandatory_option(&params.options, "-id").map_err(eprint_fwd!("Error"))
            else {
                return PARAMETER_ERROR;
            };
            let err = get_item(&id);
            success_or_err(err, GET_ITEM_FAILED)
        }
        ("item", "tag") => {
            let Ok((id, o_delete_prop, o_add_props)) =
                (|| -> anyhow::Result<(String, Option<String>, Option<String>)> {
                    Ok((
                        extract_mandatory_option(&params.options, "-id")?,
                        extract_option(&params.options, "-d")?,
                        extract_option(&params.options, "-u")?,
                    ))
                })()
                .map_err(eprint_fwd!("Error"))
            else {
                return CREATE_ITEM_FAILED;
            };

            let err = if o_add_props.is_some() {
                item_tag_update(&id, o_add_props.as_deref())
            } else {
                item_tag_delete(&id, o_delete_prop.as_deref())
            };
            success_or_err(err, PROP_ITEM_FAILED)
        }
        ("file", "upload") => {
            let Ok((item_info, path)) = (|| -> anyhow::Result<(String, String)> {
                Ok((
                    extract_mandatory_option(&params.options, "-ii")?,
                    extract_mandatory_option(&params.options, "-pt")?,
                ))
            })()
            .map_err(eprint_fwd!("Error")) else {
                return PARAMETER_ERROR;
            };
            let err = file_upload(&item_info, &path);
            success_or_err(err, FILE_UPLOAD_FAILED)
        }
        ("file", "download") => {
            let Ok((path, file_ref)) = (|| -> anyhow::Result<(String, String)> {
                Ok((
                    extract_mandatory_option(&params.options, "-pt")?,
                    extract_mandatory_option(&params.options, "-fr")?,
                ))
            })()
            .map_err(eprint_fwd!("Error")) else {
                return PARAMETER_ERROR;
            };
            let err = file_download(&path, &file_ref);
            success_or_err(err, FILE_DOWNLOAD_FAILED)
        }
        ("file", "info") => {
            let Ok(file_ref) = (|| -> anyhow::Result<String> {
                Ok(extract_mandatory_option(&params.options, "-fr")?)
            })()
            .map_err(eprint_fwd!("Error")) else {
                return PARAMETER_ERROR;
            };
            let err = file_info(&file_ref);
            success_or_err(err, FILE_DOWNLOAD_FAILED)
        }
        ("file", "list") => {
            let Ok(pattern) = (|| -> anyhow::Result<String> {
                Ok(extract_mandatory_option(&params.options, "-m")?)
            })()
            .map_err(eprint_fwd!("Error")) else {
                return PARAMETER_ERROR;
            };
            let err = file_list(&pattern);
            success_or_err(err, FILE_DOWNLOAD_FAILED)
        }
        ("file", "loading") => {
            let err = file_loading();
            success_or_err(err, FILE_DOWNLOAD_FAILED)
        }
        (_, _) => SUCCESS,
    }
}

fn success_or_err(err: anyhow::Result<()>, err_code: u16) -> u16 {
    if err.is_err() {
        err_code
    } else {
        SUCCESS
    }
}

///
/// dk [object] [action] [options]
///
/// We need a service discovery and/or a proxy to know where the services are located
/// They are potentially on different servers and ports
///
fn main() -> () {
    println!("doka-cli version 0.3.0");

    let args: Vec<String> = env::args().collect();
    let commands = load_commands();

    let params = match parse_args(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("💣 Error while parsing the arguments, err=[{}]", e);
            display_commands(&commands);
            exit_program(80);
        }
    };

    // dbg!(&params);

    match read_configuration_file() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("💣 Error while reading the configuration file, err=[{}]", e);
            exit_program(110);
        }
    }

    let server_host = get_prop_value("server.host").unwrap();
    println!("Server host [{}]", &server_host);

    // main routing

    let exit_code = dispatch(&params, &commands);
    exit_program(exit_code as i32);
}

fn exit_program(code: i32) -> ! {
    println!("Terminated [{}]", code);
    exit(code)
}
