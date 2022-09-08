#![feature(let_else)]

use std::{fs, io};
use std::fmt::format;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};
use std::thread::sleep;
use std::time::Duration;
use anyhow::anyhow;
use portpicker::{is_free, Port};

use commons_error::*;

#[derive(Debug)]
struct Config {
    pub installation_path: String,
    pub db_host: String,
    pub db_user_name: String,
    pub db_user_password: String,
    pub instance_name: String,
}

#[derive(Debug)]
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
///                 /test_1
///                     /key-manager
///                         /logs
///                         /config
///                         /keys
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

    println!("Decompress artefacts {artefact_name}");

    let zip_file = format!("{}.zip", artefact_name);

    let path  = Path::new(&config.installation_path ).join("artefacts").join(&zip_file);

    let archive = fs::read(path)?;

    let target_dir  = Path::new(&config.installation_path ).join("bin").join(artefact_name);
    // let target_dir = PathBuf::from("my_target_dir"); // Doesn't need to exist

    dbg!(&target_dir);

    // The third parameter allows you to strip away toplevel directories.
    // If `archive` contained a single directory, its contents would be extracted instead.
    let r = zip_extract::extract(Cursor::new(archive), &target_dir, true);

    // my_unzip(&path, &target_dir);

    println!("Done");

    Ok(())
}



fn download_artefacts(config: &Config) -> anyhow::Result<()> {
    println!("Downloading artefacts ...");

    // Download the doka services and the cli
    download_file(&config,  "key-manager")?;
    download_file(&config,  "session-manager")?;
    download_file(&config,  "admin-server")?;
    download_file(&config,  "document-server")?;
    download_file(&config,  "file-server")?;
    download_file(&config,  "doka-cli")?;

    // Download the extra artefacts
    // | serman : https://www.dropbox.com/s/i7gptd0l289250t/serman.zip?dl=0
    download_file(&config,  "serman")?;
    // | tika : https://www.dropbox.com/s/ftsf0elcal7pyqj/tika-server.zip?dl=0
    download_file(&config,  "tika-server")?;
    // | jdk
    download_file(&config,  "jdk-17")?;


    // Unzip artefacts
    unzip(&config,  "key-manager")?;
    unzip(&config,  "session-manager")?;
    unzip(&config,  "admin-server")?;
    unzip(&config,  "document-server")?;
    unzip(&config,  "file-server")?;
    unzip(&config,  "doka-cli")?;
    unzip(&config,  "serman")?;
    unzip(&config,  "tika-server")?;
    unzip(&config,  "jdk-17")?;

    Ok(())
}

///
///
fn test_ports(starting_port: Port) -> anyhow::Result<Port> {
    const RANGE : u16 = 10;
    let mut tested_port = starting_port;
    let mut found_port : Option<Port> = None;
    loop {
        if is_free(tested_port) {
            found_port = Some(tested_port);
            break;
        }
        tested_port += 1;
        if tested_port - starting_port >= RANGE {
            return Err(anyhow!("No port found between {starting_port} and {}", tested_port-1));
        }
    }

    match found_port {
        None => {
            Err(anyhow!("Port still not defined, last test port {tested_port}"))
        }
        Some(p) => {
            Ok(p)
        }
    }
}


fn find_service_port() -> anyhow::Result<Ports> {

    const PORT_KEY_MANAGER : u16 = 30_040;
    const PORT_SESSION_MANAGER : u16 = 30_050;
    const PORT_ADMIN_SERVER : u16 = 30_060;
    const PORT_DOCUMENT_SERVER : u16 = 30_070;
    const PORT_FILE_SERVER : u16 = 30_080;
    const PORT_TIKA_SERVER : u16 = 40_010;

    println!("Searching ports for services ...");

    let port_key_manager = test_ports(PORT_KEY_MANAGER)?;
    println!("Found port {port_key_manager}");

    let port_session_manager = test_ports(PORT_SESSION_MANAGER)?;
    println!("Found port {port_session_manager}");

    let port_admin_server = test_ports(PORT_ADMIN_SERVER)?;
    println!("Found port {port_admin_server}");

    let port_document_server = test_ports(PORT_DOCUMENT_SERVER)?;
    println!("Found port {port_document_server}");

    let port_file_server = test_ports(PORT_FILE_SERVER)?;
    println!("Found port {port_file_server}");

    let port_tika_server = test_ports(PORT_TIKA_SERVER)?;
    println!("Found port {port_tika_server}");

    Ok(Ports{
        key_manager: port_key_manager,
        session_manager: port_session_manager,
        admin_server: port_admin_server,
        document_server: port_document_server,
        file_server: port_file_server,
        tika_server: port_tika_server,
    })
}

///
/// Create a standalone windows service.
/// In the serman folder, we must have a definition file <service_id>.xml
///
fn create_service(config: &Config, service_id: &str) -> anyhow::Result<()> {
    //let service_id = "key_dummy_1";

    // serman install key_manager.xml --overwrite
    let serman_program = format!( "{}/bin/serman/serman.exe", &config.installation_path);

    // TODO check if the definition file exists
    let service_definition_file = format!("{}/bin/serman/{}.xml", &config.installation_path, service_id);

    println!("{service_id}, {serman_program}, {service_definition_file}");

    // uninstall the service

    let _the_output = Command::new(serman_program.as_str()).args(&["uninstall", service_id]).output()
        .map_err(err_fwd!("Cannot uninstall the service: {service_id}" ))?;

    println!("Service uninstalled: {service_id}");

    sleep(Duration::from_secs(4));

    // clear the service by stopping it

    let _the_output = Command::new("sc").args(&["stop", service_id]).output()
        .map_err(err_fwd!("Cannot stop the service" ))?;

    println!("Service cleared: {service_id}");

    sleep(Duration::from_secs(4));

    // install the service

    let _the_output = Command::new(serman_program.as_str()).args(&["install", &service_definition_file, "--overwrite"]).output()
        .map_err(err_fwd!("Cannot create the service" ))?;

    println!("Service created: {service_id}");

    Ok(())
}

fn build_windows_services(config: &Config) -> anyhow::Result<()> {
    println!("Creating windows services ...");

    create_service(config, "key_manager")?;
    create_service(config,  "session_manager")?;
    // create_service(config,  "admin-server")?;
    // create_service(config,  "document-server")?;
    // create_service(config,  "file-server")?;
    // create_service(config,  "tika-server")?;

    Ok(())
}

const DEF_FILE_TEMPLATE : &str = r#"
<service>
  <id>{SERVICE_ID}</id>
  <name>{SERVICE_NAME}</name>
  <description>{SERVICE_NAME}</description>
  <executable>{EXECUTABLE}</executable>
  <logmode>rotate</logmode>
  <persistent_env name="DOKA_ENV" value="{MY_ENV}" />
</service>
"#;

///
/// This should generate a correct definiton file for the windows service.
/// Nevertheless, we must generate the doka config files for each services before.
fn write_service_definition_file(config: &Config, service_id: &str, service_name: &str) -> anyhow::Result<()> {

    // ex : D:\test_install\doka.one\bin\key-manager\key-manager.exe
    let executable = format!("{}/bin/{service_id}/{service_id}.exe", &config.installation_path);
    let my_env = format!("{}/doka-configs/{}", &config.installation_path, &config.instance_name);

    dbg!(&executable, &my_env);

    let mut definition = String::from(DEF_FILE_TEMPLATE);
    // TODO : we should XML escape the replacement values, but for now, we know what we are doing.
    definition = definition.replace("{SERVICE_ID}", service_id)
        .replace("{SERVICE_NAME}", service_name)
        .replace("{EXECUTABLE}", &executable)
        .replace("{MY_ENV}", &my_env);

    dbg!(&definition);

    let definiton_file = Path::new(config.installation_path.as_str())
        .join("bin/serman/")
        .join(format!("{service_id}.xml"));
    let r = fs::write(&definiton_file, &definition);

    Ok(())
}

///
///
fn write_all_service_definition(config: &Config) -> anyhow::Result<()> {
    // Be careful the service id has an underscore, so has the file name
    write_service_definition_file(&config, "key_manager", "Doka Key Manager")
        .map_err(err_fwd!("Write definition file failed"))?;

    Ok(())
}

fn main() {
    println!("Installing Doka One...");

    let config = match  read_basic_install_info() {
        Ok(config) => {
            config
        }
        Err(e) => {
            eprintln!("💣 Cannot read the config");
            exit(42);
        }
    };

/*    if let Err(e) = download_artefacts(&config) {
        eprintln!("💣 Cannot download, {:?}", e);
        exit(43);
    };*/


/*    let Ok(ports) = find_service_port().map_err(err_fwd!("Port search failed")) else {
        exit(44);
    };

    dbg!(ports);

    let Ok(r) = build_windows_services(&config).map_err(err_fwd!("Windows services failed")) else {
        exit(45);
    };
*/

    let Ok(r) = write_all_service_definition(&config)
                    .map_err(err_fwd!("Write definition file failed")) else {
        exit(46);
    };

}
