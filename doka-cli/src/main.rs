

mod customer_commands;
mod session_commands;
mod item_commands;
mod file_commands;
mod token_commands;
mod command_options;

use std::collections::HashMap;
use std::env;
use std::env::current_exe;

use std::path::{Path, PathBuf};
use std::process::exit;
use anyhow::{anyhow};

use commons_error::*;
use dkconfig::conf_reader::{read_config_from_path};
use dkconfig::properties::{get_prop_value, set_prop_values};
use crate::command_options::{Command, display_commands, load_commands, Params, parse_args};
use crate::customer_commands::{create_customer, delete_customer, disable_customer};
use crate::file_commands::{file_download, file_upload};
use crate::item_commands::{create_item, get_item, search_item};
use crate::session_commands::{session_login};
use crate::token_commands::token_generate;


fn read_configuration_file() -> anyhow::Result<()> {
    let config_path = get_target_file("config/application.properties")?;
    let config_path_str = config_path.to_str().ok_or(anyhow!("Cannot convert path to str"))?;
    println!("Define the properties from file : {}", config_path_str);
    let props = read_config_from_path( &config_path )?;

    set_prop_values(props);

    Ok(())
}

/// Get the location of a file into the working folder
fn get_target_file(termnination_path: &str) -> anyhow::Result<PathBuf> {

    let doka_cli_env = env::var("DOKA_CLI_ENV").unwrap_or("".to_string());

    if ! doka_cli_env.is_empty() {
        Ok(Path::new(&doka_cli_env).join("doka-cli").join(termnination_path).to_path_buf())
    } else {
        let path = current_exe()?; //
        let parent_path = path.parent().ok_or(anyhow!("Problem to identify parent's binary folder"))?;
        Ok(parent_path.join(termnination_path))
    }
}

fn extract_mandatory_option(options: &HashMap<String, Option<String>>, key: &str) -> anyhow::Result<String> {
    let opt_value = options
        .get(key)
        .ok_or_else(|| anyhow!("💣 Unknown parameter, option=[{}]", key))?;
    let value = opt_value.as_ref().ok_or_else(|| anyhow!("💣 Unknown parameter, option=[{}]", key))?;
    Ok(value.to_owned())
}

fn extract_option(options: &HashMap<String, Option<String>>, key: &str) -> anyhow::Result<Option<String>> {
    let opt_value = options.get(key);
    match opt_value {
        None => {Ok(None)}
        Some(o_value) => {
            Ok(o_value.to_owned())

        }
    }
}

fn dispatch( params : &Params, commands : &[Command]) -> u16 {
    match (params.object.as_str(), params.action.as_str()) {
        ("help", "help") => {
            display_commands(commands);
            0
        }
        ("token", "generate") => {
            let Ok(cek_file) = extract_mandatory_option( &params.options, "-c").map_err(eprint_fwd!("Error")) else {
                return 70;
            };
            let _err = token_generate(&cek_file);
            0
        }
        ("customer", "create") => {
            let Ok((customer_name, email, admin_password)) = (|| -> anyhow::Result<(String, String, String)> {
                Ok((extract_mandatory_option( &params.options, "-n")?,
                    extract_mandatory_option( &params.options, "-e")?,
                    extract_mandatory_option( &params.options, "-ap")?))
            }) ().map_err(eprint_fwd!("Error")) else {
                return 70;
            };
            let err = create_customer(&customer_name, &email, &admin_password);
            0
        }
        ("customer", "disable") => {
            let Ok(customer_code) = extract_mandatory_option( &params.options, "-cc").map_err(eprint_fwd!("Error")) else {
                return 70;
            };
            let _err = disable_customer(&customer_code);
            0
        }
        ("customer", "delete") => {
            let Ok(customer_code) = extract_mandatory_option( &params.options, "-cc").map_err(eprint_fwd!("Error")) else {
                return 70;
            };
            let _err = delete_customer(&customer_code);
            0
        }
        ("session", "login") => {
            let Ok((user_name, user_password)) = (|| -> anyhow::Result<(String, String)> {
                Ok((extract_mandatory_option( &params.options, "-u")?,
                    extract_mandatory_option( &params.options, "-p")?))
            }) ().map_err(eprint_fwd!("Error")) else {
                return 80;
            };
            let _err = session_login(&user_name, &user_password);
            0
        }
        ("item", "create") => {
            let Ok((item_name, o_file_ref, o_path, o_properties))
                = (|| -> anyhow::Result<(String, Option<String>, Option<String>, Option<String>)> {
                Ok((extract_mandatory_option( &params.options, "-n")?,
                    extract_option( &params.options, "-r")?,
                   extract_option( &params.options, "-pt")?,
                extract_option( &params.options, "-p")?)
                )
            }) ().map_err(eprint_fwd!("Error")) else {
                return 90;
            };
            let _err = create_item(&item_name, o_file_ref.as_deref(), o_path.as_deref(), o_properties.as_deref());
            0
        }
        ("item", "search") => {
            let _err = search_item();
            0
        }
        ("item", "get") => {
            let Ok(id) = extract_mandatory_option( &params.options, "-id").map_err(eprint_fwd!("Error")) else {
                return 100;
            };
            let _err = get_item(&id);
            0
        }
        ("file", "upload") => {
            let Ok((item_info, path)) = (|| -> anyhow::Result<(String, String)> {
                Ok((extract_mandatory_option( &params.options, "-ii")?,
                    extract_mandatory_option( &params.options, "-pt")?))
            }) ().map_err(eprint_fwd!("Error")) else {
                return 110;
            };
            // let Ok(path) = extract_mandatory_option( &params.options, "-pt").map_err(eprint_fwd!("Error")) else {
            //     return 110;
            // };
            let _err = file_upload(&item_info, &path);
            0
        }
        ("file", "download") => {
            let Ok((path, file_ref)) = (|| -> anyhow::Result<(String, String)> {
                Ok((extract_mandatory_option( &params.options, "-pt")?,
                    extract_mandatory_option( &params.options, "-fr")?))
            }) ().map_err(eprint_fwd!("Error")) else {
                return 120;
            };
            let _err = file_download(&path, &file_ref);
            0
        }
        (_, _) => {
            0
        }
    }
}

///
/// dk [object] [action] [options]
///
/// We need a service discovery and/or a proxy to know where the services are located
/// They are potentially on different servers and ports
///
fn main() -> () {
    println!("doka-cli version 0.1.0");

    let args: Vec<String> = env::args().collect();
    let commands = load_commands();

    let params =  match parse_args(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("💣 Error while parsing the arguments, err=[{}]", e);
            display_commands(&commands);
            exit_program(80);
        }
    };

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

    // match params.object.as_str() {
    //     "token" => {
    //         match token_command(&params) {
    //             Ok(_) => {
    //                 exit_code = 0;
    //             }
    //             Err(e) => {
    //                 exit_code = 70;
    //                 eprintln!("💣 Error {exit_code} : {}", e);
    //             }
    //         }
    //     }
    //     "customer" => {
    //         match customer_command(&params) {
    //             Ok(_) => {
    //                 exit_code = 0;
    //             }
    //             Err(e) => {
    //                 exit_code = 80;
    //                 eprintln!("💣 Error {exit_code} : {}", e);
    //             }
    //         }
    //     }
    //     "session" => {
    //         match session_command(&params) {
    //             Ok(_) => {
    //                 exit_code = 0;
    //             }
    //             Err(e) => {
    //
    //                 exit_code = 90;
    //                 eprintln!("💣 Error {exit_code} : {}", e);
    //             }
    //         }
    //     }
    //     "item" => {
    //         match item_command(&params) {
    //             Ok(_) => {
    //                 exit_code = 0;
    //             }
    //             Err(e) => {
    //
    //                 exit_code = 120;
    //                 eprintln!("💣 Error {exit_code} : {}", e);
    //             }
    //         }
    //     }
    //     "file" => {
    //         match file_command(&params) {
    //             Ok(_) => {
    //                 exit_code = 0;
    //             }
    //             Err(e) => {
    //
    //                 exit_code = 140;
    //                 eprintln!("💣 Error {exit_code} : {}", e);
    //             }
    //         }
    //     }
    //     _ => {
    //
    //     }
    // }


}

fn exit_program(code: i32) -> ! {
    println!("Terminated [{}]", code);
    exit(code)
}
