use std::env;
use std::env::current_exe;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::ops::Add;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use dkcrypto::dk_crypto::DkEncrypt;

///
///
///
// pub (crate) fn token_command(params: &Params) -> anyhow::Result<()> {
//     match params.action.as_str() {
//         "generate" => {
//             token_generate(&params)
//         }
//         action => {
//             Err(anyhow!("💣 Unknown action=[{}]", action))
//         }
//     }
// }

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
pub fn token_generate(cek_file : &str) -> anyhow::Result<()> {
    println!("👶 Generate a security token...");

    let cek  = read_cek_from_file(& Path::new(&cek_file))?;
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

pub fn read_security_token() -> anyhow::Result<String> {
    let file = File::open(get_target_file("config/token.id")?)?;
    let mut buf_reader = BufReader::new(file);
    let mut content: String = "".to_string();
    let _ = buf_reader.read_to_string(&mut content)?;
    Ok(content)
}

/// Get the location of a file into the working folder
pub fn get_target_file(termnination_path: &str) -> anyhow::Result<PathBuf> {

    let doka_cli_env = env::var("DOKA_CLI_ENV").unwrap_or("".to_string());

    if ! doka_cli_env.is_empty() {
        Ok(Path::new(&doka_cli_env).join("doka-cli").join(termnination_path).to_path_buf())
    } else {
        let path = current_exe()?; //
        let parent_path = path.parent().ok_or(anyhow!("Problem to identify parent's binary folder"))?;
        Ok(parent_path.join(termnination_path))
    }
}
