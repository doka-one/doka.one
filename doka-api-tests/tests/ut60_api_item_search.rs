mod test_lib;

const TEST_TO_RUN: &[&str] = &[
    "t10_search_valid",
    "t20_search_invalid_operator",
    "t30_search_unbalanced_parens",
    "t40_search_no_filter",
];

#[cfg(test)]
mod api_item_search_tests {
    use rand::Rng;

    use dkdto::api_error::ApiError;
    use dkdto::web_types::{AddItemRequest, AddTagValue, EnumTagValue};
    use doka_cli::request_client::{AdminServerClient, DocumentServerClient};

    use crate::test_lib::{get_login_request, Lookup};
    use crate::TEST_TO_RUN;

    /// Happy path: create an item with a tag, then search by `(tag == "value")`
    /// and confirm the item is returned.
    #[test]
    fn t10_search_valid() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t10_search_valid", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;

        let tag_name = generate_random_tag();
        let tag_value = "test".to_string();

        let p = AddTagValue {
            tag_id: None,
            tag_name: Some(tag_name.clone()),
            value: EnumTagValue::Text(Some(tag_value.clone())),
        };
        let request =
            AddItemRequest { name: "Search target".to_string(), file_ref: None, properties: Some(vec![p]) };

        let document_server = DocumentServerClient::new("localhost", 30070);
        let item_reply = document_server.create_item(&request, &login_reply.session_id)?;

        let filter = format!(r#"({} == "{}")"#, tag_name, tag_value);
        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_reply.item_id),
            "expected to find the created item id={} for filter [{}]",
            item_reply.item_id,
            filter
        );

        lookup.close();
        Ok(())
    }

    /// Broken syntax: single `=` is not a valid operator (the DSL requires `==`).
    #[test]
    fn t20_search_invalid_operator() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t20_search_invalid_operator", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let filter = r#"(category = "test")"#;
        let reply = document_server.search_item(Some(filter), &login_reply.session_id);

        assert!(reply.is_err(), "expected an error for filter with a single '='");

        lookup.close();
        Ok(())
    }

    /// Broken syntax: an unbalanced opening parenthesis must be rejected.
    #[test]
    fn t30_search_unbalanced_parens() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t30_search_unbalanced_parens", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let filter = r#"(category == "test""#;
        let reply = document_server.search_item(Some(filter), &login_reply.session_id);

        assert!(reply.is_err(), "expected an error for filter with unbalanced parentheses");

        lookup.close();
        Ok(())
    }

    /// Search without a filter must return at least every item we just created.
    #[test]
    fn t40_search_no_filter() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t40_search_no_filter", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;

        let request =
            AddItemRequest { name: "Unfiltered target".to_string(), file_ref: None, properties: None };

        let document_server = DocumentServerClient::new("localhost", 30070);
        let item_reply = document_server.create_item(&request, &login_reply.session_id)?;

        let reply = document_server.search_item(None, &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_reply.item_id),
            "expected the created item id={} to appear when no filter is set",
            item_reply.item_id
        );

        lookup.close();
        Ok(())
    }

    fn generate_random_tag() -> String {
        let mut rng = rand::thread_rng();
        let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
        let random_string: String = (0..5).map(|_| chars[rng.gen_range(0..chars.len())]).collect();
        format!("tag_{}", random_string)
    }
}
