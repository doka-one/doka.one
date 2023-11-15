#[derive(Debug)]
pub enum OperatingSystem {
    WINDOWS,
    LINUX
}
#[derive(Debug)]
pub(crate) struct Config {
    pub installation_path: String,
    pub db_host: String,
    pub db_port: u16,
    pub db_user_name: String,
    pub db_user_password: String,
    pub instance_name: String,
    pub release_number: String,
    pub operating_system: OperatingSystem,
}


