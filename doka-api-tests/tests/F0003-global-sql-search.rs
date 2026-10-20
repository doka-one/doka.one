mod test_lib;

const TEST_TO_RUN: &[&str] = &[
    "t10_f0003_like_with_escapes_and_numeric",
    "t20_f0003_eq_text_with_apostrophe",
    "t30_f0003_eq_text_with_escaped_dquote",
    "t40_f0003_like_leading_wildcard",
    "t50_f0003_like_both_sides_wildcards_accents",
    "t60_f0003_like_only_literal_percent",
    "t70_f0003_neq_text_with_apostrophe",
    "t80_f0003_text_like_and_bool_eq",
    "t90_f0003_numeric_or_two_ranges",
    "t110_f0003_eq_text_with_hash_escape",
];

#[cfg(test)]
mod f0003_global_sql_search_tests {
    use rand::Rng;

    use dkdto::api_error::ApiError;
    use dkdto::web_types::{AddItemRequest, AddTagValue, EnumTagValue};
    use doka_cli::request_client::{AdminServerClient, DocumentServerClient};

    use crate::TEST_TO_RUN;
    use crate::test_lib::{Lookup, get_login_request};

    /// TC-F0003-001 — Composite expression: `LIKE` with escape rules AND a
    /// numeric branch. End-to-end black-box validation through the
    /// `document-server` HTTP API that F0001 / F0002 / F0003 escaping is
    /// honoured all the way down to the generated SQL.
    ///
    /// Seeded items:
    ///   A: name = "50 % de l'élite ranking", age = 25  -> must match
    ///   B: name = "50X de l'élite ranking" , age = 25  -> must be excluded
    ///   C: name = "50 % de l'élite ranking", age = 15  -> must be excluded
    ///
    /// Filter: ( <tag_text> LIKE "50 #% de l'élite%" ) AND ( <tag_int> >= 18 )
    #[test]
    fn t10_f0003_like_with_escapes_and_numeric() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t10_f0003_like_with_escapes_and_numeric", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        // Random tag names per run to avoid collisions on the shared customer.
        let tag_text = generate_random_tag();
        let tag_int = generate_random_tag();

        // Three items packing every escape axis into the text value:
        //   - a literal `%` (matched by the `#%` of the DFS filter)
            //   - a `'` inside `l'élite` (must be doubled in SQL by F0002 rule 1)
        //   - a trailing free sequence (caught by the trailing wildcard)
        let item_a = create_item(
            &document_server,
            &login_reply.session_id,
            "Item A (should match)",
            &tag_text,
            "50 % de l'élite ranking",
            &tag_int,
            25,
        )?;
        let item_b = create_item(
            &document_server,
            &login_reply.session_id,
            "Item B (no literal %)",
            &tag_text,
            "50X de l'élite ranking",
            &tag_int,
            25,
        )?;
        let item_c = create_item(
            &document_server,
            &login_reply.session_id,
            "Item C (age < 18)",
            &tag_text,
            "50 % de l'élite ranking",
            &tag_int,
            15,
        )?;

        let filter = format!(
            r#"({0} LIKE "50 #% de l'élite%") AND ({1} >= 18)"#,
            &tag_text, &tag_int
        );
        eprintln!("F0003 filter: {}", &filter);

        // Assertion 4 (implicit): the HTTP call must not error out.
        // If it does, the assembled SQL is malformed (typically because a
        // `'` was not doubled, or `ESCAPE '\'` ended up nested inside
        // `unaccent_lower(...)`).
        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        eprintln!("F0003 reply items: {}", reply.items.len());
        for it in &reply.items {
            eprintln!("  - item_id={}", it.item_id);
        }

        // Assertion 1: item A is returned.
        // Covers: `'` doubling + `#%` -> `\%` + bare `%` wildcard + ESCAPE
        // clause sitting outside `unaccent_lower(...)`.
        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (literal '%', age 25) to match filter [{}]",
            &filter
        );

        // Assertion 2: item B is excluded.
        // Covers: `#%` is enforced as literal `%`, not as a wildcard.
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (no literal '%') to be excluded — \\% must be enforced"
        );

        // Assertion 3: item C is excluded.
        // Covers: numeric branch honoured; no ESCAPE leak into the int sub-query.
        assert!(
            reply.items.iter().all(|it| it.item_id != item_c.item_id),
            "expected item C (age=15) to be excluded by the numeric branch"
        );

        lookup.close();
        Ok(())
    }

    /// Helper: create an item with one text tag and one int tag.
    fn create_item(
        document_server: &DocumentServerClient,
        session_id: &str,
        item_name: &str,
        tag_text: &str,
        tag_text_value: &str,
        tag_int: &str,
        tag_int_value: i64,
    ) -> Result<dkdto::web_types::AddItemReply, ApiError<'static>> {
        let text_property = AddTagValue {
            tag_id: None,
            tag_name: Some(tag_text.to_string()),
            value: EnumTagValue::Text(Some(tag_text_value.to_string())),
        };
        let int_property = AddTagValue {
            tag_id: None,
            tag_name: Some(tag_int.to_string()),
            value: EnumTagValue::Integer(Some(tag_int_value)),
        };
        let request = AddItemRequest {
            name: item_name.to_string(),
            file_ref: None,
            properties: Some(vec![text_property, int_property]),
        };
        document_server.create_item(&request, session_id)
    }

    fn generate_random_tag() -> String {
        let mut rng = rand::thread_rng();
        let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
        let random_string: String = (0..5).map(|_| chars[rng.gen_range(0..chars.len())]).collect();
        format!("tag_{}", random_string)
    }

    // ------------------------------------------------------------------
    // Generic helpers shared by TC-F0003-002 .. TC-F0003-011
    // ------------------------------------------------------------------

    fn text_prop(tag_name: &str, value: &str) -> AddTagValue {
        AddTagValue {
            tag_id: None,
            tag_name: Some(tag_name.to_string()),
            value: EnumTagValue::Text(Some(value.to_string())),
        }
    }

    fn int_prop(tag_name: &str, value: i64) -> AddTagValue {
        AddTagValue {
            tag_id: None,
            tag_name: Some(tag_name.to_string()),
            value: EnumTagValue::Integer(Some(value)),
        }
    }

    fn bool_prop(tag_name: &str, value: bool) -> AddTagValue {
        AddTagValue {
            tag_id: None,
            tag_name: Some(tag_name.to_string()),
            value: EnumTagValue::Boolean(Some(value)),
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

    // ------------------------------------------------------------------
    // TC-F0003-002 — Equality on text containing a `'` (apostrophe doubling
    // outside `LIKE`). Proves the `to_sql_literal` route is used for `==`,
    // not only for `LIKE`, AND that unaccent_lower wraps both sides so
    // equality is case-insensitive.
    //
    // Items:
    //   A: "d'arc" -> match
    //   B: "darc"  -> excluded (apostrophe is a literal character)
    //   C: "D'ARC" -> match (unaccent_lower folds case)
    // Filter: <tag_text> == "d'arc"
    // ------------------------------------------------------------------
    #[test]
    fn t20_f0003_eq_text_with_apostrophe() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t20_f0003_eq_text_with_apostrophe", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-002 A (match)",
            vec![text_prop(&tag_text, "d'arc")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-002 B (no apostrophe)",
            vec![text_prop(&tag_text, "darc")],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-002 C (uppercase)",
            vec![text_prop(&tag_text, "D'ARC")],
        )?;

        let filter = format!(r#"({0} == "d'arc")"#, &tag_text);
        eprintln!("TC-F0003-002 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (\"d'arc\") to match — proves `'` is doubled for `==`"
        );
        assert!(
            reply.items.iter().any(|it| it.item_id == item_c.item_id),
            "expected item C (\"D'ARC\") to match — proves unaccent_lower wraps both sides"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (\"darc\") to be excluded — apostrophe is part of the literal"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-003 — Equality with an escaped double quote (`#"`) and `'`.
    // After §6 unescape the AST value is `L'"équipe"`.
    //
    // Items:
    //   A: `L'"équipe"` -> match
    //   B: `L'équipe`   -> excluded (no embedded `"`)
    //   C: `L"équipe"`  -> excluded (no `'`)
    // Filter: <tag_text> == "L'#"équipe#""
    // ------------------------------------------------------------------
    #[test]
    fn t30_f0003_eq_text_with_escaped_dquote() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t30_f0003_eq_text_with_escaped_dquote", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-003 A (match)",
            vec![text_prop(&tag_text, "L'\"équipe\"")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-003 B (no quotes)",
            vec![text_prop(&tag_text, "L'équipe")],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-003 C (no apostrophe)",
            vec![text_prop(&tag_text, "L\"équipe\"")],
        )?;

        let filter = format!(r#"({0} == "L'#"équipe#"")"#, &tag_text);
        eprintln!("TC-F0003-003 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (`L'\"équipe\"`) to match — stacks F0001 #\" unescape + F0002 `'` doubling + F0003 wrapping"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (no `\"`) to be excluded — exact-match semantics for `==`"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_c.item_id),
            "expected item C (no `'`) to be excluded — exact-match semantics for `==`"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-004 — `LIKE` with a bare leading wildcard (`%suffix`).
    //
    // Items:
    //   A: "golden end" -> match (ends with "end")
    //   B: "end of road" -> excluded ("end" not at the tail)
    //   C: "bend"       -> match (ends with "end")
    // Filter: <tag_text> LIKE "%end"
    // ------------------------------------------------------------------
    #[test]
    fn t40_f0003_like_leading_wildcard() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t40_f0003_like_leading_wildcard", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-004 A (golden end)",
            vec![text_prop(&tag_text, "golden end")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-004 B (end of road)",
            vec![text_prop(&tag_text, "end of road")],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-004 C (bend)",
            vec![text_prop(&tag_text, "bend")],
        )?;

        let filter = format!(r#"({0} LIKE "%end")"#, &tag_text);
        eprintln!("TC-F0003-004 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (\"golden end\") to match — leading bare `%` is a wildcard"
        );
        assert!(
            reply.items.iter().any(|it| it.item_id == item_c.item_id),
            "expected item C (\"bend\") to match — leading bare `%` is a wildcard"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (\"end of road\") to be excluded — pattern is anchored at the end"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-005 — `LIKE` with wildcards on both sides (`%mid%`), accented.
    //
    // Items:
    //   A: "de l'élite ranking" -> match
    //   B: "de l'ELITE ranking" -> match (accent + case folding via unaccent_lower)
    //   C: "de la masse ranking" -> excluded (middle literal enforced)
    // Filter: <tag_text> LIKE "%élite%"
    // ------------------------------------------------------------------
    #[test]
    fn t50_f0003_like_both_sides_wildcards_accents() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t50_f0003_like_both_sides_wildcards_accents", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-005 A (élite)",
            vec![text_prop(&tag_text, "de l'élite ranking")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-005 B (ELITE caps)",
            vec![text_prop(&tag_text, "de l'ELITE ranking")],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-005 C (masse)",
            vec![text_prop(&tag_text, "de la masse ranking")],
        )?;

        let filter = format!(r#"({0} LIKE "%élite%")"#, &tag_text);
        eprintln!("TC-F0003-005 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (\"élite\") to match — bare `%` on both sides are wildcards"
        );
        assert!(
            reply.items.iter().any(|it| it.item_id == item_b.item_id),
            "expected item B (\"ELITE\") to match — unaccent_lower folds case + accents"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_c.item_id),
            "expected item C (\"masse\") to be excluded — middle literal must match"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-006 — `LIKE` with only a literal `%` (`#%`), no wildcard.
    //
    // Items:
    //   A: "50%"  -> match
    //   B: "50X"  -> excluded (no `%`)
    //   C: "50%%" -> excluded (no trailing wildcard in the pattern)
    // Filter: <tag_text> LIKE "50#%"
    // ------------------------------------------------------------------
    #[test]
    fn t60_f0003_like_only_literal_percent() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t60_f0003_like_only_literal_percent", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-006 A (50%)",
            vec![text_prop(&tag_text, "50%")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-006 B (50X)",
            vec![text_prop(&tag_text, "50X")],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-006 C (50%%)",
            vec![text_prop(&tag_text, "50%%")],
        )?;

        let filter = format!(r#"({0} LIKE "50#%")"#, &tag_text);
        eprintln!("TC-F0003-006 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (\"50%\") to match — `#%` rendered as `\\%` with ESCAPE"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (\"50X\") to be excluded — `#%` is literal `%`, not a wildcard"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_c.item_id),
            "expected item C (\"50%%\") to be excluded — no implicit trailing wildcard"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-007 — Inequality `!=` with a `'`. Excludes the exact match,
    // keeps the rest (apostrophe is part of the literal compared).
    //
    // Items:
    //   A: "O'Brien"  -> excluded (equal)
    //   B: "O'Connor" -> match
    //   C: "OBrien"   -> match (apostrophe is part of the literal)
    // Filter: <tag_text> != "O'Brien"
    // ------------------------------------------------------------------
    #[test]
    fn t70_f0003_neq_text_with_apostrophe() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t70_f0003_neq_text_with_apostrophe", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-007 A (O'Brien)",
            vec![text_prop(&tag_text, "O'Brien")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-007 B (O'Connor)",
            vec![text_prop(&tag_text, "O'Connor")],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-007 C (OBrien)",
            vec![text_prop(&tag_text, "OBrien")],
        )?;

        let filter = format!(r#"({0} != "O'Brien")"#, &tag_text);
        eprintln!("TC-F0003-007 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().all(|it| it.item_id != item_a.item_id),
            "expected item A (\"O'Brien\") to be excluded — `'` doubling must apply to `!=` too"
        );
        assert!(
            reply.items.iter().any(|it| it.item_id == item_b.item_id),
            "expected item B (\"O'Connor\") to match"
        );
        assert!(
            reply.items.iter().any(|it| it.item_id == item_c.item_id),
            "expected item C (\"OBrien\") to match — apostrophe is part of the literal"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-008 — Boolean equality combined with a text `LIKE`.
    //
    // Items:
    //   A: text="open d'office", bool=TRUE  -> match
    //   B: text="open d'office", bool=FALSE -> excluded
    //   C: text="closed",        bool=TRUE  -> excluded
    // Filter: (<tag_text> LIKE "open d'office%") AND (<tag_bool> == TRUE)
    // ------------------------------------------------------------------
    #[test]
    fn t80_f0003_text_like_and_bool_eq() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t80_f0003_text_like_and_bool_eq", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();
        let tag_bool = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-008 A (open + TRUE)",
            vec![text_prop(&tag_text, "open d'office"), bool_prop(&tag_bool, true)],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-008 B (open + FALSE)",
            vec![text_prop(&tag_text, "open d'office"), bool_prop(&tag_bool, false)],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-008 C (closed + TRUE)",
            vec![text_prop(&tag_text, "closed"), bool_prop(&tag_bool, true)],
        )?;

        let filter = format!(
            r#"({0} LIKE "open d'office%") AND ({1} == TRUE)"#,
            &tag_text, &tag_bool
        );
        eprintln!("TC-F0003-008 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (text matches + bool TRUE) to match — text `'` doubled + bool emitted bare"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (bool FALSE) to be excluded — boolean branch is evaluated"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_c.item_id),
            "expected item C (text mismatch) to be excluded — text branch is evaluated"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-009 — Numeric `OR` of two ranges on the same int tag.
    //
    // Items:
    //   A: 10 -> match (< 18)
    //   B: 42 -> excluded (middle)
    //   C: 80 -> match (> 65)
    //   D: 18 -> excluded (`< 18` is strict)
    // Filter: (<tag_int> < 18) OR (<tag_int> > 65)
    // ------------------------------------------------------------------
    #[test]
    fn t90_f0003_numeric_or_two_ranges() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t90_f0003_numeric_or_two_ranges", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_int = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-009 A (10)",
            vec![int_prop(&tag_int, 10)],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-009 B (42)",
            vec![int_prop(&tag_int, 42)],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-009 C (80)",
            vec![int_prop(&tag_int, 80)],
        )?;
        let item_d = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-009 D (18)",
            vec![int_prop(&tag_int, 18)],
        )?;

        let filter = format!(
            r#"({0} < 18) OR ({0} > 65)"#,
            &tag_int
        );
        eprintln!("TC-F0003-009 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (10) to match — `< 18` branch"
        );
        assert!(
            reply.items.iter().any(|it| it.item_id == item_c.item_id),
            "expected item C (80) to match — `> 65` branch"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (42) to be excluded — middle of the range"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_d.item_id),
            "expected item D (18) to be excluded — `< 18` is strict"
        );

        lookup.close();
        Ok(())
    }

    // ------------------------------------------------------------------
    // TC-F0003-011 — Literal `##` (hash) in equality. After §6 unescape the
    // AST value is `see paragraph #6` — a single `#`.
    //
    // Items:
    //   A: "see paragraph #6"   -> match
    //   B: "see paragraph 6"    -> excluded (no `#`)
    //   C: "see paragraph ##6"  -> excluded (two `#`, value sent verbatim on create)
    // Filter: <tag_text> == "see paragraph ##6"
    // ------------------------------------------------------------------
    #[test]
    fn t110_f0003_eq_text_with_hash_escape() -> Result<(), ApiError<'static>> {
        let lookup = Lookup::new("t110_f0003_eq_text_with_hash_escape", TEST_TO_RUN);
        let props = lookup.props();

        let admin_server = AdminServerClient::new("localhost", 30060);
        let login_reply = admin_server.login(&get_login_request(&props))?;
        let document_server = DocumentServerClient::new("localhost", 30070);

        let tag_text = generate_random_tag();

        let item_a = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-011 A (#6)",
            vec![text_prop(&tag_text, "see paragraph #6")],
        )?;
        let item_b = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-011 B (no hash)",
            vec![text_prop(&tag_text, "see paragraph 6")],
        )?;
        let item_c = create_item_with_props(
            &document_server,
            &login_reply.session_id,
            "TC-011 C (##6)",
            vec![text_prop(&tag_text, "see paragraph ##6")],
        )?;

        let filter = format!(r#"({0} == "see paragraph ##6")"#, &tag_text);
        eprintln!("TC-F0003-011 filter: {}", &filter);

        let reply = document_server.search_item(Some(&filter), &login_reply.session_id)?;

        assert!(
            reply.items.iter().any(|it| it.item_id == item_a.item_id),
            "expected item A (one `#`) to match — DFS `##` unescaped to single `#`"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_b.item_id),
            "expected item B (no `#`) to be excluded — exact match required"
        );
        assert!(
            reply.items.iter().all(|it| it.item_id != item_c.item_id),
            "expected item C (two `#`) to be excluded — value sent to DB is `#6`, not `##6`"
        );

        lookup.close();
        Ok(())
    }
}
