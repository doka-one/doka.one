
```yaml
id: IT-F0002
title: Integration tests — SQL Escaping (DFS → PostgreSQL)
type: integration-test
status: draft
target_flow: DOKA-ITEM-SEARCH
related_fix: F0002
naming_prefix: IT-F0002
language: rust
```

# Coverage goal
Validate that the SQL fragment emitted from a DFS `FilterCondition` is
syntactically valid PostgreSQL (9.1+) and preserves the wildcard vs
literal semantics of `%` and `_` in `LIKE`, as specified in
[F0002.md](F0002.md). The tests check:

- doubling of `'` in every emitted SQL string literal;
- pass-through of `%`, `_`, `\`, `"`, `#` inside non-`LIKE` literals
  (`standard_conforming_strings = on`);
- emission of `LIKE` patterns: `AnySequence` → bare `%`, literal
  `%` / `_` / `\` → `\%` / `\_` / `\\` plus an `ESCAPE '\'` clause;
- omission of the `ESCAPE '\'` clause when no literal escape character
  is present (purely cosmetic);
- regression on plain values that need no escaping.

System under test: the SQL emission path in
[`document-server/src/filter/mod.rs`](../../document-server/src/filter/mod.rs)
(the function that turns a `FilterCondition` / `ValuePattern` into the
operator + value-literal SQL fragment). The surrounding
`unaccent_lower((tv.value_string)::text) … unaccent_lower('…')` wrapper
is out of scope — each assertion targets only the operator + value-literal
substring, as in the example table of F0002.md.

# Test cases

## TC-F0002-001 — Single-quote doubling on `=`
Given:
- DFS input: `name == "d'arc"`
- canonical form (parser output): `[name<EQ>d'arc]`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `= 'd''arc'`
- the value's `'` is doubled exactly once
- no `ESCAPE` clause is present (operator is `=`, not `LIKE`)

## TC-F0002-002 — Special characters pass through on non-`LIKE`
Given:
- DFS input: `team == "L'#"équipe#""`
- canonical form (parser output): `[team<EQ>L'"équipe"]`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `= 'L''"équipe"'`
- the embedded `"` characters are passed through unchanged
- the `'` is doubled per rule 1
- `#` does **not** appear in the fragment (it was consumed by the DFS
  parser as an escape introducer, not by the SQL emitter)

Sibling assertion (same rule, backslash branch — rule 4):
- DFS input: `path == "C:\tmp"` with canonical form `[path<EQ>C:\tmp]`
- emitted SQL fragment: `= 'C:\tmp'`
- `\` is preserved verbatim (standard-conforming strings)

## TC-F0002-003 — `LIKE` with a pure wildcard, no `ESCAPE` clause
Given:
- DFS input: `name LIKE "den%"`
- canonical form (parser output): `[name<LIKE>den\%\]`
- AST value: `ValuePattern([Literal("den"), AnySequence])`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `LIKE 'den%'`
- the `%` is emitted bare (it is `AnySequence`, not a literal)
- the `ESCAPE '\'` clause is **omitted** — no `Literal` part contained
  `%`, `_` or `\`, so the clause is cosmetically unnecessary (rule 3)

## TC-F0002-004 — `LIKE` with a literal `%` requiring `ESCAPE '\'`
Given:
- DFS input: `code LIKE "50#%"`
- canonical form (parser output): `[code<LIKE>50%]`
- AST value: `ValuePattern([Literal("50%")])`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `LIKE '50\%' ESCAPE '\'`
- the literal `%` from the `Literal` part is prefixed with `\`
- the `ESCAPE '\'` clause is present (rule 3, main branch)

## TC-F0002-005 — `LIKE` mixing wildcards, literal `'`, and a trailing wildcard
Given:
- DFS input: `name LIKE "L'équipe de moi%"`
- canonical form (parser output): `[name<LIKE>L'équipe de moi\%\]`
- AST value: `ValuePattern([Literal("L'équipe de moi"), AnySequence])`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `LIKE 'L''équipe de moi%'`
- the `'` inside the `Literal` part is doubled (rule 1 applies inside
  `LIKE` patterns just as it does elsewhere)
- the trailing `%` is emitted bare (it is `AnySequence`)
- the `ESCAPE '\'` clause is **omitted** — the `Literal` part contained
  no `%`, `_` or `\`

## TC-F0002-006 — `LIKE` with literal `_` (and `%` wildcard)
Given:
- DFS input: `code LIKE "##_x_%"`
- canonical form (parser output): `[code<LIKE>#_x_\%\]`
- AST value: `ValuePattern([Literal("#_x_"), AnySequence])`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `LIKE '#\_x\_%' ESCAPE '\'`
- each literal `_` from the `Literal` part is prefixed with `\`
- the trailing `%` is `AnySequence` and is emitted bare
- the `ESCAPE '\'` clause is present (rule 3, main branch)

## TC-F0002-007 — `LIKE` with literal `\` (backslash doubling)
Given:
- DFS input: `path LIKE "C:\path%"`
- canonical form (parser output): `[path<LIKE>C:\path\%\]`
- AST value: `ValuePattern([Literal("C:\path"), AnySequence])`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `LIKE 'C:\\path%' ESCAPE '\'`
- the literal `\` is doubled to `\\` inside the `LIKE` pattern (rule 3)
- this is the **opposite** of rule 4: inside a non-`LIKE` literal, `\`
  passes through unchanged; inside `LIKE`, `\` must be escaped
- the `ESCAPE '\'` clause is present

## TC-F0002-008 — `!=` operator with `'` doubling
Given:
- DFS input: `name != "d'arc"`
- canonical form (parser output): `[name<NEQ>d'arc]`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `<> 'd''arc'`
- `'` doubling (rule 1) is applied identically across the full non-`LIKE`
  operator family (`=`, `<>`, `>`, `>=`, `<`, `<=`)
- no `ESCAPE` clause (not a `LIKE` operator)

## TC-F0002-009 — Non-`LIKE` value mixing `'` and pass-through `%`
Given:
- DFS input: `note == "ain't 100#%"`
- canonical form (parser output): `[note<EQ>ain't 100%]`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `= 'ain''t 100%'`
- the `'` is doubled (rule 1)
- the `%` is **not** escaped — it has no SQL meaning inside a
  single-quoted literal under `=` (rule 2)
- no `ESCAPE` clause is emitted (it is meaningful only with `LIKE`)

## TC-F0002-010 — `LIKE` combining `'` doubling, literal `%`, and `ESCAPE`
Given:
- DFS input: `note LIKE "L'eq 50#%"`
- canonical form (parser output): `[note<LIKE>L'eq 50%]`
- AST value: `ValuePattern([Literal("L'eq 50%")])`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `LIKE 'L''eq 50\%' ESCAPE '\'`
- the `'` is doubled (rule 1)
- the literal `%` is prefixed with `\` (rule 3)
- the `ESCAPE '\'` clause is present
- this case exercises rules 1 and 3 simultaneously, which none of the
  earlier `LIKE` cases does


## TC-F0002-011 — Boundary: empty string literal
Given:
- DFS input: `note == ""`
- canonical form (parser output): `[note<EQ>]`

When:
- the condition is emitted as SQL

Then:
- emitted SQL fragment: `= ''`
- the emitter produces a syntactically valid empty SQL string literal
  (two single quotes, not a bare `=` or a doubled `''''`)

## TC-F0002-013 — Composite `AND` with escaping on each side
Given:
- DFS input: `(name == "d'arc") AND (code LIKE "50#%")`
- canonical form (parser output):
  `([name<EQ>d'arc]AND[code<LIKE>50%])`

When:
- the composite expression is emitted as SQL

Then:
- the SQL fragment contains both conditions joined by `AND`, each with
  its own correctly escaped value-literal — for example:
  `… = 'd''arc' AND … LIKE '50\%' ESCAPE '\'`
- escaping is applied independently per `FilterCondition`; one branch's
  `LIKE` rules do not leak into the other branch's `=` literal
