mod lib;

#[cfg(test)]
mod api_fileserver_tests {
    use dkdto::{AddItemRequest, LoginRequest};
    use doka_cli::request_client::{AdminServerClient, DocumentServerClient, FileServerClient};
    use crate::lib::test_lib::{init, read_test_env};
    use crate::lib::test_lib::close_test;
    use log::info;

    #[test]
    fn t01_upload_file() {
        log_info!("Start the tests");


        // TODO
        //  1 - Check the direct upload
        //  2 - Fix the init routine that creates a complete customer entity ( !!! cs_ and fs_ )
        //  3 - Run the full API tests

        // init("t01_upload_file");

        let test_env = read_test_env();

        eprintln!("test_env {:?}", &test_env);

        // Login
        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: test_env.login,
            password: test_env.password,
        };
        let login_reply = admin_server.login(&login_request);

        eprintln!("login_reply {:?}", &login_reply);

        // dbg!(&login_reply);

        // Upload the document
        // let file_server = FileServerClient::new("localhost", 30080);
        //
        // let file_content = std::fs::read("C:/Users/denis/wks-poc/tika/big_planet.pdf").unwrap();
        // let reply = file_server.upload( &file_content,  &login_reply.session_id);

        // Get the information of the file

        //assert_eq!(0, reply.status.error_code);

        close_test("t01_upload_file");
    }

}