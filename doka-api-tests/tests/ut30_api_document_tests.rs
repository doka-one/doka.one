
mod test_lib;

const TEST_TO_RUN : &[&str] = &["t10_create_document", "t20_create_document_with_props", "t30_add_props", "t40_modify_tags"];

#[cfg(test)]
mod api_document_tests {
    use anyhow::anyhow;
    use rand::Rng;

    use dkdto::{AddItemRequest, AddItemTagRequest, AddTagValue, EnumTagValue, ErrorMessage, GetItemReply};
    use doka_cli::request_client::{AdminServerClient, DocumentServerClient};

    use crate::test_lib::{get_login_request, Lookup};
    use crate::TEST_TO_RUN;

    ///
    /// Create simple item
    ///
    #[test]
    fn t10_create_document() -> Result<(), ErrorMessage>  {
        let lookup = Lookup::new("t10_create_document", TEST_TO_RUN); // auto dropping
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = get_login_request(&props);
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
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = get_login_request(&props);
        let login_reply = admin_server.login(&login_request)?;

        let prop1 = generate_random_tag(); // Unique tag name
        let prop2 = generate_random_tag();

        let p1 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop1.to_owned()),
            value: EnumTagValue::Double(Option::from(1.234)),
        };

        let p2 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop2.to_owned()),
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


    ///
    /// Create item and add tags
    ///
    #[test]
    fn t30_add_props() -> Result<(), ErrorMessage>  {
        let lookup = Lookup::new("t30_add_props", TEST_TO_RUN); // auto dropping
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = get_login_request(&props);
        let login_reply = admin_server.login(&login_request)?;

        // Create an item

        let request = AddItemRequest {
            name: "A truck".to_string(),
            file_ref: None,
            properties: None,
        };

        let document_server = DocumentServerClient::new("localhost", 30070);
        let item_reply = document_server.create_item(&request, &login_reply.session_id)?;

        // Add properties

        let prop1 = generate_random_tag(); // Unique tag name
        let prop2 = generate_random_tag();

        let p1 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop1.to_owned()),
            value: EnumTagValue::Double(Option::from(1.234)),
        };

        let p2 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop2.to_owned()),
            value: EnumTagValue::String(Option::from("My prop2 value".to_owned())),
        };

        let add_item_tag_request = AddItemTagRequest {
            properties: vec![p1,p2],
        };

        let add_item_tag_reply = document_server.update_item_tag(item_reply.item_id,
                                                                 &add_item_tag_request,
                                                                 &login_reply.session_id)?;

        assert_eq!(false, add_item_tag_reply.status.is_empty());

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


    ///
    /// Create item and modify tags
    ///
    #[test]
    fn t40_modify_tags() -> Result<(), ErrorMessage>  {
        let lookup = Lookup::new("t40_modify_tags", TEST_TO_RUN); // auto dropping
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_request = get_login_request(&props);
        let login_reply = admin_server.login(&login_request)?;

        // Create an item with tags

        let prop1 = generate_random_tag(); // Unique tag name
        let prop2 = generate_random_tag();

        let p1 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop1.to_owned()),
            value: EnumTagValue::Double(Option::from(1.234)),
        };

        let p2 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop2.to_owned()),
            value: EnumTagValue::String(Option::from("My prop2 value".to_owned())),
        };

        let request = AddItemRequest {
            name: "A truck 2".to_string(),
            file_ref: None,
            properties: Some(vec![p1,p2]),
        };

        let document_server = DocumentServerClient::new("localhost", 30070);
        let item_reply = document_server.create_item(&request, &login_reply.session_id)?;

        // Change tags

        let p1 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop1.to_owned()),
            value: EnumTagValue::Double(Option::from(63.0001)),
        };

        let p2 = AddTagValue {
            tag_id: None,
            tag_name: Some(prop2.to_owned()),
            value: EnumTagValue::String(Option::from("Une histoire de tag".to_owned())),
        };

        let add_item_tag_request = AddItemTagRequest {
            properties: vec![p1,p2],
        };

        let add_item_tag_reply = document_server.update_item_tag(item_reply.item_id,
                                                                 &add_item_tag_request, &login_reply.session_id)?;

        assert_eq!(false, add_item_tag_reply.status.is_empty());

        // Read the item
        let get_item_reply = document_server.get_item(item_reply.item_id, &login_reply.session_id)?;
        let item_name_back = get_item_reply.items.get(0).unwrap().name.clone();

        let prop_value_1 = read_property(&get_item_reply, 0)?;
        let prop_value_2 = read_property(&get_item_reply, 1)?;

        assert_eq!(&request.name, &item_name_back);
        assert_eq!("63.0001", &prop_value_1);
        assert_eq!("Une histoire de tag", &prop_value_2);
        lookup.close();
        Ok(())
    }

    fn read_property(get_item_reply: &GetItemReply, prop_order: usize) -> anyhow::Result<String> {
        let item = get_item_reply.items.get(0).ok_or(anyhow!("No item found"))?;
        Ok(item.properties.as_ref().ok_or(anyhow!("No properties"))?.get(prop_order).ok_or(anyhow!("No prop 0"))?.value.to_string())
    }

    fn generate_random_tag() -> String {
        let mut rng = rand::thread_rng();
        let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();

        //let n = rng.gen_range(0..1_000_000)

        let random_string: String = (0..5)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect();

        format!("tag_{}", random_string)
    }

}
