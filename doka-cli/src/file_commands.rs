use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use anyhow::anyhow;
use dkconfig::properties::get_prop_value;
use doka_cli::request_client::FileServerClient;
use crate::{Params, read_session_id};

///
pub (crate) fn file_command(params: &Params) -> anyhow::Result<()> {

    match params.action.as_str() {
        "upload" => {
            file_upload(&params)
        }
        "download" => {
            file_download(&params)
        }
        action => {
            Err(anyhow!("💣 Unknown action=[{}]", action))
        }
    }
}

///
fn file_upload(params: &Params) -> anyhow::Result<()> {
    println!("👶 Uploading the file...");
    let mut o_path = None;
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-pt" | "--path" => {
                o_path = Some(option_value.clone());
            }
            opt => {
                return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt));
            }
        }
    }

    let server_host = get_prop_value("server.host")?;
    let file_server_port: u16 = get_prop_value("fs.port")?.parse()?;
    println!("File server port : {}", file_server_port);
    let client = FileServerClient::new(&server_host, file_server_port);

    let path =  o_path.ok_or(anyhow!("💣 Missing path"))?;
    let sid = read_session_id()?;

    let file = File::open(Path::new(&path))?;
    let mut buf_reader = BufReader::new(file);
    let mut binary : Vec<u8> = vec![];
    let _n = buf_reader.read_to_end(&mut binary)?;
    let reply = client.upload(&binary, &sid);
    if reply.status.error_code == 0 {
        println!("😎 File successfully uploaded, reference : {}, number of blocks : {} ", reply.file_ref, reply.block_count);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}

///
fn file_download(params: &Params) -> anyhow::Result<()> {
    println!("👶 Downloading the file...");

    let mut o_path = None;
    let mut o_file_ref = None;
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-pt" | "--path" => {
                o_path = Some(option_value.clone());
            }
            "-fr" | "--file_ref" => {
                o_file_ref = Some(option_value.clone());
            }
            opt => {
                return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt));
            }
        }
    }

    let server_host = get_prop_value("server.host")?;
    let file_server_port: u16 = get_prop_value("fs.port")?.parse()?;
    println!("File server port: {}", file_server_port);
    let client = FileServerClient::new(&server_host, file_server_port);
    let path = o_path.ok_or(anyhow!("💣 Missing path"))?;
    let file_reference = o_file_ref.ok_or(anyhow!("💣 Missing file reference"))?;
    let sid = read_session_id()?;

    let reply = client.download(&file_reference, &sid);

    // Store the result in a file
    let size = reply.1.len();
    if size > 0 {
        let mut file = std::fs::File::create(&path)?;
        let mut content = Cursor::new(reply.1);
        std::io::copy(&mut content, &mut file)?;
        println!("Document stored at: {}", &path);
        println!("Document type: {}", reply.0);
        println!("Document size: {}", size);
    } else {
        println!("Document not stored because it's empty");
    }

    Ok(())
}