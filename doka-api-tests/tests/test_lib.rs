use std::collections::HashMap;
use dkdto::{CreateCustomerRequest};
use doka_cli::request_client::AdminServerClient;
use lazy_static::*;
use std::sync::{Mutex, MutexGuard};
use rs_uuid::iso::uuid_v4;

pub enum TestStatus {
    INIT,
    DONE,
}


pub struct Lookup<'a> {
    test_name: String,
    test_to_run :  &'a[&'a str],
}

impl  <'a> Lookup <'a> {
    pub fn new(test_name : &str, test_to_run: &'a [&'a str]) -> Self {
        init_test(test_name);
        Lookup {
            test_name: test_name.to_string(),
            test_to_run: test_to_run,
        }
    }
    // TODO REF_TAG : UNIFORMIZE_INIT
    pub fn props() -> HashMap<String, String> {
        HashMap::new()
    }
    pub fn close(&self) {
        eprintln!("Closing the lookup: {}", self.test_name);
    }
}

/// The Lookup struct will run this code as soon as it goes out of scope.
/// It ensure the database will be cleared of data after the UT ends.
impl <'a> Drop for Lookup<'a> {
    fn drop(&mut self) {
        close_test(&self.test_name, self.test_to_run);
        eprintln!("Dropping MyStruct with data: {}", self.test_name);
    }
}


// TODO REF_TAG : UNIFORMIZE_INIT
#[derive(Debug, Clone)]
pub struct TestEnv {
    pub token: String,
    pub customer_code : String,
    pub login: String,
    pub password: String,
}

#[allow(dead_code)]
pub fn read_test_env() -> TestEnv {
    let env = TEST_ENV.lock().unwrap();
    let test_env = env.clone();
    test_env
}

lazy_static! {
    static ref TEST_ENV: Mutex<TestEnv> = Mutex::new(TestEnv{
        token: "".to_string(),
        customer_code: "".to_string(),
        login: "".to_string(),
        password: "".to_string(),
    });
}

lazy_static! {
    static ref TEST_LIST: Mutex<HashMap<String, TestStatus>> = Mutex::new(HashMap::new());
}

lazy_static! {
    static ref IS_INIT_MUT : Mutex<bool> = Mutex::new(
        {
            false
        });
}

pub fn init_test(test_name : &str) {
    {
        let mut test_list = TEST_LIST.lock().unwrap();
        test_list.insert(test_name.to_string(), TestStatus::INIT); // means the test has started

        eprintln!();
        eprintln!("🔨 ****** Register the test : {} (Test present [{}])", test_name, test_list.len() );
        eprintln!();
    }

    let mut is_init = IS_INIT_MUT.lock().unwrap();
    if *is_init == false {

        eprintln!();
        eprintln!("🚀 ****** Start the init tests process");
        eprintln!();

        // Init : Create the schema (if not exist), create the admin user (if not exist)
        let admin_server = AdminServerClient::new("localhost", 30060);

        // TODO REF_TAG : UNIFORMIZE_INIT
        // This value should depend on the environment we want to run the test.
        // Please refer to the CEK documents to clarify the call of "protected" routines on various environments
        // let r = token_generate("D:/doka.one/doka-configs/dev_6/key-manager/keys/cek.key");

        // FIXME : Generate the token
        // TODO REF_TAG : UNIFORMIZE_INIT
        // on the box
        let dev_token = "EjXpe-RzQeS8tiBIEyY_OlJv35a4cY0i6Zu29Vt3drchg6O3JHBrW9v4F_6jwJPsYTfoQUZMsN_wJLGj-2vIpj3mI0ymBIwU81RUxmPiHbcP2vDFW5jGVg";

        // chacha_1 on laptop
        //let dev_token = "WEzlHVgdvHynkb3I6EHcmx_wUt50TbV0I8xjgE95OEMnSHVaM-erNxBpbC9lRBESKM8XMwT6d1KWY131HY0sMTr2Em-BNMNw3Eq74Hb4p6d1B8DqN22Ygw";

        let login_id = uuid_v4();
        let request = CreateCustomerRequest {
            customer_name: "doo@inc.com".to_string(),
            email: format!("doo_{}@inc.com", login_id),
            admin_password: "dokatece3.XXX".to_string()
        };
        let wr_reply = admin_server.create_customer(&request, dev_token);

        match wr_reply {
            Ok(reply) => {
                let te = TestEnv {
                    token: dev_token.to_string(),
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


pub fn close_test(test_name : &str, test_to_run: &[&str]) {

    use commons_error::*;
    use log::*;

    let mut test_list = TEST_LIST.lock().unwrap();
    if test_list.contains_key(&test_name.to_owned()) {
        // test_list.remove(&test_name.to_owned());
        test_list.insert(test_name.to_string(), TestStatus::DONE);
        eprintln!();
        eprintln!("⚪️ ****** Unregister the test : {} (Test left [{}])", test_name, test_list.len());
        eprintln!();
    }

    if is_all_terminated(test_list, test_to_run) {
        eprintln!();
        eprintln!("🚀 ****** All is terminated - Start the close tests process");
        eprintln!();
        let test_env = TEST_ENV.lock().unwrap();

        // Drop the new schema
        let admin_server = AdminServerClient::new("localhost", 30060);
        let reply = admin_server.delete_customer(&test_env.customer_code, &test_env.token);

        if let Err(_e) = reply {
            log_error!("Error while deleting the schema, schema=[{}]", &test_env.customer_code );
            // dbg!(&e);
        }

        eprintln!("Deleted customer, [{}]", &test_env.customer_code);

        // eprintln!();
        eprintln!("🏁 ****** End the close tests process");
        eprintln!();
    }

}

fn is_all_terminated(list : MutexGuard<HashMap<String, TestStatus>>, test_to_run: &[&str]) -> bool {
    // Vérifier si tous les éléments de la map sont à "DONE"
    if list.values().all(|status|
        match status {
            TestStatus::DONE => true,
            _ => false,
    })
    {
        // Vérifier si tous les éléments de test_to_run sont dans la map
        let all_tests_present = test_to_run.iter().all(|test| list.contains_key(*test));
        all_tests_present
    } else {
        false
    }
}

