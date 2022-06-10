use std::fs::File;
use std::io::{BufReader, Read};
use anyhow::anyhow;
use dkconfig::properties::get_prop_value;
use dkdto::{AddItemRequest, EnumTagValue, GetItemReply, TagValueElement};
use doka_cli::request_client::DocumentServerClient;
use crate::{get_target_file, Params, read_session_id};

///
pub (crate) fn item_command(params: &Params) -> anyhow::Result<()> {

    match params.action.as_str() {
        "get" => {
            get_item(&params)
        }
        "search" => {
            search_item(&params)
        }
        action => {
            Err(anyhow!("💣 Unknown action=[{}]", action))
        }
    }
}


///
fn get_item(params: &Params) -> anyhow::Result<()> {
    println!("👶 Getting the item...");
    let mut o_item_id = None;
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-id" => {
                o_item_id = Some(option_value.clone());
            }
            opt => {
                return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt))
            }
        }
    }

    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;
    println!("Document server port : {}", document_server_port);
    let client = DocumentServerClient::new(&server_host, document_server_port);

    let item_id : i64 =  o_item_id.ok_or(anyhow!("💣 Missing item id"))?.parse()?;
    let sid = read_session_id()?;
    let reply = client.get_item(item_id, &sid);
    if reply.status.error_code == 0 {
        println!("😎 Item successfully found, count : {} ", reply.items.len());
        show_items(&reply);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}

///
fn search_item(params: &Params) -> anyhow::Result<()> {
    println!("👶 Getting the item...");
    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;
    println!("Document server port : {}", document_server_port);
    let client = DocumentServerClient::new(&server_host, document_server_port);
    let sid = read_session_id()?;
    let reply = client.search_item(&sid);
    if reply.status.error_code == 0 {
        println!("😎 Item successfully found, count : {} ", reply.items.len());
        show_items(&reply);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}



//
fn show_items(items: &GetItemReply) -> anyhow::Result<()>{
    for item in &items.items {
        let prop_str = match &item.properties {
            None => {
                "".to_string()
            }
            Some(p) => {
                let mut p_str : String = "".to_string();
                for prop in p {
                    p_str.push_str( &prop.tag_name );
                    p_str.push_str( ":" );
                    p_str.push_str( & prop.value.to_string() );
                    p_str.push_str( "\t" );
                }
                p_str
            }
        };
        println!("{}\t{}\t{}", item.item_id, item.name, prop_str);
    }
    Ok(())
}