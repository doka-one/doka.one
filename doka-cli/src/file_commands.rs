use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use anyhow::anyhow;
use dkconfig::properties::get_prop_value;
use doka_cli::request_client::FileServerClient;
use crate::command_options::Params;

use crate::session_commands::read_session_id;


///
pub(crate) fn file_upload(item_info: &str, path :&str) -> anyhow::Result<()> {
    println!("👶 Uploading the file...");

    let server_host = get_prop_value("server.host")?;
    let file_server_port: u16 = get_prop_value("fs.port")?.parse()?;
    println!("File server port : {}", file_server_port);
    let client = FileServerClient::new(&server_host, file_server_port);

    //let path =  o_path.ok_or(anyhow!("💣 Missing path"))?;
    let sid = read_session_id()?;

    let file = File::open(Path::new(&path))?;
    let mut buf_reader = BufReader::new(file);
    let mut binary : Vec<u8> = vec![];
    let _n = buf_reader.read_to_end(&mut binary)?;
    let reply = client.upload(&item_info, &binary, &sid);
    if reply.status.error_code == 0 {
        println!("😎 File successfully uploaded, reference : {}, number of blocks : {} ", reply.file_ref, reply.block_count);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}

///
/// Download the content behind the reference into the file at the path
///
pub(crate) fn file_download(path : &str, file_ref: &str) -> anyhow::Result<()> {
    println!("👶 Downloading the file...");

    let server_host = get_prop_value("server.host")?;
    let file_server_port: u16 = get_prop_value("fs.port")?.parse()?;
    println!("File server port: {}", file_server_port);
    let client = FileServerClient::new(&server_host, file_server_port);
    //let path = o_path.ok_or(anyhow!("💣 Missing path"))?;
    //let file_reference = o_file_ref.ok_or(anyhow!("💣 Missing file reference"))?;
    let sid = read_session_id()?;

    let reply = client.download(&file_ref, &sid);
    // TODO REF_TAG : HTTP_ERROR_CODE   For now the StatusCode is always 200
    println!("Status Code: {}", reply.2);

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
