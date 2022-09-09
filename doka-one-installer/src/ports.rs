use anyhow::anyhow;
use portpicker::{is_free, Port};

#[derive(Debug)]
pub(crate) struct Ports {
    pub key_manager: u16,
    pub session_manager: u16,
    pub admin_server: u16,
    pub document_server: u16,
    pub file_server: u16,
    pub tika_server: u16,
}

///
///
fn test_ports(starting_port: Port) -> anyhow::Result<Port> {
    const RANGE : u16 = 10;
    let mut tested_port = starting_port;
    let found_port : Option<Port>;
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

    let port = found_port.ok_or(anyhow!("Port still not defined, last test port {tested_port}"))?;

    Ok(port)
}


pub(crate) fn find_service_port() -> anyhow::Result<Ports> {

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
