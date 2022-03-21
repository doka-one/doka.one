mod lib;

#[cfg(test)]
mod api_document_tests {
    use dkdto::{AddItemRequest, LoginRequest};
    use doka_cli::request_client::{AdminServerClient, DocumentServerClient};
    use crate::lib::test_lib::{init, read_test_env};
    use crate::lib::test_lib::close_test;
    use log::info;

    #[test]
    fn t01_create_document() {
        init("t01_create_document");

        // Login
        let test_env = read_test_env();

        log_info!("{:?}", &test_env);

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: test_env.login,
            password: test_env.password,
        };
        let login_reply = admin_server.login(&login_request);

        // Create item
        let document_server = DocumentServerClient::new("localhost", 30070);

        let request = AddItemRequest {
            name: "A truck".to_string(),
            properties: None,
        };
        let item_reply = document_server.create_item(&request, &login_reply.session_id);
        dbg!(&item_reply);

        // Read the item
        let item_back = document_server.get_item(item_reply.item_id, &login_reply.session_id);

        assert_eq!(&request.name, &item_back.items.get(0).unwrap().name);

        close_test("t01_create_document");
    }

}