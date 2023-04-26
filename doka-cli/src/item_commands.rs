use anyhow::anyhow;
use regex::{Match, Regex};
use serde_json::Value::Bool;
use commons_error::*;

use dkconfig::properties::get_prop_value;
use dkdto::{AddItemRequest, AddTagValue, EnumTagValue, GetItemReply};
use dkdto::EnumTagValue::Boolean;
use doka_cli::request_client::DocumentServerClient;
use crate::command_options::{Command, Params};
use crate::session_commands::read_session_id;

///
// pub (crate) fn item_command(params: &Params) -> anyhow::Result<()> {
//
//     match params.action.as_str() {
//         "get" => {
//             get_item(&params)
//         }
//         // "search" => {
//         //     search_item(&params)
//         // }
//         // "create" => {
//         //     create_item(&params)
//         // }
//         action => {
//             Err(anyhow!("💣 Unknown action=[{}]", action))
//         }
//     }
// }


///
pub(crate) fn get_item(id: &str) -> anyhow::Result<()> {
    println!("👶 Getting the item...");
    // let mut o_item_id = None;
    // for (option, option_value) in &params.options {
    //     match option.as_str() {
    //         "-id" => {
    //             o_item_id = option_value.clone();
    //         }
    //         opt => {
    //             return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt))
    //         }
    //     }
    // }

    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;
    println!("Document server port : {}", document_server_port);
    let client = DocumentServerClient::new(&server_host, document_server_port);

    let item_id: i64 = id.parse()?;
    let sid = read_session_id()?;
    let reply = client.get_item(item_id, &sid);
    if reply.status.error_code == 0 {
        println!("😎 Item successfully found, count : {} ", reply.items.len());
        let _ = show_items(&reply);
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
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
    let reply = client.search_item(&sid);
    if reply.status.error_code == 0 {
        println!("😎 Item successfully found, count : {} ", reply.items.len());
        let _r = show_items(&reply); // TODO handle error and use eprint_fwd!
        Ok(())
    } else {
        Err(anyhow!("{}", reply.status.err_message))
    }
}

//
fn show_items(items: &GetItemReply) -> anyhow::Result<()> {
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
            (name.to_owned(), Some(value.trim_matches('\'').to_owned()), None)
        }
        _ => {
            let (opt_name, opt_value, opt_type) = extract_parts(prop).unwrap();
            (opt_name.unwrap(), opt_value, opt_type)
        }
    };

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
                 Err(e) => {
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
    let properties = if let Some(props_str) = o_properties {
        let re = Regex::new(r"\((.*?)\)").unwrap();
        let mut props: Vec<AddTagValue> = vec![];
        for cap in re.captures_iter(props_str) {
            // dbg!(&cap[1]);
            let tag = build_item_tag(&cap[1]).map_err(eprint_fwd!("Cannot read the tag value"))?;
            props.push(tag);
        }
        props
    } else {
        vec![]
    };

    let server_host = get_prop_value("server.host")?;
    let document_server_port: u16 = get_prop_value("ds.port")?.parse()?;
    println!("Document server port : {}", document_server_port);
    let client = DocumentServerClient::new(&server_host, document_server_port);

    // dbg!(&properties);

    let add_item_request = AddItemRequest {
        name: item_name.to_owned(),
        file_ref: o_file_ref.map(|s| { s.to_string() }),
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

#[cfg(test)]
mod tests {
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

