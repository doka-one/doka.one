use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::ops::Add;
use std::path::Path;
use anyhow::anyhow;
use chrono::{DateTime, Duration, Utc};

use dkcrypto::dk_crypto::DkEncrypt;


use serde::{Serialize, Deserialize};

use crate::{get_target_file, Params};

///
///
///
pub (crate) fn token_command(params: &Params) -> anyhow::Result<()> {
    match params.action.as_str() {
        "generate" => {
            token_generate(&params)
        }
        action => {
            Err(anyhow!("💣 Unknown action=[{}]", action))
        }
    }
}

/// {"expiry_date":"2022-11-05T14:55:60Z"}
#[derive(Debug, Serialize, Deserialize)]
struct ClearSecurityToken {
    pub expiry_date : DateTime<Utc>,
}

impl ClearSecurityToken {
    pub fn new() -> Self {
        ClearSecurityToken {
            expiry_date: Utc::now().add(Duration::minutes(60)),
        }
    }
}

fn read_cek_from_file(cek_file: &Path) -> anyhow::Result<String> {

    let cek_path = cek_file.to_str().ok_or(anyhow!("Wrong cek file"))?;
    match std::fs::read_to_string(cek_file) {
        Ok(content) => {Ok(content)}
        Err(_e) => {
            return Err(anyhow!("Cannot read the CEK file at {}", cek_path)) ;
        }
    }
}

///
fn token_generate(params: &Params) -> anyhow::Result<()> {
    println!("👶 Generate a security token...");

    let mut cek_file = None;
    for (option, option_value) in &params.options {
        match option.as_str() {
            "-c" | "--cek_file" => {
                cek_file = Some(option_value.clone());
            }
            opt => {
                return Err(anyhow!("💣 Unknown parameter, option=[{}]", opt))
            }
        }
    }

    let cek  = read_cek_from_file(& Path::new(&cek_file.ok_or(anyhow!("Wrong cek value"))?))?;
    let clear_token = serde_json::to_string(&ClearSecurityToken::new())?;
    let security_token = DkEncrypt::encrypt_str(&clear_token, &cek)?;

    write_security_token(&security_token)?;

    println!("Text Security token: {}", &clear_token);
    println!("😎 Security token generated successfully, token : {}... ", &security_token[0..7]);
    Ok(())
}


fn write_security_token(security_token: &str) -> anyhow::Result<()> {
    let mut file = File::create(get_target_file("config/token.id")?)?;
    // Write a byte string.
    file.write_all(&security_token.to_string().into_bytes()[..])?;
    println!("💾 Security token stored");
    Ok(())
}

pub (crate) fn read_security_token() -> anyhow::Result<String> {
    let file = File::open(get_target_file("config/token.id")?)?;
    let mut buf_reader = BufReader::new(file);
    let mut content: String = "".to_string();
    let _ = buf_reader.read_to_string(&mut content)?;
    Ok(content)
}