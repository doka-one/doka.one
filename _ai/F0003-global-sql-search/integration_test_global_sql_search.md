
```yaml
id: IT-F0003
title: Integration tests — Global SQL Search (DFS → item search end-to-end)
type: integration-test
status: draft
target_flow: DOKA-ITEM-SEARCH
related_fix: F0003
naming_prefix: IT-F0003
language: rust
```

# Coverage goal
Behavioural end-to-end validation, **through the public document-server
HTTP API**, that an item search driven by a DFS expression containing
F0001 / F0002 escapes returns the expected items once F0003 routes every
value literal through `to_sql_literal` inside the `unaccent_lower(…)`
wrapper.

What is asserted (black-box, no SQL inspection):

- a `'` inside a value (e.g. `l'élite`) does not break the search —
  proves `'` is doubled inside `unaccent_lower('…')` (F0002 rule 1);
- a `#%` inside a `LIKE` pattern matches **only** the literal `%`
  character, not any sequence — proves `\%` + `ESCAPE '\'` is emitted
  (F0002 rule 3, `Literal` branch);
- a bare `%` in a `LIKE` pattern still matches any sequence — proves
  `AnySequence` is emitted bare (F0002 rule 3);
- a numeric branch (`age >= 18`) co-exists with the text `LIKE` branch
  and is itself unaffected — proves F0003 business rules 2 / 3 do not
  leak across conditions;
- overall query validity — the HTTP call returns `Ok(_)` rather than
  500, indirectly proving the generated SQL parses under PostgreSQL.

# System under test
The **public document-server HTTP API**, not the internal
`generate_search_sql` function. Specifically:

- `document-server` on `localhost:30070`, exercised through
  `doka_cli::request_client::DocumentServerClient::{create_item,
  search_item}`;
- `admin-server` on `localhost:30060` for the login round-trip via
  `AdminServerClient::login`.

No mock, no internal SQL string inspection — the test only observes
what the API returns. The escaping rules of F0001 / F0002 / F0003 are
validated **indirectly**: each behavioural outcome (item matched /
excluded / no HTTP error) maps to a specific escape rule, listed
under *Then* below.

# Test cases

## TC-F0003-001 — Composite expression: `LIKE` with escape rules AND a numeric condition

Given:
- DFS input (built with the randomised tag names below):
  `(<tag_text> LIKE "50 #% de l'élite%") AND (<tag_int> >= 18)`
- Two random tag names for this run (see `generate_random_tag()` in
  `ut60`), to avoid cross-run collisions on the shared customer:
  - `<tag_text>` declared as a text tag;
  - `<tag_int>` declared as an int tag.
- Three items seeded via `document_server.create_item(...)`:

  | Item | `<tag_text>` value          | `<tag_int>` value | Should match |
  |------|-----------------------------|-------------------|--------------|
  | A    | `50 % de l'élite ranking`   | `25`              | yes          |
  | B    | `50X de l'élite ranking`    | `25`              | no — no literal `%` after `50 ` |
  | C    | `50 % de l'élite ranking`   | `15`              | no — `age < 18` |

  Item A's text value packs every escape axis into one string: a
  literal `%` (matched by `#%`), a `'` inside `l'élite` (must be
  doubled in SQL), and a trailing free sequence (matched by the
  trailing wildcard).

Parser-level facts (already verified by IT-F0001, kept here for
traceability):
- canonical form of the AST:
  `([<tag_text><LIKE>50 %\ de l'élite\%\]AND[<tag_int><GTE>18])`
- text condition AST value:
  `ValuePattern([Literal("50 % de l'élite"), AnySequence])`
- int condition AST value: `ValueInt(18)`

When:
- the test logs in via `AdminServerClient::login`, then calls
  `document_server.search_item(Some(&filter), &session_id)?` with the
  DFS filter above.

Then (assertions on the HTTP reply, **not** on the internal SQL):

1. **Item A is returned.**
   `reply.items.iter().any(|it| it.item_id == item_a.item_id)` is
   `true`. This single positive outcome exercises four rules at once:
   - the `'` in `l'élite` was doubled in the generated SQL (F0002
     rule 1) — otherwise the SQL would be malformed and the call
     would have returned an error, not item A;
   - the `#%` was rendered as `\%` with an `ESCAPE '\'` clause (F0002
     rule 3, `Literal` branch) — otherwise A's literal `%` would not
     have matched the pattern;
   - the trailing `%` was rendered as bare `%` (F0002 rule 3,
     `AnySequence` branch) — otherwise the ` ranking` suffix would
     not have matched;
   - the `ESCAPE '\'` clause sits outside the `unaccent_lower(…)`
     call (F0003 business rule 2) — otherwise the SQL would be
     malformed and the call would have errored out.

2. **Item B is excluded.**
   `reply.items.iter().all(|it| it.item_id != item_b.item_id)` is
   `true`. If F0003 had emitted `#%` as a bare wildcard, B's
   `50X de l'élite…` would have matched. The exclusion proves that
   literal-`%` semantics are enforced.

3. **Item C is excluded.**
   `reply.items.iter().all(|it| it.item_id != item_c.item_id)` is
   `true`. Proves the `<tag_int> >= 18` branch is honoured (F0003
   business rule 3) and that the `ESCAPE '\'` clause did not leak
   into the int sub-query (which would have produced a parse error
   surfaced as a 500, not a silent mis-match).

4. **The search did not error out.**
   `search_item` returned `Ok(_)`. This proves the assembled SQL is
   syntactically valid PostgreSQL — every `'` was doubled, every
   literal `%` / `_` was prefixed, and the `ESCAPE '\'` clause was
   correctly attached to the `LIKE`, not nested inside
   `unaccent_lower(…)`.

The test lives in its own dedicated file [`doka-api-tests/tests/F0003-global-sql-search.rs`](../../doka-api-tests/tests/F0003-global-sql-search.rs), and follows the practice 
of doka integration tests.

---

# Additional test cases (TC-F0003-002 … TC-F0003-009, TC-F0003-011)

> TC-F0003-010 (date-as-string lexicographic `>=` on a text tag) was
> removed: the current document-server implementation does not allow
> order operators (`>`, `>=`, `<`, `<=`) on `TagType::Text` — see the
> whitelist `LEGAL_OPERATORS_BY_TAG_TYPE` in
> [`document-server/src/engine/generator.rs`](../../document-server/src/engine/generator.rs).
> Re-introduce when ordering on text tags is supported.

The cases below extend coverage to operators and escape combinations not
exercised by TC-F0003-001. They share the same conventions:

- Tag names are randomised per run (`generate_random_tag()` from `ut60`)
  to avoid collisions on the shared customer schema.
- The system under test is still the public document-server HTTP API —
  no SQL inspection, no mocks. Every assertion is on the items returned
  by `document_server.search_item(...)`.
- An implicit assertion applies to every case: `search_item` must
  return `Ok(_)`. A `5xx` reply would mean the generated SQL is
  malformed (typically a single `'` that escaped the doubling, or an
  `ESCAPE '\'` clause nested inside `unaccent_lower(...)`).
- Each case lists, in the *Then* block, which F0001 / F0002 / F0003
  rule(s) the behavioural outcome proves.

All ten cases live in
[`doka-api-tests/tests/F0003-global-sql-search.rs`](../../doka-api-tests/tests/F0003-global-sql-search.rs),
under the same `f0003_global_sql_search_tests` module, registered in
`TEST_TO_RUN`.

## TC-F0003-002 — Equality on text containing a `'` (apostrophe doubling outside `LIKE`)

Given:
- One random text tag `<tag_text>`.
- Three seeded items:

  | Item | `<tag_text>` value     | Should match |
  |------|------------------------|--------------|
  | A    | `d'arc`                | yes          |
  | B    | `darc`                 | no           |
  | C    | `D'ARC`                | yes — `unaccent_lower` makes equality case-insensitive |

DFS filter: `<tag_text> == "d'arc"`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Item A is returned — proves `'` is doubled inside
   `unaccent_lower('d''arc')` (F0002 rule 1) even for the `=` operator,
   not only for `LIKE`. Without doubling the SQL would be malformed and
   the call would have failed.
2. Item C is returned — proves the text branch is wrapped in
   `unaccent_lower(...)` on both sides (F0003 rule 2), so the
   comparison is case-insensitive.
3. Item B is excluded — proves the apostrophe is treated as a literal
   character of the value, not stripped or collapsed.

## TC-F0003-003 — Equality with an escaped double quote (`#"`) and apostrophe

Given:
- One random text tag `<tag_text>`.
- Three seeded items:

  | Item | `<tag_text>` value     | Should match |
  |------|------------------------|--------------|
  | A    | `L'"équipe"`           | yes          |
  | B    | `L'équipe`             | no — no embedded `"` |
  | C    | `L"équipe"`            | no — no `'`  |

DFS filter: `<tag_text> == "L'#"équipe#""`.

(After §6 unescape the AST value is the 11-character string
`L'"équipe"`.)

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Item A is returned — proves the DFS `#"` escape is unescaped to a
   literal `"` before reaching the SQL layer, AND that the `'` is
   doubled inside `unaccent_lower('L''"équipe"')` (F0001 + F0002 rule 1
   + F0003 rule 2 stacked in a single value).
2. Items B and C are excluded — proves exact-match semantics for `==`
   over text wrapped in `unaccent_lower(...)`.

## TC-F0003-004 — `LIKE` with a bare leading wildcard (`%suffix`)

Given:
- One random text tag `<tag_text>`.
- Three seeded items:

  | Item | `<tag_text>` value     | Should match |
  |------|------------------------|--------------|
  | A    | `golden end`           | yes — ends with `end` |
  | B    | `end of road`          | no  — `end` is not at the tail |
  | C    | `bend`                 | yes — `bend` ends with `end` |

DFS filter: `<tag_text> LIKE "%end"`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Items A and C are returned — proves the leading bare `%` is emitted
   as a bare `%` (F0002 rule 3, `AnySequence` branch) and not as `\%`.
2. Item B is excluded — proves the pattern is anchored at the end of
   the value (no trailing wildcard).
3. No `ESCAPE '\'` clause is needed for this pattern; the call must
   still succeed, proving F0003 only appends `ESCAPE` when a `Literal`
   `%`/`_` is actually present (F0002 rule 3).

## TC-F0003-005 — `LIKE` with wildcards on both sides (`%mid%`) over accented text

Given:
- One random text tag `<tag_text>`.
- Three seeded items:

  | Item | `<tag_text>` value          | Should match |
  |------|-----------------------------|--------------|
  | A    | `de l'élite ranking`        | yes          |
  | B    | `de l'ELITE ranking`        | yes — `unaccent_lower` folds case + accent |
  | C    | `de la masse ranking`       | no           |

DFS filter: `<tag_text> LIKE "%élite%"`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Item A is returned — proves both bare `%` are emitted as wildcards
   (F0002 rule 3, `AnySequence`).
2. Item B is returned — proves the `unaccent_lower(...)` wrapper is
   applied to both sides of the `LIKE` (F0003 rule 2), making the
   match accent- and case-insensitive.
3. Item C is excluded — proves the middle literal `élite` is enforced
   verbatim and not collapsed by the wildcards.

## TC-F0003-006 — `LIKE` with only a literal `%` (`#%`), no wildcard at all

Given:
- One random text tag `<tag_text>`.
- Three seeded items:

  | Item | `<tag_text>` value     | Should match |
  |------|------------------------|--------------|
  | A    | `50%`                  | yes — exact literal percent |
  | B    | `50X`                  | no  — no `%` at all |
  | C    | `50%%`                 | no  — extra trailing `%` |

DFS filter: `<tag_text> LIKE "50#%"`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Item A is returned — proves `#%` is rendered as `\%` with an
   `ESCAPE '\'` clause sitting outside `unaccent_lower(...)` (F0002
   rule 3, `Literal` branch + F0003 rule 2).
2. Items B and C are excluded — proves the pattern matches `50%`
   exactly, with no implicit trailing wildcard. If `#%` had leaked as
   a bare `%`, B (`50X`) would have matched; C (`50%%`) would also
   match either reading, so its exclusion specifically pins the
   "no trailing wildcard" invariant.

## TC-F0003-007 — Inequality `!=` with a `'` excludes the exact match, keeps the rest

Given:
- One random text tag `<tag_text>`.
- Three seeded items:

  | Item | `<tag_text>` value     | Should match |
  |------|------------------------|--------------|
  | A    | `O'Brien`              | no  — equal to the filter value |
  | B    | `O'Connor`             | yes |
  | C    | `OBrien`               | yes — apostrophe is part of the literal, not optional |

DFS filter: `<tag_text> != "O'Brien"`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Item A is excluded — proves `'` is correctly doubled for `!=` (the
   `to_sql_literal` route, F0002 rule 1, applies to every operator and
   not just `=` / `LIKE`).
2. Items B and C are returned — proves `!=` is honoured against the
   `unaccent_lower(...)` wrapper (F0003 rule 2) and that the apostrophe
   is part of the literal being compared (no silent normalisation).

## TC-F0003-008 — Boolean equality combined with a text `LIKE`

Given:
- Two random tag names: `<tag_text>` (text), `<tag_bool>` (boolean).
- Three seeded items:

  | Item | `<tag_text>` value       | `<tag_bool>` value | Should match |
  |------|--------------------------|--------------------|--------------|
  | A    | `open d'office`          | `TRUE`             | yes          |
  | B    | `open d'office`          | `FALSE`            | no           |
  | C    | `closed`                 | `TRUE`             | no           |

DFS filter:
`(<tag_text> LIKE "open d'office%") AND (<tag_bool> == TRUE)`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Item A is returned — proves the text branch handled `'` doubling
   (F0002 rule 1) and that the boolean branch was emitted as the bare
   atom `TRUE` (F0003 rule 3) without an `unaccent_lower(...)` wrapper.
   A wrapped boolean would have produced a parse error surfaced as a
   500.
2. Item B is excluded — proves the boolean branch is evaluated and not
   short-circuited.
3. Item C is excluded — proves the text branch is evaluated; the
   trailing wildcard does not allow `closed` to slip in.

## TC-F0003-009 — Numeric `OR` of two ranges on the same int tag

Given:
- One random int tag `<tag_int>`.
- Four seeded items:

  | Item | `<tag_int>` value | Should match |
  |------|-------------------|--------------|
  | A    | `10`              | yes — below 18 |
  | B    | `42`              | no  — in the excluded middle |
  | C    | `80`              | yes — above 65 |
  | D    | `18`              | no  — boundary: `< 18` is strict |

DFS filter: `(<tag_int> < 18) OR (<tag_int> > 65)`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Items A and C are returned — proves both branches are kept after
   `OR` and that the int branch uses `tv.value_integer` without an
   `unaccent_lower(...)` wrapper (F0003 rule 3); a wrapped int would
   have produced a 500.
2. Items B and D are excluded — proves the strict-vs-loose operator
   distinction (`<` vs `<=`) is preserved end-to-end (F0001 operator
   round-trip).

## TC-F0003-011 — Literal `##` (hash) in equality

Given:
- One random text tag `<tag_text>`.
- Three seeded items:

  | Item | `<tag_text>` value     | Should match |
  |------|------------------------|--------------|
  | A    | `see paragraph #6`     | yes          |
  | B    | `see paragraph 6`      | no           |
  | C    | `see paragraph ##6`    | no — value contains two `#`, not one |

DFS filter: `<tag_text> == "see paragraph ##6"`.

(After §6 unescape the AST value is `see paragraph #6` — a single `#`.)

Parser-level fact (already covered by IT-F0001, kept for traceability):
the canonical form of the AST condition value is `see paragraph #6`.

When: `search_item(Some(&filter), &session_id)`.

Then:
1. Item A is returned — proves the DFS `##` escape is unescaped to a
   single `#` before reaching `to_sql_literal`, and that `#` itself
   has no special meaning at the SQL layer (no spurious escaping
   applied — `#` is not a PostgreSQL `LIKE` meta-character anyway, but
   the proof here is that F0003 does not over-escape it on the `=`
   operator path).
2. Items B and C are excluded — proves exact-match semantics under
   the `unaccent_lower(...)` wrapper; in particular C confirms that
   the value sent to the DB was `#6`, not `##6`.
