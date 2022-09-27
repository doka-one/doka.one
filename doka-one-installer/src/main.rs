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
fn read_basic_install_info(args: MyArgs) -> anyhow::Result<Config> {

    let _ = step_println("Get the install informations...")?;

    let db_user_password = match args.db_user_password {
        None => {
            let password = rpassword::prompt_password("Enter your PostgreSQL password : ").unwrap();
            password
        }
        Some(v) => {v}
    };

    println!("Done. Install information.");

    Ok(Config {
        installation_path : args.installation_path,
        db_host : args.db_host,
        db_port : args.db_port,
        db_user_name : args.db_user_name,
        db_user_password,
        instance_name : args.instance_name,
        release_number : args.release_number,
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
    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("artefacts").join(&config.release_number))?; // release number
    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("bin"))?;
    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("service-definitions"))?;
    // ex : D:\test_install\doka.one\doka-configs\prod_1
    let _ = fs::create_dir_all(&Path::new(&config.installation_path).join("doka-configs").join(&config.instance_name))?;

    create_std_doka_service_folders(&config, "key-manager")?;
    create_std_doka_service_folders(&config, "session-manager")?;
    create_std_doka_service_folders(&config,  "admin-server")?;
    create_std_doka_service_folders(&config,  "document-server")?;
    create_std_doka_service_folders(&config,  "file-server")?;

    create_std_doka_service_folders(&config,  "tika-server")?;
    create_std_doka_service_folders(&config,  "doka-cli")?;

    Ok(())
}

use clap::Parser;

/// Doka Installer for Windows
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct MyArgs {
    /// Path to a local folder where to install doka
    /// `Ex : D:\app\doka.one`
    #[clap(short='i', long, display_order=1, value_parser)]
    installation_path: String,

    /// Machine name which hosts the database
    /// `Ex : doka.one`
    #[clap(short='H', long, display_order=2,value_parser)]
    db_host: String,

    /// Port on which the database runs
    /// `Ex : 5432`
    #[clap(short='P', long, display_order=3,value_parser)]
    db_port: u16,

    /// Database user name
    /// `Ex : john`
    #[clap(short='u', long, display_order=4,value_parser)]
    db_user_name: String,

    /// Database user password (optional)
    /// `Ex : doo`
    #[clap(short='p', display_order=5,long, required=false, value_name="[DB_USER_PASSWORD]", value_parser)]
    db_user_password: Option<String>,

    /// Doka instance name
    /// `Ex : prod_1`
    #[clap(short='I', long, display_order=6,value_parser)]
    instance_name: String,

    /// Doka release number
    #[clap(short='r', short, display_order=7,long, possible_values=["0.1.0", "0.2.0"], value_parser)]
    release_number: String,

}


/*
  doka-one-installer.exe \
         --installation-path "D:/test_install/doka.one" \
         --db-host "localhost" \
         --db-port "5432" \
         --db-user-name "denis" \
         --db-user-password "xxx" \
         --instance-name "test_2" \
         --release-number "0.1.0"
*/
fn main() {

    let args = MyArgs::parse();

    let _ = main_println("Installing Doka One...");

    // Phase 1 Enter the install information

    let config = match  read_basic_install_info(args) {
        Ok(config) => {
            config
        }
        Err(e) => {
            eprintln!("💣 Cannot read the config, {}", e);
            exit(10);
        }
    };

    // Phase 2 : Verification

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

    // Phase 3a : Uninstall Windows services

    let Ok(_) = uninstall_windows_services(&config).map_err(eprint_fwd!("Uninstall Windows services failed")) else {
        exit(25);
    };

    // Phase 3b : Download artefacts

    if let Err(e) = download_artefacts(&config) {
        eprintln!("💣 Cannot download, {:?}", e);
        exit(30);
    };


    // Phase 4 : Initialization

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

    // Phase 5 : Start up services

    let Ok(_) = build_windows_services(&config).map_err(eprint_fwd!("Windows services failed")) else {
        exit(60);
    };

    let _ = end_println("Doka installed with success");
}
