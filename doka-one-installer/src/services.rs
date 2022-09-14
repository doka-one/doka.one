use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use commons_error::*;
use crate::{Config, DEF_FILE_TEMPLATE, step_println};

fn uninstall_service(config: &Config, service_id: &str) -> anyhow::Result<()> {
    // serman install key_manager.xml --overwrite
    let serman_program = format!( "{}/bin/serman/serman.exe", &config.installation_path);

    // println!("{service_id}, {serman_program}, {service_definition_file}");

    // uninstall the service

    let _the_output = Command::new(serman_program.as_str()).args(&["uninstall", service_id]).output()
        .map_err(eprint_fwd!("Cannot uninstall the service: {service_id}" ))?;

    println!("Service uninstalled: {service_id}");

    sleep(Duration::from_secs(4));

    // clear the service by stopping it

    let _the_output = Command::new("sc").args(&["stop", service_id]).output()
        .map_err(eprint_fwd!("Cannot stop the service" ))?;

    println!("Service cleared: {service_id}");

    sleep(Duration::from_secs(4));

    Ok(())
}

///
/// Create a standalone windows service.
/// In the serman folder, we must have a definition file <service_id>.xml
///
fn create_service(config: &Config, service_id: &str) -> anyhow::Result<()> {

    // serman install key_manager.xml --overwrite
    let serman_program = format!( "{}/bin/serman/serman.exe", &config.installation_path);

    // TODO check if the definition file exists
    let service_definition_file = format!("{}/service-definitions/{}.xml", &config.installation_path, service_id);

    //println!("{service_id}, {serman_program}, {service_definition_file}");

    // install the service

    let _the_output = Command::new(serman_program.as_str()).args(&["install", &service_definition_file, "--overwrite"]).output()
        .map_err(eprint_fwd!("Cannot create the service" ))?;

    println!("Service created: {service_id}");

    Ok(())
}


pub(crate) fn uninstall_windows_services(config: &Config) -> anyhow::Result<()> {
    let _ = step_println("Uninstall windows services ...");

    uninstall_service(config, "key-manager")?;
    uninstall_service(config,  "session-manager")?;
    // uninstall_service(config,  "admin-server")?;
    // uninstall_service(config,  "document-server")?;
    // uninstall_service(config,  "file-server")?;
    // uninstall_service(config,  "tika-server")?;

    Ok(())
}

pub(crate) fn build_windows_services(config: &Config) -> anyhow::Result<()> {
    let _ = step_println("Creating windows services ...");

    create_service(config, "key-manager")?;
    create_service(config,  "session-manager")?;
    // create_service(config,  "admin-server")?;
    // create_service(config,  "document-server")?;
    // create_service(config,  "file-server")?;
    // create_service(config,  "tika-server")?;

    Ok(())
}



///
/// This should generate a correct definiton file for the windows service.
/// Nevertheless, we must generate the doka config files for each services before.
fn write_service_definition_file(config: &Config, service_id: &str,  service_name: &str) -> anyhow::Result<()> {

    println!("Write service definition for {service_id}");

    // ex : D:\test_install\doka.one\bin\key-manager\key-manager.exe
    let executable = format!("{}/bin/{service_id}/{service_id}.exe", &config.installation_path);
    let my_env = format!("{}/doka-configs/{}", &config.installation_path, &config.instance_name);

    // dbg!(&executable, &my_env);

    let mut definition = String::from(DEF_FILE_TEMPLATE);
    // TODO : we should XML escape the replacement values, but for now, we know what we are doing.
    definition = definition.replace("{SERVICE_ID}", service_id)
        .replace("{SERVICE_NAME}", service_name)
        .replace("{EXECUTABLE}", &executable)
        .replace("{MY_ENV}", &my_env);

    // dbg!(&definition);

    let definiton_file = Path::new(config.installation_path.as_str())
        .join("service-definitions/")
        .join(format!("{service_id}.xml"));
    let _ = std::fs::write(&definiton_file, &definition);

    println!("Write service definition for {service_id}");

    Ok(())
}

///
///
pub (crate) fn write_all_service_definition(config: &Config) -> anyhow::Result<()> {

    write_service_definition_file(&config, "key-manager", "Doka Key Manager")
        .map_err(eprint_fwd!("Write definition file failed"))?;

    write_service_definition_file(&config, "session-manager", "Doka Session Manager")
        .map_err(eprint_fwd!("Write definition file failed"))?;


    Ok(())
}