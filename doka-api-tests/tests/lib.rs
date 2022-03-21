
pub mod test_lib {
    use std::collections::HashMap;
    use dkdto::{CreateCustomerRequest};
    use doka_cli::request_client::AdminServerClient;
    use lazy_static::*;
    use std::sync::{Mutex};
    use rs_uuid::iso::uuid_v4;
    use log::error;

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
        static ref TEST_LIST: Mutex<HashMap<String, bool>> = Mutex::new(HashMap::new());
    }

    lazy_static! {
        static ref IS_INIT_MUT : Mutex<bool> = Mutex::new(
            {
                false
            });
    }


    pub fn init(test_name : &str) {
        {
            let mut test_list = TEST_LIST.lock().unwrap();
            test_list.insert(test_name.to_string(), true); // means the test has started

            eprintln!();
            eprintln!("🔨 ****** Register the test : {} (Test present [{}])", test_name, test_list.len() );
            eprintln!();
        }

        let mut is_init = IS_INIT_MUT.lock().unwrap();
        dbg!(&is_init);

        if *is_init == false {

            eprintln!();
            eprintln!("🚀 ****** Start the init tests process");
            eprintln!();

            // Init : Create the schema (if not exist), create the admin user (if not exist)
            let admin_server = AdminServerClient::new("localhost", 30060);

            // This value should depend on the environment we want to run the test.
            // Please refer to the CEK documents to clarify the call of "protected" routines on various environments
            let dev_token = "j6nk2GaKdfLl3nTPbfWW0C_Tj-MFLrJVS2zdxiIKMZpxNOQGnMwFgiE4C9_cSScqshQvWrZDiPyAVYYwB8zCLRBzd3UUXpwLpK-LMnpqVIs";

            let login_id = uuid_v4();
            let request = CreateCustomerRequest {
                customer_name: "doo@inc.com".to_string(),
                email: format!("doo_{}@inc.com", login_id),
                admin_password: "dokatece3.XXX".to_string()
            };
            let reply = admin_server.create_customer(&request, dev_token);

            if reply.status.error_code != 0 {
                panic!("Error code [{:?}]", &reply.status);
            }

            let te = TestEnv {
                token: dev_token.to_string(),
                customer_code: reply.customer_code.clone(),
                login: request.email.clone(),
                password: request.admin_password.clone(),
            };

            let reply = admin_server.customer_removable(&reply.customer_code, dev_token);

            if reply.error_code != 0 {
                panic!("Error code [{:?}]", &reply);
            }

            let mut test_env = TEST_ENV.lock().unwrap();
            *test_env = te;

            eprintln!();
            eprintln!("🏁 ****** End the init tests process");
            eprintln!();

        }
        *is_init = true;

    }


    pub fn close_test(test_name : &str) {

        let mut test_list = TEST_LIST.lock().unwrap();
        if test_list.contains_key(&test_name.to_owned()) {
            test_list.remove(&test_name.to_owned());
            eprintln!();
            eprintln!("⚪️ ****** Unregister the test : {} (Test left [{}])", test_name, test_list.len());
            eprintln!();
        }

        if test_list.is_empty() {
            eprintln!();
            eprintln!("🚀 ****** Start the close tests process");
            eprintln!();
            let test_env = TEST_ENV.lock().unwrap();
            dbg!(&test_env);

            // Drop the new schema
            let admin_server = AdminServerClient::new("localhost", 30060);
            let reply = admin_server.delete_customer(&test_env.customer_code, &test_env.token);

            if reply.error_code != 0 {
                log_error!("Error while deleting the schema, schema=[{}]", &test_env.customer_code );
                dbg!(&reply);
            }
            eprintln!();
            eprintln!("🏁 ****** End the close tests process");
            eprintln!();
        }

    }


}
