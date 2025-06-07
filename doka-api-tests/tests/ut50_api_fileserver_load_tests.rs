mod test_lib;

const TEST_TO_RUN: &[&str] = &["t10_upload_mass_file"];

#[cfg(test)]
mod api_fileserver_load_tests {
    use base64::Engine;
    use core::time::Duration;
    use dkdto::{ErrorMessage, UploadReply};
    use doka_cli::request_client::{AdminServerClient, FileServerClient};
    use std::collections::HashMap;
    use std::thread;

    use crate::test_lib::{get_login_request, Lookup};
    use crate::TEST_TO_RUN;

    const NB_PARTS: u32 = 37;

    #[test]
    fn t10_upload_mass_file() -> Result<(), ErrorMessage> {
        let lookup = Lookup::new("t10_upload_mass_file", TEST_TO_RUN); // auto dropping
        let props = lookup.props();

        let file_path = props.get("file.path").unwrap();

        // Login
        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = get_login_request(&props);
        let login_reply = admin_server.login(&login_request)?;
        eprintln!("login_reply {:?}", &login_reply);

        let num_threads = 20; // Changer le nombre de threads selon vos besoins

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let local_file_path = file_path.clone();
                let local_session_id = login_reply.session_id.clone();
                thread::spawn(move || send_a_files(&local_file_path, &local_session_id).unwrap())
            })
            .collect();

        let mut new_file_refs = vec![];
        for handle in handles {
            let upload_reply = handle.join().unwrap();
            assert_eq!(NB_PARTS, upload_reply.block_count);
            eprintln!("Upload reply [{:?}]", &upload_reply);
            new_file_refs.push(upload_reply.file_ref);
        }

        let file_server = FileServerClient::new("localhost", 30080);

        // let duration = Duration::from_secs(20 * 60);
        // thread::sleep(duration);
        // Use the routine below to know when the processing is finished
        wait_until_loading_complete(&file_server, &login_reply.session_id);

        for file_ref in new_file_refs {
            // Get the information of the file
            let stats_reply = file_server.stats(&file_ref, &login_reply.session_id)?;
            eprintln!("Info reply [{:?}]", &stats_reply);
            assert_eq!(NB_PARTS as i64, stats_reply.encrypted_count);
        }

        lookup.close();
        Ok(())
    }

    fn send_a_files(file_path: &str, session_id: &str) -> Result<UploadReply, ErrorMessage> {
        const FILE_NAME: &str = "1111-38M.pdf";

        // Upload the document
        let file_server = FileServerClient::new("localhost", 30080);

        // encode in base64 url the file name
        let encoded_file_name = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(FILE_NAME);

        let file_name = format!(r"{}/{}", &file_path, FILE_NAME);
        let file_content = std::fs::read(file_name).unwrap();
        let upload_reply = file_server.upload(&encoded_file_name, &file_content, &session_id)?;
        Ok(upload_reply)
    }

    fn wait_until_loading_complete(file_server: &FileServerClient, session_id: &str) {
        let mut count = 0;
        let duration = Duration::from_millis(2000);
        loop {
            eprintln!("Loading count [{}]", count);
            match file_server.loading(&session_id) {
                Ok(loading_reply) => {
                    println!("Loading reply [{:?}]", &loading_reply);
                    if loading_reply.list_of_upload_info.is_empty() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Loading reply error [{:?}]", &e);
                }
            }
            thread::sleep(duration);
            if count > 50 {
                break;
            }
            count += 1;
        }
    }
}
