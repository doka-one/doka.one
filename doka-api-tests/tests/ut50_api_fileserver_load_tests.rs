mod test_lib;

const TEST_TO_RUN : &[&str] = &["t10_upload_mass_file"];

#[cfg(test)]
mod api_fileserver_load_tests {
    use std::thread;
    use core::time::Duration;
    use std::collections::HashMap;
    use dkdto::{ErrorMessage, UploadReply};
    use doka_cli::request_client::{AdminServerClient, FileServerClient};
    use crate::test_lib::{get_login_request, Lookup};
    use crate::TEST_TO_RUN;

    const NB_PARTS : u32 = 9;

    #[ignore]
    #[test]
    fn t10_upload_mass_file() -> Result<(), ErrorMessage> {
        let lookup = Lookup::new("t10_upload_mass_file", TEST_TO_RUN); // auto dropping
        let props = lookup.props();

        use std::thread;

        let num_threads = 1000; // Changer le nombre de threads selon vos besoins

        let handles: Vec<_> = (0..num_threads).map(|_| {
            let local_props = props.clone();
            // let local_test_env = test_env.clone();
            thread::spawn(move || {
                let upload_reply = send_a_files(&local_props).unwrap();
                assert_eq!(NB_PARTS, upload_reply.block_count);
            })
        }).collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let duration = Duration::from_secs(20*60);
        thread::sleep(duration);
        // TODO use the routine below to know when the processing is finished
        // wait_until_file_processing_complete(&file_server, &upload_reply.file_ref, &login_reply.session_id,upload_reply.block_count);

        lookup.close();
        Ok(())
    }

    fn send_a_files(props: &HashMap<String, String>) -> Result<UploadReply, ErrorMessage> {

        // Login
        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = get_login_request(&props);
        let login_reply = admin_server.login(&login_request)?;
        eprintln!("login_reply {:?}", &login_reply);

        // Upload the document
        let file_server = FileServerClient::new("localhost", 30080);

        let file_name = format!(r"{}\111-Bright_Snow.jpg", &props.get("file.path").unwrap() );

        let file_content = std::fs::read(file_name).unwrap();
        let upload_reply = file_server.upload( "bright snow", &file_content,  &login_reply.session_id)?;
        eprintln!("Upload reply [{:?}]", &upload_reply);
        // assert_eq!(NB_PARTS, upload_reply.block_count);
        Ok(upload_reply)
    }

    fn wait_until_file_processing_complete(file_server: &FileServerClient, file_ref: &str, session_id: &str, block_count: u32) {
        let mut count = 0;
        let duration = Duration::from_millis(500);
        loop {
            eprintln!("Stats count [{}]", count);
            match file_server.stats(&file_ref, &session_id) {
                Ok(stats_reply) => {
                    eprintln!("Stats reply [{:?}]", &stats_reply);
                    if stats_reply.encrypted_count == block_count as i64 {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Stats reply error [{:?}]", &e);
                }
            }
            thread::sleep(duration);
            if count > 10 {
                break;
            }
            count += 1;
        }
    }

}
