#![feature(let_else)]

mod templates;
mod artefacts;
mod config;
mod services;
mod ports;
mod color_text;
mod databases;
mod schema_dokaadmin;
mod schema_dokasys;
mod schema_keymanager;
mod application_properties;

use std::{fs};

use std::path::{Path};
use std::process::{exit};


use commons_error::*;
use crate::application_properties::generate_all_app_properties;
use crate::artefacts::download_artefacts;
use crate::color_text::{end_println, main_println, step_println};
use crate::config::{Config};
use crate::databases::{create_all_admin_schemas, create_databases, test_db_connection};
use crate::ports::{find_service_port, Ports};
use crate::services::{build_windows_services, uninstall_windows_services, write_all_service_definition};
use crate::templates::{DEF_FILE_TEMPLATE, STD_APP_PROPERTIES_TEMPLATE};


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
    let instance_name = "test_1".to_string();

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

    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("service-definitions"))?;

    // ex : D:\test_install\doka.one\doka-configs\prod_1
    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("doka-configs").join(&config.instance_name))?;

    create_std_doka_service_folders(&config, "key-manager")?;
    create_std_doka_service_folders(&config, "session-manager")?;
    create_std_doka_service_folders(&config,  "admin-server")?;
    create_std_doka_service_folders(&config,  "document-server")?;
    create_std_doka_service_folders(&config,  "file-server")?;

    Ok(())
}




fn main() {
    let _ = step_println("Installing Doka One...");

    // Phase 1
    let _ = main_println("Enter the install information");

    let config = match  read_basic_install_info() {
        Ok(config) => {
            config
        }
        Err(e) => {
            eprintln!("💣 Cannot read the config, {}", e);
            exit(10);
        }
    };

    // Phase 2
    let _ = main_println("Verification");

    let Ok(_) = verification(&config)
        .map_err(eprint_fwd!("Verification failed")) else {
        exit(20);
    };

    let Ok(_) = test_db_connection(&config).map_err(eprint_fwd!("Failure while connecting the databases")) else {
        exit(21);
    };

    let Ok(_) = create_databases(&config).map_err(eprint_fwd!("Failure while creating the databases")) else {
        exit(22);
    };


    // Phase 3
    let _ = main_println("Download artefacts");

    let Ok(_) = uninstall_windows_services(&config).map_err(eprint_fwd!("Uninstall Windows services failed")) else {
        exit(25);
    };

    if let Err(e) = download_artefacts(&config) {
        eprintln!("💣 Cannot download, {:?}", e);
        exit(30);
    };


    // Phase 4
    let _ = main_println("Initialization");

    let Ok(ports) = find_service_port().map_err(eprint_fwd!("Port search failed")) else {
        exit(40);
    };


    let Ok(_) = create_all_admin_schemas(&config).map_err(eprint_fwd!("Admin schema creation failed")) else {
        exit(42);
    };


    let Ok(_) = generate_all_app_properties(&config, &ports).map_err(eprint_fwd!("")) else {
        exit(45);
    };

    let Ok(_) = write_all_service_definition(&config)
        .map_err(eprint_fwd!("Write definition file failed")) else {
        exit(50);
    };

    // Phase 5
    let _ = main_println("Start up services");

    let Ok(_) = build_windows_services(&config).map_err(eprint_fwd!("Windows services failed")) else {
        exit(60);
    };

    // TODO call the http://localhost:30040/key-manager/health  request to ensure all is working.


    let _ = end_println("Doka installed with success");
}
