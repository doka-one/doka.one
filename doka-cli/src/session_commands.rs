use std::fs::File;
use std::io::{BufReader, Read, Write};
use anyhow::anyhow;
use dkconfig::properties::get_prop_value;

use dkdto::LoginRequest;
use doka_cli::request_client::AdminServerClient;
use crate::{get_target_file};
use crate::command_options::Params;


///
pub (crate)  fn session_login(user_name: &str, user_password: &str) -> anyhow::Result<()> {
    println!("👶 Open a session...");

    let server_host = get_prop_value("server.host")?;
    let admin_server_port : u16 = get_prop_value("as.port")?.parse()?;
    println!("Admin server port : {}", admin_server_port);
    let client = AdminServerClient::new(&server_host, admin_server_port);
    let login_request = LoginRequest {
        login: user_name.to_owned(),
        password: user_password.to_owned(),
    };
    let reply = client.login(&login_request);
    if reply.status.error_code == 0 {
        let customer_code = reply.customer_code.clone();
        println!("Connected as customer {}", &customer_code);
        write_session_id(&reply.session_id)?;

        println!("😎 Session successfully opened, session id : {}... ", &reply.session_id[0..7]);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}

fn write_session_id(session_id: &str) -> anyhow::Result<()> {
    let mut file = File::create(get_target_file("config/session.id")?)?;
    // Write a byte string.
    file.write_all(&session_id.to_string().into_bytes()[..])?;
    println!("💾 Session id stored");
    Ok(())
}

pub (crate) fn read_session_id() -> anyhow::Result<String> {
    let file = File::open(get_target_file("config/session.id")?)?;
    let mut buf_reader = BufReader::new(file);
    let mut content: String = "".to_string();
    let _ = buf_reader.read_to_string(&mut content)?;
    Ok(content)
}
