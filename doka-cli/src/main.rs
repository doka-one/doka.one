#![feature(let_else)]

use std::env;
use std::path::Path;
use std::process::exit;
use anyhow::{anyhow};
use dkconfig::conf_reader::{read_config, read_config_from_path};
use dkconfig::properties::{get_prop_value, set_prop_values};
use dkdto::{CreateCustomerRequest};
use doka_cli::request_client::AdminServerClient;

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
    println!("number of args, [{}]", args.len());
    let object = args.get(1).ok_or(anyhow!("Don't find 1st param"))?.clone();
    let action = args.get(2).ok_or(anyhow!("Don't find 2nd param"))?.clone();
    let mut options : Vec<(String, String)> = vec![];
    let mut i = 3;

    loop {
        let option_name = args.get(i).ok_or(anyhow!("Don't find param, i=[{}]", i))?.clone();
        let option_value = args.get(i+1).ok_or(anyhow!("Don't find param, i+1=[{}]", i+1))?.clone();
        options.push((option_name, option_value));
        println!("option=[{:?}]", &options);
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

///
///
///
fn customer_command(params: &Params) -> anyhow::Result<()> {

    match params.action.as_str() {
        "create" => {
            create_customer(&params)
        }
        "disable" => {
            disable_customer(&params)
        }
        "delete" => {
            delete_customer(&params)
        }
        action => {
            Err(anyhow!("💣 Unknown action=[{}]", action))
        }
    }
}

///
fn create_customer(params: &Params) -> anyhow::Result<()> {
    println!("👶 Create a customer...");
    let mut customer_name = None;
    let mut email = None;
    let mut admin_password = None;
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-c" => {
                customer_name = Some(option_value.clone());
            }
            "-e" => {
                email = Some(option_value.clone())
            }
            "-ap" => {
                admin_password = Some(option_value.clone())
            }
            opt => {
                return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt))
            }
        }
    }

    let server_host = get_prop_value("server.host")?;
    let admin_server_port : u16 = get_prop_value("as.port")?.parse()?;
    println!("Admin server port : {}", admin_server_port);
    let client = AdminServerClient::new(&server_host, admin_server_port);
    let create_customer_request = CreateCustomerRequest {
        customer_name: customer_name.ok_or(anyhow!("💣 Missing customer name"))?,
        email: email.ok_or(anyhow!("💣 Missing email"))?,
        admin_password: admin_password.ok_or(anyhow!("💣 Missing admin password"))?
    };
    let token = SECURITY_TOKEN;
    let reply = client.create_customer(&create_customer_request, token);
    if reply.status.error_code == 0 {
        println!("😎 Customer successfully created, customer code : {} ", reply.customer_code);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}



// disable customer
fn disable_customer(params: &Params) -> anyhow::Result<()> {
    println!("💧 Disable a customer...");
    let mut o_customer_code = None;
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-cc" => {
                o_customer_code = Some(option_value.clone());
            }
            opt => {
                return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt))
            }
        }
    }

    let server_host = get_prop_value("server.host")?;
    let admin_server_port : u16 = get_prop_value("as.port")?.parse()?;
    println!("Admin server port : {}", admin_server_port);
    let client = AdminServerClient::new(&server_host, admin_server_port);

    let customer_code = o_customer_code.ok_or(anyhow!("💣 Missing customer code"))?;

    let token = SECURITY_TOKEN;
    let reply = client.customer_removable(&customer_code, token);
    if reply.error_code == 0 {
        println!("😎 Customer successfully disabled, customer code : {} ", &customer_code);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.err_message))
    }
}


// disable customer
fn delete_customer(params: &Params) -> anyhow::Result<()> {
    println!("🔥 Delete a customer...");
    let mut o_customer_code = None;
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-cc" => {
                o_customer_code = Some(option_value.clone());
            }
            opt => {
                return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt))
            }
        }
    }

    let server_host = get_prop_value("server.host")?;
    let admin_server_port : u16 = get_prop_value("as.port")?.parse()?;
    println!("Admin server port : {}", admin_server_port);
    let client = AdminServerClient::new(&server_host, admin_server_port);

    let customer_code = o_customer_code.ok_or(anyhow!("💣 Missing customer code"))?;
    // TODO Token generation from a system user (should be limited in time)
    let token = SECURITY_TOKEN;
    let reply = client.delete_customer(&customer_code, token);
    if reply.error_code == 0 {
        println!("😎 Customer successfully deleted, customer code : {} ", &customer_code);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.err_message))
    }
}

fn read_configuration_file() -> anyhow::Result<()> {

    let doka_cli_env = env::var("DOKA_CLI_ENV").unwrap_or("".to_string());

    let props = if ! doka_cli_env.is_empty() {
        // For debug or advanced usage, you define a DOKA_CLI_ENV environment variable
        // and the path will be {DOKA_CLI_ENV}/doka-cli/config/application.properties
        println!("Define the properties from {}/doka-cli", &doka_cli_env);
        read_config("doka-cli", "DOKA_CLI_ENV")
    } else {
        println!("Define the properties from local file");
        read_config_from_path(&Path::new("config/application.properties").to_path_buf())?
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

    println!("Params [{:?}]", &params);

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
