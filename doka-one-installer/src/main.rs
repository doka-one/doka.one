#![feature(let_else)]

mod templates;
mod artefacts;
mod config;
mod services;
mod ports;
mod color_text;

use std::{fs};
use std::path::{Path};
use std::process::{exit};
use termcolor::Color;

use commons_error::*;
use crate::artefacts::download_artefacts;
use crate::color_text::{color_println, end_println, step_println};
use crate::config::{Config};
use crate::ports::{find_service_port, Ports};
use crate::services::{build_windows_services, uninstall_windows_services, write_all_service_definition};
use crate::templates::{DEF_FILE_TEMPLATE, KM_APP_PROPERTIES_TEMPLATE};


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
///             /service-definitions
///             /doka-configs
///                 /prod_1
///                     /key-manager
///                         /logs
///                         /config
///                         /keys
///                     /session-manager
///
fn read_basic_install_info() -> anyhow::Result<Config> {
    println!("Read basic install information ...");
    let installation_path = "d:/test_install/doka.one".to_string();
    let db_host = "localhost".to_string();
    let db_port: u16 = 5432;
    let db_user_name = "denis".to_string();
    let db_user_password = "Oratece4.".to_string();
    let instance_name = "dev_1".to_string();

    Ok(Config {
        installation_path,
        db_host,
        db_port,
        db_user_name,
        db_user_password,
        instance_name
    })
}

fn create_std_doka_service_folders(config: &Config, service_id: &str) -> anyhow::Result<()> {
    let _ = fs::create_dir_all(&Path::new(&config.installation_path)
        .join("doka-configs")
        .join(&config.instance_name)
        .join(service_id)
        .join("logs")
    )?;

    let _ = fs::create_dir_all(&Path::new(&config.installation_path)
        .join("doka-configs")
        .join(&config.instance_name)
        .join(service_id)
        .join("config")
    )?;

    let _ = fs::create_dir_all(&Path::new(&config.installation_path)
        .join("doka-configs")
        .join(&config.instance_name)
        .join(service_id)
        .join("keys")
    )?;
    Ok(())
}

fn verification(config: &Config) -> anyhow::Result<()> {
    let _ = step_println("Verification...")?;

    let _ = fs::create_dir_all(&config.installation_path).map_err(eprint_fwd!("Error on installation path"))?;

    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("artefacts"))?;

    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("bin"))?;

    // ex : D:\test_install\doka.one\doka-configs\prod_1
    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("doka-configs").join(&config.instance_name))?;

    create_std_doka_service_folders(&config, "key-manager")?;
    create_std_doka_service_folders(&config, "session-manager")?;
    create_std_doka_service_folders(&config,  "admin-server")?;
    create_std_doka_service_folders(&config,  "document-server")?;
    create_std_doka_service_folders(&config,  "file-server")?;

    Ok(())
}


fn generate_key_manager_app_properties(config: &Config, ports: &Ports) -> anyhow::Result<()> {

    let _ = step_println("Generate Doka Services property files");

    println!("Generate application.properties for key-manager");

    // ex : D:\test_install\doka.one\bin\key-manager\key-manager.exe
    let instance_name = &config.instance_name;
    let km_port =  ports.key_manager;
    let km_cek = format!("{}/doka-configs/{instance_name}/key-manager/keys/cek.key", &config.installation_path);
    let db_host =  &config.db_host;
    let db_port = &config.db_port;

    let db_user = &config.db_user_name;
    let db_password = &config.db_user_password;
    let km_log4rs = format!("{}/doka-configs/{instance_name}/key-manager/config/log4rs.yaml", &config.installation_path);

    let mut properties_file_content = String::from(KM_APP_PROPERTIES_TEMPLATE);
    // TODO : we should property escape the replacement values, but for now, we know what we are doing.
    properties_file_content = properties_file_content
        .replace("{KM_PORT}", &format!("{}", km_port))
        .replace("{KM_CEK}", &km_cek)
        .replace("{DB_HOST}", db_host)
        .replace("{DB_PORT}", &format!("{}", db_port))
        .replace("{DOKA_INSTANCE}", instance_name)
        .replace("{DB_USER}", db_user)
        .replace("{DB_PASSWORD}", db_password)
        .replace("{KM_LOG4RS}", &km_log4rs);

    // dbg!(&properties_file_content);

    let properties_file = Path::new(config.installation_path.as_str())
        .join("doka-configs")
        .join( instance_name)
        .join( "key-manager")
        .join( "config")
        .join("application.properties");

    fs::write(&properties_file, &properties_file_content)
        .map_err(eprint_fwd!("Cannot create the properties file for key-manager"))?;

    println!("Done. Generate application.properties for key-manager");

    Ok(())
}

fn main() {
    let _ = step_println("Installing Doka One...");

    let config = match  read_basic_install_info() {
        Ok(config) => {
            config
        }
        Err(e) => {
            eprintln!("💣 Cannot read the config, {}", e);
            exit(10);
        }
    };


    let Ok(_) = verification(&config)
        .map_err(eprint_fwd!("Verification failed")) else {
        exit(20);
    };

    let Ok(_) = uninstall_windows_services(&config).map_err(eprint_fwd!("Uninstall Windows services failed")) else {
        exit(25);
    };

    if let Err(e) = download_artefacts(&config) {
        eprintln!("💣 Cannot download, {:?}", e);
        exit(30);
    };


    let Ok(ports) = find_service_port().map_err(eprint_fwd!("Port search failed")) else {
        exit(40);
    };


    let Ok(_) = generate_key_manager_app_properties(&config, &ports).map_err(eprint_fwd!("")) else {
        exit(45);
    };

    let Ok(_) = write_all_service_definition(&config)
        .map_err(eprint_fwd!("Write definition file failed")) else {
        exit(50);
    };

    let Ok(_) = build_windows_services(&config).map_err(eprint_fwd!("Windows services failed")) else {
        exit(60);
    };

    // TODO call the http://localhost:30040/key-manager/health  request to ensure all is working.


    let _ = end_println("Doka installed with success");
}
