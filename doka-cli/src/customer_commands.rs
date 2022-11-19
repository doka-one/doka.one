use anyhow::anyhow;
use commons_error::*;
use dkconfig::properties::get_prop_value;
use dkdto::CreateCustomerRequest;
use doka_cli::request_client::AdminServerClient;
use crate::{Params};
use crate::token_commands::read_security_token;

///
///
///
pub (crate) fn customer_command(params: &Params) -> anyhow::Result<()> {

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
            "-n" => {
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
    let token = read_security_token().map_err(eprint_fwd!("Cannot read security token"))?;
    let reply = client.create_customer(&create_customer_request, &token);
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

    let token = read_security_token()?;
    let reply = client.customer_removable(&customer_code, &token);
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

    let token = read_security_token()?;
    let reply = client.delete_customer(&customer_code, &token);
    if reply.error_code == 0 {
        println!("😎 Customer successfully deleted, customer code : {} ", &customer_code);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.err_message))
    }
}
