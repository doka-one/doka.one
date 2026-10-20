
```yaml
id: IT-F0004
title: Integration tests — No-filter in item search (empty DFS → match all)
type: integration-test
status: draft
target_flow: DOKA-ITEM-SEARCH
related_feature: F0004
naming_prefix: IT-F0004
language: rust
```

# Coverage goal
Behavioural end-to-end validation, **through the public document-server
HTTP API**, that the four equivalent "no-filter" forms (omitted /
empty / whitespace-only / explicit `()`) are normalised to a
"match-all" search and that:

- the call returns `Ok(_)` (200 OK) — proves the delegate no longer
  routes the input through `analyse_expression("()")`;
- items created in the same test appear in the reply — proves the SQL
  produced for the sentinel AST is a valid, all-matching query;
- multiple items created in one test all appear together — proves
  "match all", not "match first";
- a non-empty malformed filter **still** returns an error — proves the
  normalisation is conservative and does not silently swallow bad
  input.

# System under test
The **public document-server HTTP API**, exercised through
`doka_cli::request_client::DocumentServerClient::{create_item,
search_item}`. `admin-server` on `localhost:30060` provides the session
via `AdminServerClient::login`. No mocks, no SQL inspection — every
assertion is on the items returned by `search_item`.

The `DocumentServerClient::search_item` signature

```rust
pub fn search_item(&self, filter: Option<&str>, sid: &str) -> WebResponse<GetItemReply>
```

makes the four no-filter wire forms directly reachable:

| Test case   | `filter` argument         | Wire effect                                |
|-------------|---------------------------|--------------------------------------------|
| TC-F0004-001 | `None`                    | `GET /search` (no `filters` param)        |
| TC-F0004-002 | `Some("")`                | `GET /search?filters=` ("")          |
| TC-F0004-003 | `Some("   ")`             | `GET /search?filters=%20%20%20`           |
| TC-F0004-004 | `Some("()")`              | `GET /search?filters=%28%29`              |

`order_tags` are intentionally **not** covered by IT-F0004: the CLI client does not expose them on `search_item`. The behaviour is
specified by F0004 rule 4 and is intended to be exercised by a follow-up IT once a client method exists. The spec calls this out
explicitly.

# Test cases

## TC-F0004-001 — `None` filter returns the created item

**Given**:

- One random text tag `<tag_text>` (`generate_random_tag()` from the F0003 test file, to avoid cross-run collisions on the shared customer).
- One seeded item:

  | Item | `<tag_text>` value | Should match |
  |------|--------------------|--------------|
  | A    | `match-all-001`    | yes          |

**Filter**: `None` (omitted query parameter).

When: `document_server.search_item(None, &session_id)`.

**Then**:

1. The call returns `Ok(_)` — proves the delegate no longer falls back to `analyse_expression("()")`.
2. Item A appears in `reply.items` — proves the sentinel-AST path produces a SQL query that matches all items, including the one just created.

## TC-F0004-002 — Empty-string filter is equivalent to `None`

Given: same seeding as TC-F0004-001 with a fresh random tag and
`<tag_text>` value `match-all-002`.

Filter: `Some("")`.

When: `document_server.search_item(Some(""), &session_id)`.

Then:
1. `Ok(_)`.
2. The created item appears in `reply.items` — proves the wire form
   `?filters=` is normalised to `None` by
   `normalize_filter_input` (F0004 rule 1).

## TC-F0004-003 — Whitespace-only filter is equivalent to `None`

Given: same seeding pattern with `<tag_text>` value `match-all-003`.

Filter: `Some("   ")` (three spaces).

When: `document_server.search_item(Some("   "), &session_id)`.

Then:
1. `Ok(_)`.
2. The created item appears in `reply.items` — proves the trim step in
   `normalize_filter_input` is applied before the empty check
   (F0004 rule 1).

## TC-F0004-004 — Explicit `()` filter is equivalent to `None`

Given: same seeding pattern with `<tag_text>` value `match-all-004`.

Filter: `Some("()")`.

When: `document_server.search_item(Some("()"), &session_id)`.

Then:
1. `Ok(_)`.
2. The created item appears in `reply.items` — proves `()` is short-circuited by the delegate and never reaches the lexer
   (which would still reject it with `EmptyLogicalOperation` if it did).

## TC-F0004-005 — Two items in one test, `None` filter: both appear

Given:
- One random text tag `<tag_text>`.
- Two seeded items:

  | Item | `<tag_text>` value      | Should match |
  |------|-------------------------|--------------|
  | A    | `match-all-005-A`       | yes          |
  | B    | `match-all-005-B`       | yes          |

Filter: `None`.

When: `document_server.search_item(None, &session_id)`.

Then:
1. `Ok(_)`.
2. Both A and B appear in `reply.items`. Proves the sentinel-AST SQL
   matches **all** rows, not just the first / a single row — guards against a regression where the implementation accidentally emits
   `WHERE FALSE` or a `LIMIT 1`-like shape.

## TC-F0004-006 — `None` reply is a superset of a matching filter reply

Given:
- One random text tag `<tag_text>`.
- One seeded item:

  | Item | `<tag_text>` value          | Should match both calls |
  |------|-----------------------------|-------------------------|
  | A    | `match-all-006-uniqueval`   | yes                     |

When:
- Call 1: `search_item(None, &session_id)` → `reply_all`.
- Call 2: `search_item(Some("(<tag_text> == \"match-all-006-uniqueval\")"), &session_id)` → `reply_filtered`.

Then:
1. Both calls return `Ok(_)`.
2. `reply_filtered.items` contains A.
3. Every item in `reply_filtered.items` is also in `reply_all.items` (by `item_id`). Proves the no-filter path is a true superset of the  filtered path — guards against a regression where the no-filter path accidentally diverges (e.g. wrong schema, missing rows).

## TC-F0004-007 — Malformed non-empty filter still returns an error

Given:
- No item seeding needed.

Filter: `Some("(unbalanced")` (open paren, no close, no value).

When: `document_server.search_item(Some("(unbalanced"), &session_id)`.

Then:
1. The call returns `Err(_)`. Proves `normalize_filter_input` does **not** silently re-route malformed
   input to the match-all path — only the four declared no-filter forms are accepted; everything else continues to be lexed/parsed by `analyse_expression` and surface the existing 400 contract (F0004 errors section).

# Regression coverage
The previously `#[ignore]`d test [`doka-api-tests/tests/ut60_api_item_search.rs::t40_search_no_filter`](../../doka-api-tests/tests/ut60_api_item_search.rs)  becomes redundant once IT-F0004 lands. Remove it. 

# Test file
All cases live in their own dedicated file [`doka-api-tests/tests/F0004-no-filter-in-search.rs`](../../doka-api-tests/tests/F0004-no-filter-in-search.rs), under the module  `f0004_no_filter_in_search_tests`, with the `TEST_TO_RUN` list following the F0003 convention.
