use anyhow::anyhow;

use commons_error::*;
use dkconfig::properties::get_prop_value;
use dkdto::CreateCustomerRequest;
use doka_cli::request_client::AdminServerClient;

use crate::token_commands::read_security_token;

///
pub (crate) fn create_customer(customer_name: &str, email : &str, admin_password : &str) -> anyhow::Result<()> {
    println!("👶 Create a customer...");

    let server_host = get_prop_value("server.host")?;
    let admin_server_port : u16 = get_prop_value("as.port")?.parse()?;
    println!("Admin server port : {}", admin_server_port);
    let client = AdminServerClient::new(&server_host, admin_server_port);
    let create_customer_request = CreateCustomerRequest {
        customer_name: customer_name.to_string(),
        email: email.to_string(),
        admin_password: admin_password.to_string(),
    };
    let token = read_security_token().map_err(eprint_fwd!("Cannot read security token"))?;
    let reply = client.create_customer(&create_customer_request, &token);

    match reply {
        Ok(create_customer_reply) => {
            println!("😎 Customer successfully created, customer code : {} ", create_customer_reply.customer_code);
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{}", e.message))
        }
    }
}

// disable customer
pub (crate) fn disable_customer(customer_code: &str) -> anyhow::Result<()> {
    println!("💧 Disable a customer...");

    let server_host = get_prop_value("server.host")?;
    let admin_server_port : u16 = get_prop_value("as.port")?.parse()?;
    println!("Admin server port : {}", admin_server_port);
    let client = AdminServerClient::new(&server_host, admin_server_port);

    let token = read_security_token()?;
    let wr_reply = client.customer_removable(&customer_code, &token);

    match wr_reply {
        Ok(_reply) => {
            println!("😎 Customer successfully disabled, customer code : {} ", &customer_code);
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{}", e.message))
        }
    }

}


// disable customer
pub (crate) fn delete_customer(customer_code: &str) -> anyhow::Result<()> {
    println!("🔥 Delete a customer...");

    let server_host = get_prop_value("server.host")?;
    let admin_server_port : u16 = get_prop_value("as.port")?.parse()?;
    println!("Admin server port : {}", admin_server_port);
    let client = AdminServerClient::new(&server_host, admin_server_port);

    let token = read_security_token()?;
    let wr_reply = client.delete_customer(&customer_code, &token);

    match wr_reply {
        Ok(message) => {
            println!("😎 Customer successfully deleted, customer code : [{}], message : [{:?}] ", &customer_code, &message);
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{}", e.message))
        }
    }

}
