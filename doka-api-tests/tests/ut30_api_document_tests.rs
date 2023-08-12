
mod test_lib;

const TEST_TO_RUN : &[&str] = &["t10_create_document", "t20_create_document_with_props"];

#[cfg(test)]
mod api_document_tests {
    use anyhow::anyhow;
    use dkdto::{AddItemRequest, AddTagValue, EnumTagValue, ErrorMessage, GetItemReply, LoginRequest, TagValueElement};
    use doka_cli::request_client::{AdminServerClient, DocumentServerClient};
    use crate::test_lib::{Lookup, read_test_env};
    use crate::TEST_TO_RUN;

    ///
    /// Create simple item
    ///
    #[test]
    fn t10_create_document() -> Result<(), ErrorMessage>  {
        let lookup = Lookup::new("t10_create_document", TEST_TO_RUN); // auto dropping

        // Login
        let test_env = read_test_env();

        eprintln!("{:?}", &test_env);

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: test_env.login,
            password: test_env.password,
        };
        let login_reply = admin_server.login(&login_request)?;

        let request = AddItemRequest {
            name: "A truck".to_string(),
            file_ref: None,
            properties: None,
        };

        let document_server = DocumentServerClient::new("localhost", 30070);
        let item_reply = document_server.create_item(&request, &login_reply.session_id)?;

        // Read the item
        let get_item_reply = document_server.get_item(item_reply.item_id, &login_reply.session_id)?;
        let item_name_back = get_item_reply.items.get(0).unwrap().name.clone();
        assert_eq!(&request.name, &item_name_back);
        lookup.close();
        Ok(())
    }


    ///
    /// Create item with props
    ///
    #[test]
    fn t20_create_document_with_props() -> Result<(), ErrorMessage>  {
        let lookup = Lookup::new("t20_create_document_with_props", TEST_TO_RUN); // auto dropping

        // Login
        let test_env = read_test_env();

        eprintln!("{:?}", &test_env);

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = LoginRequest {
            login: test_env.login,
            password: test_env.password,
        };
        let login_reply = admin_server.login(&login_request)?;

        let p1 = AddTagValue {
            tag_id: None,
            tag_name: Some("prop1".to_owned()),
            value: EnumTagValue::Double(Option::from(1.234)),
        };

        let p2 = AddTagValue {
            tag_id: None,
            tag_name: Some("prop2".to_owned()),
            value: EnumTagValue::String(Option::from("My prop2 value".to_owned())),
        };

        let request = AddItemRequest {
            name: "A truck".to_string(),
            file_ref: None,
            properties: Some(vec![p1,p2]),
        };

        let document_server = DocumentServerClient::new("localhost", 30070);
        let item_reply = document_server.create_item(&request, &login_reply.session_id)?;

        // Read the item
        let get_item_reply = document_server.get_item(item_reply.item_id, &login_reply.session_id)?;
        let item_name_back = get_item_reply.items.get(0).unwrap().name.clone();

        let prop_value_1 = read_property(&get_item_reply, 0)?;
        let prop_value_2 = read_property(&get_item_reply, 1)?;

        assert_eq!(&request.name, &item_name_back);
        assert_eq!("1.234", &prop_value_1);
        assert_eq!("My prop2 value", &prop_value_2);
        lookup.close();
        Ok(())
    }

    fn read_property(get_item_reply: &GetItemReply, prop_order: usize) -> anyhow::Result<String> {
        let item = get_item_reply.items.get(0).ok_or(anyhow!("No item found"))?;
        Ok(item.properties.as_ref().ok_or(anyhow!("No properties"))?.get(prop_order).ok_or(anyhow!("No prop 0"))?.value.to_string())
    }
}
