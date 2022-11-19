
use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use commons_error::*;
use crate::{Config, DEF_FILE_TEMPLATE, step_println};
use crate::templates::DEF_FILE_WITH_ARGS_TEMPLATE;

fn uninstall_service(config: &Config, service_id: &str) -> anyhow::Result<()> {
    // serman install key_manager.xml --overwrite
    let serman_program = format!( "{}/bin/serman/serman.exe", &config.installation_path);

    let o =  Command::new(serman_program.as_str()).args(&["uninstall", service_id]).output();


    match o {
        Ok(_) => {

        }
        Err(e) => {
            eprint!("Cannot uninstall the service: {service_id}, e=[{}]", e );
        }
    }

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
    let service_definition_file = format!("{}/service-definitions/{}.xml", &config.installation_path, service_id);

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
    uninstall_service(config,  "admin-server")?;
    uninstall_service(config,  "document-server")?;
    uninstall_service(config,  "file-server")?;
    uninstall_service(config,  "tika-server")?;

    Ok(())
}

pub(crate) fn build_windows_services(config: &Config) -> anyhow::Result<()> {
    let _ = step_println("Creating windows services ...");

    create_service(config, "key-manager")?;
    create_service(config,  "session-manager")?;
    create_service(config,  "admin-server")?;
    create_service(config,  "document-server")?;
    create_service(config,  "file-server")?;
    create_service(config,  "tika-server")?;

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
    // We should XML escape the replacement values, but for now, we know what we are doing.
    definition = definition.replace("{SERVICE_ID}", service_id)
        .replace("{SERVICE_NAME}", service_name)
        .replace("{EXECUTABLE}", &executable)
        .replace("{MY_ENV}", &my_env);

    let definiton_file = Path::new(config.installation_path.as_str())
        .join("service-definitions/")
        .join(format!("{service_id}.xml"));
    let _ = std::fs::write(&definiton_file, &definition);

    println!("Done. Write service definition for {service_id}");

    Ok(())
}

fn write_service_definition_file_for_tika(config: &Config) -> anyhow::Result<()> {

    let service_id: &str = "tika-server";
    let service_name: &str = "Apache Tika Server for Doka";

    println!("Write service definition for {service_id}");

    //   executable : C:\Program Files\Java\jdk-17\bin\java.exe
    let executable = format!("{}/bin/jdk-17/bin/java.exe", &config.installation_path);

    //   arguments : -Dlog4j.configurationFile=file:///D:/test_install/doka.one/doka-configs/test_1/tika-server/config/log4j.xml
    //                  -jar c:\Users\denis\wks-poc\tika\tika-server-standard-2.2.0.jar --port 40010

    // let log4j_path = format!("file:///{}/doka-configs/{}/{}/config/log4j.xml",
    //                          &config.installation_path, &config.instance_name, service_id);

    let tika_config_path = format!("{}/doka-configs/{}/{}/config/tika-config.xml",
                             &config.installation_path, &config.instance_name, service_id);

    let jar_path = format!("{}/bin/{service_id}/tika-server-standard-2.2.0.jar", &config.installation_path);
    // let port_str = ports.tika_server.to_string();
    //let arguments = format!("-Dlog4j.configurationFile={} -jar {} --port {}", &log4j_path, &jar_path, &port_str );
    let arguments = format!("-jar {} -c {}", &jar_path, &tika_config_path );

    let my_env = format!("{}/doka-configs/{}", &config.installation_path, &config.instance_name);

    let mut definition = String::from(DEF_FILE_WITH_ARGS_TEMPLATE);
    // We should XML escape the replacement values, but for now, we know what we are doing.
    definition = definition.replace("{SERVICE_ID}", service_id)
        .replace("{SERVICE_NAME}", service_name)
        .replace("{EXECUTABLE}", &executable)
        .replace("{ARGUMENTS}", &arguments)
        .replace("{MY_ENV}", &my_env);

    // dbg!(&definition);

    let definiton_file = Path::new(config.installation_path.as_str())
        .join("service-definitions/")
        .join(format!("{service_id}.xml"));
    let _ = std::fs::write(&definiton_file, &definition);

    println!("Done. Write service definition for {service_id}");

    Ok(())
}


///
///
///
pub (crate) fn write_all_service_definition(config: &Config) -> anyhow::Result<()> {

    write_service_definition_file(&config, "key-manager", "Doka Key Manager")
        .map_err(eprint_fwd!("Write definition file failed"))?;

    write_service_definition_file(&config, "session-manager", "Doka Session Manager")
        .map_err(eprint_fwd!("Write definition file failed"))?;

    write_service_definition_file(&config, "admin-server", "Doka Admin Server")
        .map_err(eprint_fwd!("Write definition file failed"))?;

    write_service_definition_file(&config, "document-server", "Doka Document Server")
        .map_err(eprint_fwd!("Write definition file failed"))?;

    write_service_definition_file(&config, "file-server", "Doka File Server")
        .map_err(eprint_fwd!("Write definition file failed"))?;

    write_service_definition_file_for_tika(&config).map_err(eprint_fwd!("Write definition file for tika failed"))?;

    Ok(())
}