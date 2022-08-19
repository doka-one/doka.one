use std::fs::File;
use std::io::{BufReader, Read};
use anyhow::anyhow;
use serde_json::ser::State::Empty;
use dkconfig::properties::get_prop_value;
use dkdto::{AddItemRequest, AddTagValue, EnumTagValue, GetItemReply, TagValueElement};
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
        "create" => {
            create_item(&params)
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
        let default_value = "none".to_string();
        let file_ref = item.file_ref.as_ref().unwrap_or(&default_value);
        println!("id:{}\tname:{}\tfile_ref:{}\t{}", item.item_id, item.name, file_ref, prop_str);
    }
    Ok(())
}

///
fn create_item(params: &Params) -> anyhow::Result<()> {
    println!("👶 Creating the item...");
    let mut o_name = None;
    let mut o_file_ref = None;
    let mut o_path = None;
    let mut properties : Vec<AddTagValue>= vec![];
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-n" | "--name"  => {
                o_name = Some(option_value.clone());
            }
            "-r" | "--ref"  => {
                o_file_ref = Some(option_value.clone());
            }
            "-pt" | "--path"  => {
                o_path = Some(option_value.clone());
            }
            "-p" | "--property"  => {
                // name:value , value is optional
                let separator = option_value.find(":").unwrap_or(option_value.len());
                let (tag_name, tag_value)  = option_value.split_at(separator);

                // dbg!((tag_name, tag_value));
                if tag_value == ":" {
                    return Err(anyhow!("Property value is empty after \":\""));
                }

                let add_tag_value = if tag_value.is_empty() {
                    AddTagValue {
                        tag_id: None,
                        tag_name: Some(tag_name.to_string()),
                        value: EnumTagValue::Boolean(Some(true)),
                    }
                } else {
                    let tag_value = &tag_value[1..];
                    // TODO guess the tag type from it's value format, ex : 2022-01-02 is date, and so on
                    AddTagValue {
                        tag_id: None,
                        tag_name: Some(tag_name.to_string()),
                        value: EnumTagValue::String(Some(tag_value.to_string())),
                    }
                };

                properties.push(add_tag_value);
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

    // dbg!(&properties);

    let name =  o_name.ok_or(anyhow!("💣 Missing name"))?;
    let add_item_request = AddItemRequest {
        name,
        file_ref: o_file_ref,
        properties: Some(properties),
    };

    // dbg!(&add_item_request);

    let sid = read_session_id()?;

    // TODO The web service must ensure the file_ref exists and is not taken.
    let reply = client.create_item(&add_item_request, &sid);
    if reply.status.error_code == 0 {
        println!("😎 Item successfully created, id : {} ", reply.item_id);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}