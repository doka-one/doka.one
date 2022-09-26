
use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::sync::{RwLock};
use anyhow::anyhow;
use lazy_static::lazy_static;
use portpicker::Port;

use commons_error::*;
use dkcrypto::dk_crypto::DkEncrypt;
use crate::{Config, STD_APP_PROPERTIES_TEMPLATE, Ports, step_println};
use crate::templates::{ADMIN_SERVER_APP_PROPERTIES_TEMPLATE, DOCUMENT_SERVER_APP_PROPERTIES_TEMPLATE, FILE_SERVER_APP_PROPERTIES_TEMPLATE, LOG4RS_TEMPLATE, TIKA_CONFIG_TEMPLATE, TIKA_LOG4J_TEMPLATE};

type ReplacementProcess = fn(config: &Config, ports: &Ports) -> String;

fn std_replacement_process(config: &Config, _ports: &Ports, service_name: &str, service_port : Port, template: &str) -> String {
    // ex : D:\test_install\doka.one\bin\key-manager\key-manager.exe
    let km_cek = format!("{}/doka-configs/{}/{service_name}/keys/cek.key", &config.installation_path, &config.instance_name);
    let km_log4rs = format!("{}/doka-configs/{}/{service_name}/config/log4rs.yaml", &config.installation_path, &config.instance_name );

    template
        .replace("{SERVICE_PORT}", &format!("{}", service_port))
        .replace("{SERVICE_CEK}", &km_cek)
        .replace("{DB_HOST}", &config.db_host)
        .replace("{DB_PORT}", &format!("{}", &config.db_port))
        .replace("{DOKA_INSTANCE}", &config.instance_name)
        .replace("{DB_USER}", &config.db_user_name)
        .replace("{DB_PASSWORD}", &config.db_user_password)
        .replace("{SERVICE_LOG4RS}", &km_log4rs)

}



fn generate_key_manager_app_properties(config: &Config, ports: &Ports) -> anyhow::Result<()> {

    let replacement : ReplacementProcess = |config: &Config, ports: &Ports| {
        std_replacement_process(config, ports, "key-manager", ports.key_manager, STD_APP_PROPERTIES_TEMPLATE)
    };

    generate_service_app_properties(config, ports, "key-manager", replacement)
}


fn generate_session_manager_app_properties(config: &Config, ports: &Ports) -> anyhow::Result<()> {

    let replacement : ReplacementProcess = |config: &Config, ports: &Ports| {
        std_replacement_process(config, ports, "session-manager", ports.session_manager, STD_APP_PROPERTIES_TEMPLATE)
    };

    generate_service_app_properties(config, ports, "session-manager",  replacement)
}

fn generate_admin_server_app_properties(config: &Config, ports: &Ports) -> anyhow::Result<()> {

    let admin_server_replacement_process : ReplacementProcess = |config: &Config, ports: &Ports| {
        std_replacement_process(config, ports, "admin-server", ports.admin_server, ADMIN_SERVER_APP_PROPERTIES_TEMPLATE)
            .replace("{KM_HOST}", "localhost")
            .replace("{KM_PORT}", & ports.key_manager.to_string())
            .replace("{SM_HOST}", "localhost")
            .replace("{SM_PORT}", & ports.session_manager.to_string())
    };

    generate_service_app_properties(config, ports, "admin-server", admin_server_replacement_process)
}

fn generate_document_server_app_properties(config: &Config, ports: &Ports) -> anyhow::Result<()> {

    let admin_server_replacement_process : ReplacementProcess = |config: &Config, ports: &Ports| {
        std_replacement_process(config, ports, "document-server", ports.document_server, DOCUMENT_SERVER_APP_PROPERTIES_TEMPLATE)
            .replace("{KM_HOST}", "localhost")
            .replace("{KM_PORT}", & ports.key_manager.to_string())
            .replace("{SM_HOST}", "localhost")
            .replace("{SM_PORT}", & ports.session_manager.to_string())
            .replace("{TKS_HOST}", "localhost")  // TKS is for TIKA Server
            .replace("{TKS_PORT}", & ports.tika_server.to_string())
    };

    generate_service_app_properties(config, ports, "document-server", admin_server_replacement_process)
}

fn generate_file_server_app_properties(config: &Config, ports: &Ports) -> anyhow::Result<()> {

    let admin_server_replacement_process : ReplacementProcess = |config: &Config, ports: &Ports| {
        std_replacement_process(config, ports, "file-server", ports.file_server, FILE_SERVER_APP_PROPERTIES_TEMPLATE)
            .replace("{KM_HOST}", "localhost")
            .replace("{KM_PORT}", & ports.key_manager.to_string())
            .replace("{SM_HOST}", "localhost")
            .replace("{SM_PORT}", & ports.session_manager.to_string())
            .replace("{DS_HOST}", "localhost")
            .replace("{DS_PORT}", & ports.document_server.to_string())
            .replace("{TKS_HOST}", "localhost")  // TKS is for TIKA Server
            .replace("{TKS_PORT}", & ports.tika_server.to_string())
    };

    generate_service_app_properties(config, ports, "file-server", admin_server_replacement_process)
}


fn generate_service_app_properties(config: &Config, ports: &Ports, service_name: &str, replacement_process : ReplacementProcess) -> anyhow::Result<()> {
    println!("Generate application.properties for {service_name}");

    let properties_file_content = replacement_process(config, ports);

    let properties_file = Path::new(config.installation_path.as_str())
        .join("doka-configs")
        .join( &config.instance_name)
        .join( service_name)
        .join( "config")
        .join("application.properties");

    fs::write(&properties_file, &properties_file_content)
        .map_err(eprint_fwd!("Cannot create the properties file for {service_name}"))?;

    println!("Done. Generate application.properties for {service_name}");

    // Log4rs
    let _ = generate_log4rs_config(config, service_name)?;

    // cek.key
    let _ = generate_cek_file(config, service_name)?;

    Ok(())
}



fn generate_log4rs_config(config: &Config, service_id: &str) -> anyhow::Result<()> {

    println!("Generate log4rs.yaml for {service_id}");

    // D:\test_install\doka.one\doka-configs\test_1\admin-server\logs\admin-server.log
    let log_folder = format!("{}/doka-configs/{}/{service_id}/logs/{service_id}.log", &config.installation_path, &config.instance_name );

    let log4rs_file_content = LOG4RS_TEMPLATE.replace("{LOG_FOLDER}", &log_folder);

    let log4rs_file = Path::new(config.installation_path.as_str())
        .join("doka-configs")
        .join( &config.instance_name)
        .join(service_id)
        .join( "config")
        .join("log4rs.yaml");

    fs::write(&log4rs_file, &log4rs_file_content)
        .map_err(eprint_fwd!("Cannot create the log4rs.yaml file for {service_id}"))?;

    println!("Done. Generate log4rs.yaml for {service_id}");

    Ok(())
}


fn generate_log4j_config_for_tika(config: &Config) -> anyhow::Result<()> {

    let service_id : &str = "tika-server";
    println!("Generate log4j.xml for {service_id}");

    let log4j_file_content = TIKA_LOG4J_TEMPLATE
                                        .replace("{INSTALL_DIR}", &config.installation_path)
                                        .replace("{DOKA_INSTANCE}", &config.instance_name)
                                        .replace("{SERVICE_ID}", service_id );

    let log4j_file = Path::new(config.installation_path.as_str())
        .join("doka-configs")
        .join( &config.instance_name)
        .join( service_id)
        .join( "config")
        .join("log4j.xml");

    fs::write(&log4j_file, &log4j_file_content)
        .map_err(eprint_fwd!("Cannot create the log4j.xml file for {service_id}, log4j_file=[{}]", log4j_file.to_str().unwrap_or("Unknown")))?;

    println!("Done. Generate log4j.xml for {service_id}");

    Ok(())
}

fn generate_config_for_tika(config: &Config, ports: &Ports) -> anyhow::Result<()> {
    let service_id : &str = "tika-server";
    println!("Generate tika-config.xml for {service_id}");

    let log4j_path = format!("{}/doka-configs/{}/{service_id}/config/log4j.xml", &config.installation_path, &config.instance_name );

    let tika_config_file_content = TIKA_CONFIG_TEMPLATE
        .replace("{TIKA_PORT}", &ports.tika_server.to_string())
        .replace("{LOG4J_PATH}", &log4j_path);

    let tika_config_file = Path::new(config.installation_path.as_str())
        .join("doka-configs")
        .join( &config.instance_name)
        .join( service_id)
        .join( "config")
        .join("tika-config.xml");

    fs::write(&tika_config_file, &tika_config_file_content)
        .map_err(eprint_fwd!("Cannot create the tika-config.xml file for {service_id}, tika_config_file=[{}]",
            tika_config_file.to_str().unwrap_or("Unknown")))?;

    println!("Done. Generate tika-config.xml for {service_id}");

    Ok(())
}


lazy_static! {
    static ref CEK : RwLock<Option<String>> = RwLock::new(None);
}

fn generate_cek_file(config: &Config, service_name: &str) -> anyhow::Result<()> {
    println!("Generate cek.key for {service_name}");

    let cek_file = Path::new(config.installation_path.as_str())
        .join("doka-configs")
        .join( &config.instance_name)
        .join( service_name)
        .join( "keys")
        .join("cek.key");

    match cek_file.exists() {
        true => {
            println!("cek.key file already exists for {service_name}. Skip the process.");
        }
        false => {

            // Generate the CEK if needed
            {
                let mut guard_cek = match CEK.write() {
                    Ok(v) => {v}
                    Err(_) => {
                        return Err(anyhow!("Cannot unlock the CEK from memory"));
                    }
                };

                if guard_cek.is_none() {
                    guard_cek.replace(DkEncrypt::generate_random_key());
                }

                dbg!(&guard_cek);
            }

            let t = CEK.read().unwrap();
            let cek = match t.deref() {
                None => {
                    return Err(anyhow!("Cannot read the CEK from memory"));
                }
                Some(v) => {
                    v
                }
            };


            fs::write(&cek_file, cek)
                .map_err(eprint_fwd!("Cannot create the cek.key file for {service_name}"))?;

            println!("Done. Generate cek.key for {service_name}");
        }
    }

    Ok(())
}

pub (crate) fn generate_all_app_properties(config: &Config, ports: &Ports) -> anyhow::Result<()> {

    let _ = step_println("Generate Doka Services property files");

    let _ = generate_key_manager_app_properties(config, ports)?;
    let _ = generate_session_manager_app_properties(config, ports)?;
    let _ = generate_admin_server_app_properties(config, ports)?;
    let _ = generate_document_server_app_properties(config, ports)?;
    let _ = generate_file_server_app_properties(config, ports)?;

    let _ = generate_log4j_config_for_tika(config)?;
    let _ = generate_config_for_tika(config, ports)?;

    Ok(())
}
