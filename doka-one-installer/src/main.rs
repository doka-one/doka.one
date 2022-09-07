#![feature(let_else)]

use std::{fs, io};
use std::fmt::format;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Duration;

struct Config {
    pub installation_path: String,
    pub db_host: String,
    pub db_user_name: String,
    pub db_user_password: String,
    pub instance_name: String,
}

struct Ports {
    pub key_manager: u16,
    pub session_manager: u16,
    pub admin_server: u16,
    pub document_server: u16,
    pub file_server: u16,
    pub tika_server: u16,
}

///
///   <intallation_path>
///             /artefacts
///             /bin
///                 /key-manager
///                 /session-manager
///                 ....
///                 /tika
///                 /serman
///                 /jdk
///             /services
///             /doka-configs
///                 /logs
///                 /config
///                     /key-manager
///                     /session-manager
///
fn read_basic_install_info() -> anyhow::Result<Config> {
    println!("Read basic install information ...");
    let installation_path = "d:/test_install/doka.one".to_string();
    let db_host = "localhost:5432".to_string();
    let db_user_name = "denis".to_string();
    let db_user_password = "Oratece4.".to_string();
    let instance_name = "test_1".to_string();

    Ok(Config {
        installation_path,
        db_host,
        db_user_name,
        db_user_password,
        instance_name
    })
}

fn verification(config: &Config) -> anyhow::Result<()> {
    println!("Verification ...");
    Ok(())
}


const TIMEOUT : Duration = Duration::from_secs(60 * 60); // 60 min

fn get_binary_data( url : &str) -> anyhow::Result<bytes::Bytes> {
    let request_builder = reqwest::blocking::Client::new().get(url).timeout(TIMEOUT);
    let response = request_builder.send()?; // .bytes().unwrap();
    Ok(response.bytes()?)
}


/// Download the artefact into the <install_dir>/artefacts  folder
fn download_file(config: &Config, artefact_name: &str) -> anyhow::Result<()> {

    println!("Downloading artefacts {artefact_name}");

    let zip_file = format!("{}.zip", artefact_name);

    // TODO Please create the correct folders before...
    let p = Path::new(&config.installation_path ).join("artefacts").join(&zip_file);

    // TODO test the https when the certificate is correct...
    let url = format!("http://doka.one/artefacts/0.1.0/{}", zip_file);

    let bin_artefact = get_binary_data(&url)?;

    let mut file = std::fs::File::create(&p)?;
    let mut content = Cursor::new(bin_artefact);
    std::io::copy(&mut content, &mut file)?;

    Ok(())
}


fn unzip(config: &Config, artefact_name: &str) -> anyhow::Result<()> {

    let zip_file = format!("{}.zip", artefact_name);

    let path  = Path::new(&config.installation_path ).join("artefacts").join(&zip_file);
dbg!(&path);
    let archive = fs::read(path)?;

    let target_dir  = Path::new(&config.installation_path ).join("bin").join(artefact_name);
    // let target_dir = PathBuf::from("my_target_dir"); // Doesn't need to exist

    dbg!(&target_dir);

    // The third parameter allows you to strip away toplevel directories.
    // If `archive` contained a single directory, its contents would be extracted instead.
    let r = zip_extract::extract(Cursor::new(archive), &target_dir, true);

    // my_unzip(&path, &target_dir);

    dbg!(&r);

    Ok(())
}



fn download_artefacts(config: &Config) -> anyhow::Result<()> {
    println!("Downloading artefacts ...");

    // key-manager
    download_file(&config,  "key-manager")?;

    // session-manager
    download_file(&config,  "session-manager")?;

    // ...

    // serman : https://www.dropbox.com/s/i7gptd0l289250t/serman.zip?dl=0
    download_file(&config,  "serman")?;

    // tika : https://www.dropbox.com/s/ftsf0elcal7pyqj/tika-server.zip?dl=0
    download_file(&config,  "tika-server")?;

    // jdk
    download_file(&config,  "jdk-17")?;

    // Unzip artefacts
    unzip(&config,  "key-manager")?;
    unzip(&config,  "session-manager")?;
    unzip(&config,  "serman")?;
    unzip(&config,  "tika-server")?;
    unzip(&config,  "jdk-17")?;

    Ok(())
}

fn initialization(config: &Config) -> anyhow::Result<Ports> {
    println!("Service initialization ...");
    println!("Ports search ...");

    Ok(Ports{
        key_manager: 30_040,
        session_manager: 30_050,
        admin_server: 30_060,
        document_server: 30_070,
        file_server: 30_080,
        tika_server: 40_010,
    })
}

fn start_services(config: &Config) -> anyhow::Result<()> {
    println!("Service startup ...");
    Ok(())
}


fn main() {
    println!("Installing Doka One...");

    let config = match  read_basic_install_info()  {
        Ok(config) => {
            config
        }
        Err(e) => {
            eprintln!("💣 Cannot read the config");
            exit(42);
        }
    };


    if let Err(e) = download_artefacts(&config)  {
        eprintln!("💣 Cannot download, {:?}", e);
        exit(43);
    };

}
