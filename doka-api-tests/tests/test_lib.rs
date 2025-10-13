use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use lazy_static::*;
use rs_uuid::iso::uuid_v4;

use common_config::conf_reader::{read_config, read_env};
use dkdto::web_types::{CreateCustomerRequest, LoginRequest};
use doka_cli::request_client::AdminServerClient;

pub enum TestStatus {
    INIT,
    DONE,
}

pub struct Lookup<'a> {
    test_name: String,
    test_to_run: &'a [&'a str],
    props: HashMap<String, String>,
}

impl<'a> Lookup<'a> {
    pub fn new(test_name: &str, test_to_run: &'a [&'a str]) -> Self {
        let props = read_props();
        init_test(test_name, &props);
        Lookup { test_name: test_name.to_string(), test_to_run, props }
    }
    // TODO REF_TAG : UNIFORMIZE_INIT
    pub fn props(&self) -> HashMap<String, String> {
        let test_env = Lookup::read_test_env();
        let mut props = self.props.clone();
        props.insert("customer.code".to_string(), test_env.customer_code);
        props.insert("login".to_string(), test_env.login);
        props.insert("password".to_string(), test_env.password);
        props
    }

    #[allow(dead_code)]
    pub fn read_test_env() -> TestEnv {
        let env = TEST_ENV.lock().unwrap();
        let test_env = env.clone();
        test_env
    }

    pub fn close(&self) {
        eprintln!("Closing the lookup: {}", self.test_name);
    }

    /// Run from the Drop
    pub fn close_test(&self) {
        use commons_error::*;
        use log::*;

        let mut test_list = TEST_LIST.lock().unwrap();
        if test_list.contains_key(&self.test_name.to_owned()) {
            // test_list.remove(&test_name.to_owned());
            test_list.insert(self.test_name.to_string(), TestStatus::DONE);
            eprintln!();
            eprintln!("⚪️ ****** Unregister the test : {} (Test left [{}])", &self.test_name, test_list.len());
            eprintln!();
        }

        if is_all_terminated(test_list, &self.test_to_run) {
            eprintln!();
            eprintln!("🚀 ****** All is terminated - Start the close tests process");
            eprintln!();
            let test_env = TEST_ENV.lock().unwrap();

            // Drop the new schema
            let admin_server = AdminServerClient::new("localhost", 30060);
            let reply = admin_server.delete_customer(
                &test_env.customer_code,
                self.props.get("dev.token").unwrap(), /*&test_env.token*/
            );

            if let Err(_e) = reply {
                log_error!("Error while deleting the schema, schema=[{}]", &test_env.customer_code);
                // dbg!(&e);
            }

            eprintln!("Deleted customer, [{}]", &test_env.customer_code);

            // eprintln!();
            eprintln!("🏁 ****** End the close tests process");
            eprintln!();
        }
    }
}

/// The Lookup struct will run this code as soon as it goes out of scope.
/// It ensure the database will be cleared of data after the UT ends.
impl<'a> Drop for Lookup<'a> {
    fn drop(&mut self) {
        self.close_test();
        eprintln!("Dropping MyStruct with data: {}", self.test_name);
    }
}

// TODO REF_TAG : UNIFORMIZE_INIT
#[derive(Debug, Clone)]
pub struct TestEnv {
    pub customer_code: String,
    pub login: String,
    pub password: String,
}

lazy_static! {
    static ref TEST_ENV: Mutex<TestEnv> =
        Mutex::new(TestEnv { customer_code: "".to_string(), login: "".to_string(), password: "".to_string() });
}

lazy_static! {
    static ref TEST_LIST: Mutex<HashMap<String, TestStatus>> = Mutex::new(HashMap::new());
}

lazy_static! {
    static ref IS_INIT_MUT: Mutex<bool> = Mutex::new({ false });
}

pub fn read_props() -> HashMap<String, String> {
    read_config("doka-test", &read_env("DOKA_UT_ENV"), &Some("DOKA_CLUSTER_PROFILE".to_string()))
}

pub fn init_test(test_name: &str, props: &HashMap<String, String>) {
    {
        let mut test_list = TEST_LIST.lock().unwrap();
        test_list.insert(test_name.to_string(), TestStatus::INIT); // means the test has started

        eprintln!();
        eprintln!("🔨 ****** Register the test : {} (Test present [{}])", test_name, test_list.len());
        eprintln!();
    }

    let mut is_init = IS_INIT_MUT.lock().unwrap();
    if *is_init == false {
        eprintln!();
        eprintln!("🚀 ****** Start the init tests process");
        eprintln!();

        // Init : Create the schema (if not exist), create the admin user (if not exist)
        let admin_server = AdminServerClient::new("localhost", 30060);

        let dev_token = props.get("dev.token").unwrap();
        let customer_name_format = props.get("customer.name.format").unwrap().to_owned();
        let email_format = props.get("email.format").unwrap();
        let admin_password = props.get("admin.password").unwrap().to_owned();

        let login_id = uuid_v4();
        let request = CreateCustomerRequest {
            customer_name: customer_name_format.replace("{}", &login_id),
            email: email_format.replace("{}", &login_id),
            admin_password,
        };

        let wr_reply = admin_server.create_customer(&request, dev_token);

        match wr_reply {
            Ok(reply) => {
                let te = TestEnv {
                    customer_code: reply.customer_code.clone(),
                    login: request.email.clone(),
                    password: request.admin_password.clone(),
                };

                let reply = admin_server.customer_removable(&reply.customer_code, dev_token);
                if let Err(e) = reply {
                    panic!("Error code [{:?}]", &e);
                }

                let mut test_env = TEST_ENV.lock().unwrap();
                *test_env = te;

                eprintln!("Created customer, [{}]", &test_env.customer_code);
                eprintln!("🏁 ****** End the init tests process");
                eprintln!();
            }
            Err(e) => {
                panic!("Error code [{:?}]", &e);
            }
        }
    }
    *is_init = true;
}

fn is_all_terminated(list: MutexGuard<HashMap<String, TestStatus>>, test_to_run: &[&str]) -> bool {
    // Vérifier si tous les éléments de la map sont à "DONE"
    if list.values().all(|status| match status {
        TestStatus::DONE => true,
        _ => false,
    }) {
        // Vérifier si tous les éléments de test_to_run sont dans la map
        let all_tests_present = test_to_run.iter().all(|test| list.contains_key(*test));
        all_tests_present
    } else {
        false
    }
}

pub fn get_login_request(props: &HashMap<String, String>) -> LoginRequest {
    LoginRequest { login: props.get("login").unwrap().to_owned(), password: props.get("password").unwrap().to_owned() }
}
