mod lib;

#[cfg(test)]
pub mod api_login_tests {

    use dkdto::{LoginRequest};
    use doka_cli::request_client::AdminServerClient;
    use crate::lib::test_lib::{init, read_test_env};
    use crate::lib::test_lib::close_test;

    #[test]
    fn t10_login_ok() {

        init("t10_login_ok");

        let test_env = read_test_env();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: test_env.login.clone(),
            password: test_env.password.clone(),
        };
        let login_reply = admin_server.login(&login_request);

        assert_eq!(0, login_reply.status.error_code);
        assert_eq!(false, login_reply.session_id.is_empty());

        close_test("t10_login_ok");

    }

    #[test]
    fn t20_login_fail() {

        init("t20_login_fail");

        let test_env = read_test_env();
        // Init : Create the schema (if not exist), create the admin user (if not exist)


        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: test_env.login.clone(),
            password: "dokatece3.WRONG".to_string()
        };
        let login_reply = admin_server.login(&login_request);

        dbg!(&login_reply);

        assert_ne!(0, login_reply.status.error_code);
        assert_eq!(true, login_reply.session_id.is_empty());

        close_test("t20_login_fail");

    }

    #[test]
    fn t30_login_fail() {

        init("t30_login_fail");

        let test_env = read_test_env();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: "inconnu@doka.com".to_string(),
            password: test_env.password.clone(),
        };
        let login_reply = admin_server.login(&login_request);
        assert_ne!(0, login_reply.status.error_code);
        assert_eq!(true, login_reply.session_id.is_empty());

        close_test("t30_login_fail");
    }

}


