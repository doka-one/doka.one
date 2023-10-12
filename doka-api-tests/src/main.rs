use std::collections::HashMap;
use std::panic::panic_any;
use std::time::Duration;
use log::*;
use commons_error::*;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkdto::{ErrorMessage, LoginReply, LoginRequest, UploadReply, WebResponse};
use doka_api_tests::{Lookup, read_test_env, TestEnv};
use doka_cli::request_client::{AdminServerClient, FileServerClient};

const TEST_TO_RUN : &[&str] = &["t10_upload_mass_file"];

fn t10_upload_mass_file() -> Result<(), ErrorMessage> {
    let lookup = Lookup::new("t10_upload_mass_file", TEST_TO_RUN); // auto dropping
    let test_env = read_test_env();
    let props = read_config("doka-test", &read_doka_env("DOKA_UT_ENV"));
    eprintln!("test_env {:?}", &test_env);

    const NB_PARTS : u32 = 9;
    use std::thread;

    let num_threads = 1000; // Changer le nombre de threads selon vos besoins
    let handles: Vec<_> = (0..num_threads).map(|i| {
        eprintln!("Run {}", i);
        let local_props = props.clone();
        let local_test_env = test_env.clone();
        let duration = Duration::from_millis(100);
        thread::sleep(duration);
        thread::spawn(move || {
            let upload_reply = send_a_files(&local_test_env, &local_props).unwrap();
            eprintln!("{:?}", upload_reply);
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }

//    let duration = Duration::from_secs(20*60);
//    thread::sleep(duration);

    /*        let props = read_config("doka-test", &read_doka_env("DOKA_UT_ENV"));
            let upload_reply = send_a_files(&test_env, &props).unwrap();
            assert_eq!(NB_PARTS, upload_reply.block_count);

            let upload_reply2 = send_a_files(&test_env, &props).unwrap();
            assert_eq!(NB_PARTS, upload_reply2.block_count);
    */


    // wait_until_file_processing_complete(&file_server, &upload_reply.file_ref, &login_reply.session_id,upload_reply.block_count);
    //
    // // Get the information of the file
    // let info_reply = file_server.info(&upload_reply.file_ref, &login_reply.session_id)?;
    //
    // eprintln!("Info reply [{:?}]", &info_reply);
    // assert_eq!("image/jpeg", info_reply.media_type.unwrap());

    lookup.close();
    Ok(())
}

fn send_a_files(test_env: &TestEnv, props: &HashMap<String, String>) -> Result<UploadReply, ErrorMessage> {

    // Login
    let admin_server = AdminServerClient::new("localhost", 30060);
    let login_request = LoginRequest {
        login: test_env.login.to_owned(),
        password: test_env.password.to_owned(),
    };
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
    let file_name = format!(r"{}\111-Bright_Snow.jpg", &props.get("file.path").unwrap() );

    let file_content = std::fs::read(file_name).unwrap();
    let upload_reply = match file_server.upload( "bright snow", &file_content,  &login_reply.session_id) {
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


fn simple_login(i: i32, test_env: &TestEnv, props: &HashMap<String, String>) -> Result<LoginReply, ErrorMessage> {
    // Login
    let admin_server = AdminServerClient::new("localhost", 30060);
    let login_request = LoginRequest {
        login: test_env.login.to_owned(),
        password: test_env.password.to_owned(),
    };
    let login_reply = admin_server.login(&login_request)?;
    eprintln!("{} login_reply {:?}", i, &login_reply);
    Ok(login_reply)
}


fn main() {
    let _ = t10_upload_mass_file();
}
