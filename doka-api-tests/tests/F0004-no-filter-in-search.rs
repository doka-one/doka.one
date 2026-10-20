mod test_lib;

const TEST_TO_RUN: &[&str] = &[
    "t10_f0004_no_filter_none",
    "t20_f0004_no_filter_empty_string",
    "t30_f0004_no_filter_whitespace",
    "t40_f0004_no_filter_explicit_parens",
    "t50_f0004_no_filter_two_items",
    "t60_f0004_no_filter_is_superset_of_filtered",
    "t70_f0004_malformed_still_400",
];

#[cfg(test)]
mod f0004_no_filter_in_search_tests {
    use std::collections::HashSet;

    use rand::Rng;

    use dkdto::api_error::ApiError;
    use dkdto::web_types::{AddItemRequest, AddTagValue, EnumTagValue};
    use doka_cli::request_client::{AdminServerClient, DocumentServerClient};

    use crate::TEST_TO_RUN;
    use crate::test_lib::{Lookup, get_login_request};

    // ------------------------------------------------------------------
    // Helpers — kept self-contained, mirroring the F0003 test file.
    // ------------------------------------------------------------------

    /// Random tag name (avoids collisions on the shared customer schema).
    fn generate_random_tag() -> String {
        let mut rng = rand::thread_rng();
        let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
        let random_string: String = (0..5).map(|_| chars[rng.gen_range(0..chars.len())]).collect();
        format!("tag_{}", random_string)
    }

    fn text_prop(tag_name: &str, value: &str) -> AddTagValue {
        AddTagValue {
            tag_id: None,
            tag_name: Some(tag_name.to_string()),
            value: EnumTagValue::Text(Some(value.to_string())),
        }
    }

    fn create_item_with_props(
        document_server: &DocumentServerClient,
        session_id: &str,
        item_name: &str,
        properties: Vec<AddTagValue>,
    ) -> Result<dkdto::web_types::AddItemReply, ApiError<'static>> {
        let request = AddItemRequest {
            name: item_name.to_string(),
            file_ref: None,
            properties: Some(properties),
        };
        document_server.create_item(&request, session_id)
    }

    /// Boilerplate identical to every TC: log in, build a document-server
    /// client, return both the session id and the client.
    fn login(test_name: &str) -> Result<(Lookup<'static>, DocumentServerClient, String), ApiError<'static>> {
        let lookup = Lookup::new(test_name, TEST_TO_RUN);
        let props = lookup.props();
        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);
        Ok((lookup, document_server, login_reply.session_id))
    }

    // ------------------------------------------------------------------
    // TC-F0004-001 — `None` filter returns the created item.
    //
    // Given: one item A with a random text tag value "match-all-001".
    // When : search_item(None, &session_id).
    // Then : Ok(_) and item A appears — proves the delegate no longer
    //        falls back to analyse_expression("()") and that the
    //        sentinel-AST path produces a valid match-all SQL query.
    // ------------------------------------------------------------------
    #[test]
    fn t10_f0004_no_filter_none() -> Result<(), ApiError<'static>> {
        let (lookup, document_server, session_id) = login("t10_f0004_no_filter_none")?;

        let tag_text = generate_random_tag();
        let item_a = create_item_with_props(
            &document_server,
            &session_id,
            "TC-001 A (match-all-001)",
            vec![text_prop(&tag_text, "match-all-001")],
        )?;

        let reply = document_server.search_item(None, &session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A to appear when filter is None — proves the delegate routes None to a match-all path"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0004-002 — `Some("")` is equivalent to `None`.
    //
    // Given: one item A with value "match-all-002".
    // When : search_item(Some(""), &session_id).
    // Then : Ok(_) and item A appears — proves `?filters=` is
    //        normalised to no-filter by `normalize_filter_input`
    //        (F0004 rule 1).
    // ------------------------------------------------------------------
    #[test]
    fn t20_f0004_no_filter_empty_string() -> Result<(), ApiError<'static>> {
        let (lookup, document_server, session_id) = login("t20_f0004_no_filter_empty_string")?;

        let tag_text = generate_random_tag();
        let item_a = create_item_with_props(
            &document_server,
            &session_id,
            "TC-002 A (match-all-002)",
            vec![text_prop(&tag_text, "match-all-002")],
        )?;

        let reply = document_server.search_item(Some(""), &session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A to appear when filter is empty string — proves `?filters=` is normalised to no-filter"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0004-003 — Whitespace-only filter is equivalent to `None`.
    //
    // Given: one item A with value "match-all-003".
    // When : search_item(Some("   "), &session_id).
    // Then : Ok(_) and item A appears — proves the trim step in
    //        `normalize_filter_input` runs before the empty check
    //        (F0004 rule 1).
    // ------------------------------------------------------------------
    #[test]
    fn t30_f0004_no_filter_whitespace() -> Result<(), ApiError<'static>> {
        let (lookup, document_server, session_id) = login("t30_f0004_no_filter_whitespace")?;

        let tag_text = generate_random_tag();
        let item_a = create_item_with_props(
            &document_server,
            &session_id,
            "TC-003 A (match-all-003)",
            vec![text_prop(&tag_text, "match-all-003")],
        )?;

        let reply = document_server.search_item(Some("   "), &session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A to appear when filter is whitespace-only — proves trim happens before the empty check"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0004-004 — Explicit `()` filter is equivalent to `None`.
    //
    // Given: one item A with value "match-all-004".
    // When : search_item(Some("()"), &session_id).
    // Then : Ok(_) and item A appears — proves `()` is short-circuited
    //        by the delegate and never reaches the lexer (which would
    //        otherwise reject it with EmptyLogicalOperation).
    // ------------------------------------------------------------------
    #[test]
    fn t40_f0004_no_filter_explicit_parens() -> Result<(), ApiError<'static>> {
        let (lookup, document_server, session_id) = login("t40_f0004_no_filter_explicit_parens")?;

        let tag_text = generate_random_tag();
        let item_a = create_item_with_props(
            &document_server,
            &session_id,
            "TC-004 A (match-all-004)",
            vec![text_prop(&tag_text, "match-all-004")],
        )?;

        let reply = document_server.search_item(Some("()"), &session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A to appear when filter is `()` — proves the delegate short-circuits before the lexer"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0004-005 — Two items, `None` filter: both appear.
    //
    // Given: items A ("match-all-005-A") and B ("match-all-005-B") on a
    //        random text tag.
    // When : search_item(None, &session_id).
    // Then : Ok(_) and both A and B appear — guards against "match
    //        first" or `WHERE FALSE` regressions.
    // ------------------------------------------------------------------
    #[test]
    fn t50_f0004_no_filter_two_items() -> Result<(), ApiError<'static>> {
        let (lookup, document_server, session_id) = login("t50_f0004_no_filter_two_items")?;

        let tag_text = generate_random_tag();
        let item_a = create_item_with_props(
            &document_server,
            &session_id,
            "TC-005 A (match-all-005-A)",
            vec![text_prop(&tag_text, "match-all-005-A")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &session_id,
            "TC-005 B (match-all-005-B)",
            vec![text_prop(&tag_text, "match-all-005-B")],
        )?;

        let reply = document_server.search_item(None, &session_id)?;

        let ids: HashSet<_> = reply.items.iter().map(|it| it.item_id).collect();
        assert!(
            ids.contains(&item_a.item_id),
            "expected item A to appear — match-all must include every item, not just the first one"
        );
        assert!(
            ids.contains(&item_b.item_id),
            "expected item B to appear — match-all must include every item, not just the first one"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0004-006 — `None` reply is a superset of a matching filter reply.
    //
    // Given: one item A on a random text tag with the unique value
    //        "match-all-006-uniqueval".
    // When : (1) search_item(None, sid) -> reply_all
    //        (2) search_item(Some("(<tag> == \"match-all-006-uniqueval\")"), sid) -> reply_filtered
    // Then : both Ok(_); reply_filtered contains A; every item in
    //        reply_filtered is also in reply_all. Guards against the
    //        no-filter path drifting away from the filtered path
    //        (wrong schema, missing rows, …).
    // ------------------------------------------------------------------
    #[test]
    fn t60_f0004_no_filter_is_superset_of_filtered() -> Result<(), ApiError<'static>> {
        let (lookup, document_server, session_id) = login("t60_f0004_no_filter_is_superset_of_filtered")?;

        let tag_text = generate_random_tag();
        let unique_value = "match-all-006-uniqueval";
        let item_a = create_item_with_props(
            &document_server,
            &session_id,
            "TC-006 A (uniqueval)",
            vec![text_prop(&tag_text, unique_value)],
        )?;

        let reply_all = document_server.search_item(None, &session_id)?;
        let filter = format!(r#"({0} == "{1}")"#, &tag_text, unique_value);
        let reply_filtered = document_server.search_item(Some(&filter), &session_id)?;

        assert!(
            reply_filtered.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected the filtered reply to contain item A under filter [{}]",
            &filter
        );

        let all_ids: HashSet<_> = reply_all.items.iter().map(|it| it.item_id).collect();
        for filtered in &reply_filtered.items {
            assert!(
                all_ids.contains(&filtered.item_id),
                "item_id={} is in the filtered reply but missing from the no-filter reply — no-filter must be a superset",
                filtered.item_id
            );
        }

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0004-007 — A malformed non-empty filter still returns an error.
    //
    // Given: no seeding.
    // When : search_item(Some("(unbalanced"), &session_id).
    // Then : Err(_) — proves `normalize_filter_input` is conservative
    //        and does not silently re-route malformed input to the
    //        match-all path. Only the four declared no-filter forms are
    //        accepted; everything else still flows through
    //        `analyse_expression` and surfaces the existing 400
    //        contract (F0004 errors section).
    // ------------------------------------------------------------------
    #[test]
    fn t70_f0004_malformed_still_400() -> Result<(), ApiError<'static>> {
        let (lookup, document_server, session_id) = login("t70_f0004_malformed_still_400")?;

        let reply = document_server.search_item(Some("(unbalanced"), &session_id);

        assert!(
            reply.is_err(),
            "expected an error for a malformed filter — F0004 must not swallow bad input"
        );

        lookup.close();
        Ok(())
    }
}
