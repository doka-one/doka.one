
mod test_lib;

const TEST_TO_RUN : &[&str] = &["t10_login_ok", "t20_login_fail", "t30_login_fail"];


/// cargo test  --package doka-api-tests --test ut10_api_login_tests --  --nocapture --test-threads=1

#[cfg(test)]
pub mod api_login_tests {

    use dkdto::{ErrorMessage, LoginRequest};
    use doka_cli::request_client::AdminServerClient;
    use crate::test_lib::{get_login_request, Lookup};
    use crate::TEST_TO_RUN;

    #[test]
    fn t10_login_ok() -> Result<(), ErrorMessage> {
        let lookup = Lookup::new("t10_login_ok", TEST_TO_RUN); // auto dropping
        let props = lookup.props();
        eprintln!("props {:?}", &props);
        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = get_login_request(&props);
        let login_reply = admin_server.login(&login_request)?;
        eprintln!("Login reply={:?}", &login_reply);
        assert_eq!(false, login_reply.customer_code.is_empty());
        assert_eq!(false, login_reply.session_id.is_empty());
        lookup.close();
        Ok(())
    }

    #[test]
    fn t20_login_fail() -> Result<(), ErrorMessage> {
        let lookup = Lookup::new("t20_login_fail", TEST_TO_RUN); // auto dropping
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: props.get("login").unwrap().to_owned(),
            password: "dokatece3.WRONG".to_string()
        };
        let login_reply = admin_server.login(&login_request);

        assert_eq!(true, login_reply.is_err());

        let http_code = login_reply.err().unwrap().http_error_code;
        eprintln!("{}", http_code);
        assert_eq!(403, http_code);
        lookup.close();
        Ok(())
    }

    #[test]
    fn t30_login_fail() -> Result<(), ErrorMessage> {
        let lookup = Lookup::new("t30_login_fail", TEST_TO_RUN); // auto dropping
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: "inconnu@doka.com".to_string(),
            password: props.get("password").unwrap().to_owned(),
        };
        let login_reply = admin_server.login(&login_request);

        assert_eq!(true, login_reply.is_err());
        let http_code = login_reply.err().unwrap().http_error_code;
        assert_eq!(403, http_code);
        lookup.close();
        Ok(())
    }

}


