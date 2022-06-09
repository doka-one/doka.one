#![feature(let_else)]

mod customer_commands;

use std::env;
use std::env::current_exe;
use std::path::Path;
use std::process::exit;
use anyhow::{anyhow};
use dkconfig::conf_reader::{read_config, read_config_from_path};
use dkconfig::properties::{get_prop_value, set_prop_values};
use dkdto::{CreateCustomerRequest};
use doka_cli::request_client::AdminServerClient;
use crate::customer_commands::customer_command;

// This is a dummy token
// TODO Token generation from a system user (should be limited in time)
const SECURITY_TOKEN : &str = "j6nk2GaKdfLl3nTPbfWW0C_Tj-MFLrJVS2zdxiIKMZpxNOQGnMwFgiE4C9_cSScqshQvWrZDiPyAVYYwB8zCLRBzd3UUXpwLpK-LMnpqVIs";

#[derive(Debug)]
struct Params {
    object: String,
    action: String,
    options : Vec<(String, String)>,
}

fn parse(args : &Vec<String>) -> anyhow::Result<Params> {
    // println!("number of args, [{}]", args.len());
    let object = args.get(1).ok_or(anyhow!("Don't find 1st param"))?.clone();
    let action = args.get(2).ok_or(anyhow!("Don't find 2nd param"))?.clone();
    let mut options : Vec<(String, String)> = vec![];
    let mut i = 3;

    loop {
        let option_name = args.get(i).ok_or(anyhow!("Don't find param, i=[{}]", i))?.clone();
        let option_value = args.get(i+1).ok_or(anyhow!("Don't find param, i+1=[{}]", i+1))?.clone();
        options.push((option_name, option_value));
        // println!("option=[{:?}]", &options);
        i += 2;
        if i > args.len()-1 {
            break;
        }
    }

    Ok(Params {
        object,
        action,
        options,
    })
}


fn read_configuration_file() -> anyhow::Result<()> {

    let doka_cli_env = env::var("DOKA_CLI_ENV").unwrap_or("".to_string());

    let props = if ! doka_cli_env.is_empty() {
        // For debug or advanced usage, you define a DOKA_CLI_ENV environment variable
        // and the path will be {DOKA_CLI_ENV}/doka-cli/config/application.properties
        println!("Define the properties from {}/doka-cli", &doka_cli_env);
        read_config("doka-cli", "DOKA_CLI_ENV")
    } else {
        let path = current_exe()?; //
        let parent_path = path.parent().ok_or(anyhow!("Problem to identify parent's binary folder"))?;
        let config_path = parent_path.join("config/application.properties");
        let config_path_str = config_path.to_str().unwrap();
        println!("Define the properties from local file : {}", config_path_str);
        read_config_from_path( &config_path )?
    };

    set_prop_values(props);

    Ok(())

}

///
/// dk [object] [action] [options]
///
/// We need a service discovery and/or a proxy to know where the services are located
/// They are potentially on different servers and ports
///
fn main() -> () {
    println!("dk cli version 0.1.0");

    let mut exit_code = 0;
    let args: Vec<String> = env::args().collect();

    let params =  match parse(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("💣 Error while parsing the arguments, err=[{}]", e);
            exit_program(80);
        }
    };

    // println!("Params [{:?}]", &params);

    match read_configuration_file() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("💣 Error while reading the configuration file, err=[{}]", e);
            exit_program(110);
        }
    }

    let server_host = get_prop_value("server.host").unwrap();
    println!("Server host [{}]", &server_host);

    //

    match params.object.as_str() {
        "customer" => {
            match customer_command(&params) {
                Ok(_) => {
                    exit_code = 0;
                }
                Err(e) => {
                    eprintln!("💣 Error : {}", e);
                    exit_code = 90;
                }
            }
        }
        "user" => {

        }
        _ => {

        }
    }

    exit_program(exit_code);
}

fn exit_program(code: i32) -> ! {
    println!("Terminated [{}]", code);
    exit(code)
}
