
```yaml
id: UT-F0005
title: Unit tests — Date filter support
type: unit-test
status: draft
target_flow: DOKA-ITEM-SEARCH
related_feature: F0005
naming_prefix: UT-F0005
language: rust
```

# Coverage goal
White-box validation of the three new touch points introduced by F0005, exercised in-process via `cargo test --bin document-server`:

- the new `filter/type_resolver.rs` module (`collect_attribute_names`, `resolve_filter_value_types`, `ResolvedFilter`, `FilterResolutionError`);
- the new `TagType::Date` arm in `build_tag_value_filter` and the new entry in `LEGAL_OPERATORS_BY_TAG_TYPE`;
- the new `FilterValue::ValueDate` variant's `Display` impl and the `to_sql_literal` defensive arm.

End-to-end HTTP behaviour is covered separately by [IT-F0005](./integration_test_date_filter.md).

# System under test
Functions are called directly inside `#[cfg(test)]` modules. No HTTP layer, no DB, no `admin-server`. **Resolver tests build their input AST by direct enum construction** (`FilterExpressionAST::Condition { ... }`, `FilterExpressionAST::Logical { ... }`) — *not* via `analyse_expression`. This isolates a resolver-test failure to a resolver bug; lexer/parser behaviour is already covered by the existing tests around `analyse_expression` and `parse_tokens`. Each test also supplies a small hand-built `Vec<TagDefinition>` and asserts against the returned `ResolvedFilter` or `FilterResolutionError`.

# Test cases

## type_resolver (new module — 6 cases)

### TC-F0005-001 — `collect_attribute_names` walks AND/OR trees
Given: an AST built by direct construction —
```
Logical { AND, [
  Condition { attribute: "birthdate", op: EQ, value: ValueString("2025-12-31") },
  Logical { OR, [
    Condition { attribute: "age",  op: GT, value: ValueInt(18) },
    Condition { attribute: "city", op: EQ, value: ValueString("Paris") },
  ]},
]}
```
When: `collect_attribute_names(&ast)`.
Then: returns the set `{"birthdate", "age", "city"}`. Proves the walk visits every `Condition` leaf inside nested `Logical` nodes — no parser involvement.

### TC-F0005-002 — `resolve_filter_value_types` happy path
Given: an AST built directly as `Condition { attribute: "birthdate", op: EQ, value: ValueString("2025-12-31") }` + `definitions = [TagDefinition { tag_names: "birthdate", tag_type: Date }]`.
When: `resolve_filter_value_types(ast, definitions)`.
Then: `Ok(ResolvedFilter { ast, definitions })` where the single `Condition`'s `value` is `FilterValue::ValueDate(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap())`. Proves the rewrite produces a `chrono::NaiveDate`, not a string.

### TC-F0005-003 — Invalid date literal is rejected
Given: hand-built AST `Condition { attribute: "birthdate", op: EQ, value: ValueString("2025-13-40") }` + birthdate-as-Date definition.
When: `resolve_filter_value_types(ast, definitions)`.
Then: `Err(FilterResolutionError::InvalidDateLiteral { tag: "birthdate", value: "2025-13-40" })`. Repeat with `ValueString("not-a-date")` — same error.

### TC-F0005-004 — Wrong value shape on a Date tag
Given: hand-built AST `Condition { attribute: "birthdate", op: EQ, value: FilterValue::ValueInt(2025) }` + birthdate-as-Date definition. (This shape cannot be produced by the parser; the resolver must still reject it defensively.)
When: `resolve_filter_value_types(...)`.
Then: `Err(FilterResolutionError::IncompatibleValueShape { tag: "birthdate", tag_type: Date, got: "ValueInt" })`.

### TC-F0005-005 — Non-Date tags pass through unchanged
Given: hand-built AST —
```
Logical { AND, [
  Condition { attribute: "name", op: EQ, value: ValueString("alice") },
  Condition { attribute: "age",  op: GT, value: ValueInt(18) },
]}
```
+ definitions `[name: Text, age: Int]`.
When: `resolve_filter_value_types(...)`.
Then: `Ok(resolved)` where each `Condition.value` is bit-for-bit identical to the input (`ValueString("alice")`, `ValueInt(18)`). Proves the resolver is a no-op outside the `Date` case.

### TC-F0005-006 — `ResolvedFilter.definitions` preserves the input
Given: a `definitions` vec of length 3 in a specific order + any trivial hand-built AST referencing one of them.
When: `resolve_filter_value_types(...)`.
Then: `resolved.definitions` equals the input vec by element and order. Proves the bundle moves ownership without reordering or dropping entries — SQL gen relies on linear lookups by name.

## engine/generator (4 cases added to the existing `#[cfg(test)]` module)

### TC-F0005-007 — `build_tag_value_filter` emits `DATE 'YYYY-MM-DD'` for EQ
Given: `FilterCondition { attribute: "birthdate", operator: EQ, value: ValueDate(NaiveDate::from_ymd_opt(2025,12,31).unwrap()) }` + `tag_type = TagType::Date`.
When: `build_tag_value_filter(&fc, &tag_type)`.
Then: `Ok("tv.value_date = DATE '2025-12-31'")`. Note the `DATE '…'` prefix — a bare `'2025-12-31'` would be wrong.

### TC-F0005-008 — All ordered operators emit the right symbol
Table-driven over `{NEQ → <>, GT → >, GTE → >=, LT → <, LTE → <=}` on the same date literal. Proves the operator switch reused from text/int paths applies to dates.

### TC-F0005-009 — Defensive arm rejects non-`ValueDate` values
Given: `FilterCondition { ..., value: ValueString("2025-12-31") }` + `tag_type = TagType::Date` (only reachable if the resolver was bypassed — defensive check).
When: `build_tag_value_filter(&fc, &Date)`.
Then: `Err(GenerationError::TagIncompatibleType(_))` mentioning the attribute name. Proves the SQL stage refuses to silently fall back to text behaviour.

### TC-F0005-010 — `LIKE` is rejected on a Date tag at the operator-check stage
Given: `filter_conditions` containing `(birthdate LIKE "x")` + `definitions = [birthdate: Date]`.
When: `verify_filter_conditions(&filter_conditions, &definitions)`.
Then: `Err(GenerationError::TagIncompatibleType(_))`. Proves `LEGAL_OPERATORS_BY_TAG_TYPE[Date]` excludes `LIKE` (the entry must include exactly `EQ, NEQ, GT, GTE, LT, LTE`).

## filter/filter_ast + filter/mod (2 cases)

### TC-F0005-011 — `Display` for `ValueDate` emits ISO `YYYY-MM-DD`
Given: `FilterValue::ValueDate(NaiveDate::from_ymd_opt(2025,12,31).unwrap())`.
When: `format!("{}", v)`.
Then: `"2025-12-31"`. Proves alignment with the SQL emission format and prevents accidental locale-dependent formatting.

### TC-F0005-012 — `to_sql_literal` defensive arm for `ValueDate`
Given: `FilterValue::ValueDate(NaiveDate::from_ymd_opt(2025,12,31).unwrap())` + any non-LIKE operator.
When: `to_sql_literal(&v, &op)`.
Then: `"DATE '2025-12-31'"`. This arm is unreachable in the F0005 pipeline (the `Date` branch of `build_tag_value_filter` emits SQL directly), but the match must be exhaustive — the test guards against the arm silently emitting `'2025-12-31'` without the `DATE` prefix if the surrounding code ever delegates to it.

# Regression coverage
All existing tests across `filter/filter_lexer.rs`, `filter/filter_normalizer.rs`, `filter/filter_ast.rs`, `filter/mod.rs`, and `engine/generator.rs` must continue to pass with **no test-body edits**. The only compilation effect of F0005 on existing code is the two new exhaustive-match arms (`Display`, `to_sql_literal`); once they are added, every `Text`/`Int`/`Bool`/`Double` filter test runs untouched. A CI failure on any pre-existing test name indicates an unintended regression in F0005, not a missing update.

# Test file
The new cases live in the `#[cfg(test)]` module of their owning production file:

- TC-F0005-001…006 — `document-server/src/filter/type_resolver.rs` (new file, `mod tests`).
- TC-F0005-007…010 — appended to the existing `#[cfg(test)] mod tests` in `document-server/src/engine/generator.rs`.
- TC-F0005-011 — appended to `#[cfg(test)] mod tests` in `document-server/src/filter/filter_ast.rs`.
- TC-F0005-012 — appended to `#[cfg(test)] mod tests` in `document-server/src/filter/mod.rs`.

Run with `cargo test --color=always --bin document-server F0005` (substring match catches all new cases by their `TC-F0005-…` naming).

# Coding norms
Follow the coding norms from : _ai/guides/Code standards and norms.md
