use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::anyhow;
use regex::Regex;

use commons_error::*;
use dkconfig::properties::get_prop_value;
use dkdto::{AddItemRequest, AddItemTagRequest, AddTagValue, EnumTagValue, GetItemReply};
use doka_cli::request_client::{DocumentServerClient, FileServerClient};

use crate::item_commands::DisplayFormat::{INLINE, JSON};
use crate::session_commands::read_session_id;

enum DisplayFormat {
    #[allow(dead_code)]
    INLINE,
    JSON,
}

///
pub(crate) fn get_item(id: &str) -> anyhow::Result<()> {
    println!("👶 Getting the item...");

    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;
    println!("Document server port : {}", document_server_port);
    let client = DocumentServerClient::new(&server_host, document_server_port);

    let item_id: i64 = id.parse()?;
    let sid = read_session_id()?;
    match client.get_item(item_id, &sid) {
        Ok(reply) => {
            println!("😎 Item successfully found, count : {} ", reply.items.len());
            let _ = show_items(&reply, JSON);
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{} - {}", e.http_error_code, e.message))
        }
    }
}

///
pub(crate) fn search_item() -> anyhow::Result<()> {
    println!("👶 Getting the item...");
    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;
    println!("Document server port : {}", document_server_port);
    let client = DocumentServerClient::new(&server_host, document_server_port);
    let sid = read_session_id()?;
    let wr_reply = client.search_item(&sid);

    match wr_reply {
        Ok(reply) => {
            println!("😎 Item successfully found, count : {} ", reply.items.len());
            let _r = show_items(&reply, INLINE); // TODO handle error and use eprint_fwd!
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{}", e.message))
        }
    }
}


//
fn show_items(items: &GetItemReply, display_format: DisplayFormat) -> anyhow::Result<()> {
    match &display_format {
        DisplayFormat::INLINE => {
            for item in &items.items {
                let prop_str = match &item.properties {
                    None => {
                        "".to_string()
                    }
                    Some(p) => {
                        let mut p_str: String = "".to_string();
                        for prop in p {
                            p_str.push_str(&prop.tag_name);
                            p_str.push_str(":");
                            p_str.push_str(&prop.value.to_string());
                            p_str.push_str("\t");
                        }
                        p_str
                    }
                };
                let default_value = "none".to_string();
                let file_ref = item.file_ref.as_ref().unwrap_or(&default_value);
                println!("id:{}\tname:{}\tfile_ref:{}\t{}", item.item_id, item.name, file_ref, prop_str);
            }
        }
        DisplayFormat::JSON => {
            let s = serde_json::to_string_pretty(&items.items).unwrap();
            println!("{}", &s);
        }
    }
    Ok(())
}

fn extract_parts(input: &str) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let re = Regex::new(r#"^([^:]+):(.+)?:([^:]+)?$"#).unwrap();
    if let Some(captures) = re.captures(input) {
        let part1 = match captures.get(1) {
            None => {
                None
            }
            Some(v) => {
                Some(v.as_str().to_owned())
            }
        };

        let part2 = match captures.get(2) {
            None => {
                None
            }
            Some(v) => {
                Some(v.as_str().trim_matches('\'').to_owned())
            }
        };

        let part3 = match captures.get(3) {
            None => {
                None
            }
            Some(v) => {
                Some(v.as_str().to_owned())
            }
        };
        Some((part1, part2, part3))
    } else {
        None
    }
}


fn parse_property(prop: &str) -> anyhow::Result<(String, Option<String>, Option<String>)> {
    let nb_sep = prop.chars().filter(|&c| c == ':').count();
    let (tag_name, opt_tag_value, opt_tag_type) = match nb_sep {
        0 => {
            (prop.to_owned(), None, None)
        }
        1 => {
            let separator = prop.find(":").unwrap_or(prop.len());
            let (name, value) = prop.split_at(separator);
            (name.to_owned(), Some(value.replace(":", "").trim_matches('\'').to_owned()), None)
        }
        _ => {
            let (opt_name, opt_value, opt_type) = extract_parts(prop).unwrap();
            (opt_name.unwrap(), opt_value, opt_type)
        }
    };

    dbg!(&opt_tag_value);

    Ok((tag_name, opt_tag_value, opt_tag_type))
}

///
/// The input must be of "age:24:integer" "age:24"  or "flag1"
///
fn build_item_tag(param_value: &str) -> anyhow::Result<AddTagValue> {

    let (tag_name, tag_value, tag_type)  = parse_property(&param_value)
        .map_err(tr_fwd!())?;

    let enum_tag_value = match (tag_value, tag_type) {
        (None, None) => {
            EnumTagValue::Boolean(Some(true))
        }
        (Some(v), None) => {
            EnumTagValue::String(Some(v))
        }
        (None, Some(_)) => {
            return Err(anyhow!("Missing value for property: {}", param_value));
        }
        (Some(v), Some(t)) => {
             let r = EnumTagValue::from_string(&v, &t);
             match r {
                 Ok(v) => {
                     v
                 }
                 Err(_e) => {
                     return Err(anyhow!("Property type and value does not match: {}", param_value));
                 }
             }
        }
    };

    Ok(AddTagValue {
        tag_id: None,
        tag_name: Some(tag_name),
        value: enum_tag_value
    })

}

///
pub(crate) fn create_item(item_name: &str, o_file_ref: Option<&str>, o_path: Option<&str>, o_properties: Option<&str>) -> anyhow::Result<()> {
    println!("👶 Creating the item...");

    // Fill the properties vector from the "(tag[:value[:link|date|text|number]])()..."
    let properties = build_properties_from_string(o_properties)?;

    let sid = read_session_id()?;

    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;
    let file_server_port: u16 = get_prop_value("fs.port")?.parse()?;
    let file_server_client = FileServerClient::new(&server_host, file_server_port);

    let new_file_ref = match o_path {
        None => {None}
        Some(path) => {
            println!("Uploading the file...");
            let file = File::open(Path::new(path))?;
            let mut buf_reader = BufReader::new(file);
            let mut binary : Vec<u8> = vec![];
            let _n = buf_reader.read_to_end(&mut binary)?;
            let fs_reply = file_server_client.upload(&item_name, &binary, &sid);

            if let Err(e) = fs_reply {
                eprintln!("File upload failed, {}", &e.message);
                return Err(anyhow!("{}", e.message));
            }
            Some(fs_reply.unwrap().file_ref)
        }
    };

    // println!("Document server port : {}", document_server_port);
    let document_server_client = DocumentServerClient::new(&server_host, document_server_port);

    // dbg!(&properties);

    let file_ref = if let Some(ref new_file_ref) = new_file_ref {
        println!("New file reference: {}", new_file_ref);
        Some(new_file_ref.clone())
    } else {
        // Ensure the file_ref exists
        if let Some(fr) = o_file_ref {
            let wt_get_file_info = file_server_client.info(fr, &sid);
            match wt_get_file_info {
                Ok(reply) => {
                    println!("File reference found, {}", reply.file_ref);
                    Some(reply.file_ref)
                }
                Err(_e) => {
                    eprintln!("Error, cannot find the file reference, {}", fr);
                    None
                }
            }
        } else {
            eprintln!("No file to link");
            None
        }
    };

    let add_item_request = AddItemRequest {
        name: item_name.to_owned(),
        file_ref,
        properties: Some(properties),
    };

    // dbg!(&add_item_request);
    let wt_create_item_reply = document_server_client.create_item(&add_item_request, &sid);

    match wt_create_item_reply {
        Ok(reply) => {
            println!("😎 Item successfully created, id : {} ", reply.item_id);
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{}", e.message))
        }
    }
}

pub fn item_tag_update(id: &str, o_add_props: Option<&str>) -> anyhow::Result<()> {
    println!("👶 Change the item tags...");

    let item_id: i64 = id.parse()?;

    // Fill the properties vector from the "(tag[:value[:link|date|text|number]])()..."
    let properties = build_properties_from_string(o_add_props)?;
    dbg!(&properties);

    let sid = read_session_id()?;
    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;

    let document_server_client = DocumentServerClient::new(&server_host, document_server_port);
    let add_item_tag_request = AddItemTagRequest { item_id, properties };
    let r_add_item_tag = document_server_client.update_item_tag(item_id, &add_item_tag_request, &sid);

    match r_add_item_tag {
        Ok(_reply) => {
            println!("😎 Tags successfully added, for item id : {} ", item_id);
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{}", e.message))
        }
    }
}

pub fn item_tag_delete(id: &str, o_delete_props: Option<&str>) -> anyhow::Result<()> {
    println!("👶 Delete the item tags...");

    let item_id: i64 = id.parse()?;
    dbg!(&o_delete_props);

    let tag_names: Vec<String> = o_delete_props.unwrap_or("")
        .split(',')
        .map(|tag| tag.to_string())
        .collect();

    dbg!(&tag_names);

    let sid = read_session_id()?;
    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;

    let document_server_client = DocumentServerClient::new(&server_host, document_server_port);
    // let add_item_tag_request = AddItemTagRequest { item_id, properties };
    let r_add_item_tag = document_server_client.delete_item_tag(item_id, &tag_names, &sid);

    match r_add_item_tag {
        Ok(_reply) => {
            println!("😎 Tags successfully deleted, for item id : {} ", item_id);
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("{}", e.message))
        }
    }
}

fn build_properties_from_string(o_props: Option<&str>) -> anyhow::Result<Vec<AddTagValue>> {
    let properties = if let Some(props_str) = o_props {
        let re = Regex::new(r"\((.*?)\)").unwrap();
        let mut props: Vec<AddTagValue> = vec![];
        for cap in re.captures_iter(props_str) {
            dbg!(&cap[1]);
            let tag = build_item_tag(&cap[1]).map_err(eprint_fwd!("Cannot read the tag value"))?;
            props.push(tag);
        }
        props
    } else {
        vec![]
    };
    Ok(properties)
}

#[cfg(test)]
mod tests {
    use std::{dbg, println};
    use crate::item_commands::{build_item_tag, extract_parts, parse_property};

    #[test]
    fn test_two_more_colon_prop_parsing() {
        let prop = "my_prop1:'value:value2':text";
        let x = extract_parts(prop);

        dbg!(x);
        println!("-------------");

        let prop = "my_prop1:value:value2:text";
        let x = extract_parts(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_prop1::type";
        let x = extract_parts(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_prop1::";
        let x = extract_parts(prop);
        dbg!(x);
        println!("-------------");
        // let (name, value, prop_type) = extract_prop(prop);
        // dbg!(name, value, prop_type);
        assert_eq!(2 + 2, 4);
    }


    #[test]
    fn test_prop_parsing() {
        let prop = "my_prop1:'value:value2':text";
        let x = parse_property(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_prop1:value:value2:text";
        let x = parse_property(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_prop1::type";
        let x = parse_property(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_prop1::";
        let x = parse_property(prop);
        dbg!(x);
        println!("-------------");
    }


    #[test]
    fn test_build_tag_value() {

        let prop = "my_double:3.14159:decimal";
        let x = build_item_tag(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_int:456:int";
        let x = build_item_tag(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_bool";
        let x = build_item_tag(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_bool:false:bool";
        let x = build_item_tag(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_prop1:'mon text complet d été':text";
        let x = build_item_tag(prop);
        dbg!(x);
        println!("-------------");
        let prop = "my_prop1:'mon text : complet d' été':text";
        let x = build_item_tag(prop);
        dbg!(x);
        println!("-------------");
    }
}

