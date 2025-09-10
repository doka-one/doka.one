use std::collections::HashMap;
use std::time::Duration;

use dkconfig::conf_reader::{read_config, read_doka_env};
use dkdto::api_error::ApiError;
use dkdto::web_types::{LoginReply, LoginRequest, UploadReply};
use doka_api_tests::{read_test_env, Lookup, TestEnv};
use doka_cli::request_client::{AdminServerClient, FileServerClient};

const TEST_TO_RUN: &[&str] = &["t10_upload_mass_file"];

/// Test the capacity for the file server to handle many uploads at the same time
/// In this case, it can happen that the network buffer gets filled and some client request are rejected.
/// In a distributed micro-services environment, this case is not supposed to happen.
/// Anyhow, the file server must process all the incoming requests.
fn t10_upload_mass_file() -> Result<(), ApiError<'static>> {
    let props = read_config("doka-test", &read_doka_env("DOKA_UT_ENV"), &Some("DOKA_CLUSTER_PROFILE".to_string()));
    let lookup = Lookup::new("t10_upload_mass_file", TEST_TO_RUN, &props.get("token").unwrap()); // auto dropping
    let test_env = read_test_env();
    eprintln!("test_env {:?}", &test_env);

    use std::thread;

    let num_threads = 1000;
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            eprintln!("Run {}", i);
            let local_props = props.clone();
            let local_test_env = test_env.clone();
            thread::sleep(Duration::from_millis(1000)); // prevent from network buffer overflow
            thread::spawn(move || {
                let upload_reply = send_a_files(&local_test_env, &local_props).unwrap();
                eprintln!("{:?}", upload_reply);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    lookup.close();
    Ok(())
}

fn send_a_files(test_env: &TestEnv, props: &HashMap<String, String>) -> Result<UploadReply, ApiError<'static>> {
    // Login
    let admin_server = AdminServerClient::new("localhost", 30060);
    let login_request = LoginRequest { login: test_env.login.to_owned(), password: test_env.password.to_owned() };
    let login_reply = match admin_server.login(&login_request) {
        Ok(login_reply) => {
            eprintln!("login_reply {:?}", &login_reply);
            login_reply
        }
        Err(e) => {
            eprintln!("Panic login error {:?}", e);
            panic!();
        }
    };

    // Upload the document
    let file_server = FileServerClient::new("localhost", 30080);
    let file_name = format!(r"{}\111-Bright_Snow.jpg", &props.get("file.path").unwrap());

    let file_content = std::fs::read(file_name).unwrap();
    let upload_reply = match file_server.upload("bright snow", &file_content, &login_reply.session_id) {
        Ok(upload_reply) => {
            eprintln!("Upload reply [{:?}]", &upload_reply);
            upload_reply
        }
        Err(e) => {
            eprintln!("Panic upload error : {} - {:?}", &login_reply.session_id, e);
            panic!();
        }
    };

    Ok(upload_reply)
}

fn _simple_login(
    i: i32,
    test_env: &TestEnv,
    _props: &HashMap<String, String>,
) -> Result<LoginReply, ApiError<'static>> {
    // Login
    let admin_server = AdminServerClient::new("localhost", 30060);
    let login_request = LoginRequest { login: test_env.login.to_owned(), password: test_env.password.to_owned() };
    let login_reply = admin_server.login(&login_request)?;
    eprintln!("{} login_reply {:?}", i, &login_reply);
    Ok(login_reply)
}

fn main() {
    let _ = t10_upload_mass_file();
}
