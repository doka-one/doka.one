
```yaml
id: IT-F0001
title: Integration tests — Filter Escaping (DFS)
type: integration-test
status: draft
target_flow: DOKA-ITEM-SEARCH
related_fix: F0001
naming_prefix: IT-F0001
language: rust
```

# Coverage goal
Validate the `#`-prefix escape support in string literals of the Doka Filter
Syntax (DFS), as specified in [F0001.md](F0001.md) and §6 of
[`_ai/specs/filter-syntax.md`](../specs/filter-syntax.md). The tests check:

- conversion of the sequences `##`, `#%`, `#"` into their literal characters
  in the intermediate value produced by the parser;
- the canonical form printed by the server after unescaping;
- syntax errors (`InvalidEscapeSequence`) when the escape rules are violated;
- regression on plain strings and on the `%` wildcard in `LIKE` (see
  "Open questions").

System under test: the filter parser in
[`document-server/src/filter/mod.rs`](../../document-server/src/filter/mod.rs)
(`analyse_expression` then `to_sql_form`).

# Test cases

## TC-F0001-001 — Escape a double quote (`#"`)
Given:
- filter expression: `category == "super #"extra#" player"`

When:
- the expression is parsed by `analyse_expression`

Then:
- parsing succeeds
- the literal value of the condition is `super "extra" player`
- canonical form: `([category<EQ>super "extra" player])`

## TC-F0001-002 — Escape a percent (`#%`) outside `LIKE`
Given:
- filter expression: `percent == "less than 10 #%"`

When:
- the expression is parsed

Then:
- parsing succeeds
- the literal value is `less than 10 %`
- canonical form: `([percent<EQ>less than 10 %])`

## TC-F0001-003 — Escape a hash (`##`)
Given:
- filter expression: `comment == "see paragraph ##6"`

When:
- the expression is parsed

Then:
- parsing succeeds
- the literal value is `see paragraph #6`
- canonical form: `([comment<EQ>see paragraph #6])`

## TC-F0001-004 — Multiple escape sequences in the same string
Given:
- filter expression: `note == "100#% ##1 says #"hi#""`

When:
- the expression is parsed

Then:
- parsing succeeds
- the literal value is `100% #1 says "hi"`

## TC-F0001-005 — Escapes combined with `AND` / `OR`
Given:
- filter expression:
  `(category == "super #"extra#" player") AND (percent == "less than 10 #%")`

When:
- the expression is parsed

Then:
- parsing succeeds
- canonical form:
  `([category<EQ>super "extra" player]AND[percent<EQ>less than 10 %])`

## TC-F0001-006 — Regression: string with no character to escape
Given:
- filter expression: `name == "denis"`

When:
- the expression is parsed

Then:
- parsing succeeds
- the literal value is `denis`
- canonical form: `([name<EQ>denis])`

## TC-F0001-007 — Error: `#` followed by a digit
Given:
- filter expression: `comment == "see paragraph #6"`

When:
- the expression is parsed

Then:
- parsing fails
- error code: `InvalidEscapeSequence`
- the message contains `Invalid escape sequence in string literal at position 27`

## TC-F0001-008 — Error: `#` followed by an arbitrary letter
Given:
- filter expression: `comment == "C#a"`

When:
- the expression is parsed

Then:
- parsing fails
- error code: `InvalidEscapeSequence`
- the message contains `Invalid escape sequence in string literal at position 14`

## TC-F0001-009 — Error: `#` followed by a space
Given:
- filter expression: `comment == "lone # in middle"`

When:
- the expression is parsed

Then:
- parsing fails
- error code: `InvalidEscapeSequence`
- the message contains `Invalid escape sequence in string literal at position 18`

## TC-F0001-010 — Error: trailing `#` consumes the closing quote
Given:
- filter expression: `comment == "hello #"`

When:
- the expression is parsed

Then:
- parsing fails: the `#"` sequence is interpreted as an escaped quote, so
  the string literal is left unterminated
- error code: `UnclosedQuote` (existing variant — **not** the new
  `InvalidEscapeSequence`)

## TC-F0001-011 — Error: bare `%` outside `LIKE`
Given:
- filter expression: `name == "100%"`

When:
- the expression is parsed

Then:
- parsing fails
- error code: `InvalidEscapeSequence`
- the user must write `name == "100#%"` to express a literal `%`

## TC-F0001-012 — Regression: `%` wildcard in a `LIKE`
Given:
- filter expression: `name LIKE "den%"`

When:
- the expression is parsed

Then:
- parsing succeeds (the `%` remains a `LIKE` wildcard)
- canonical form: `([name<LIKE>den%])`

## TC-F0001-013 — Literal `#%` inside a `LIKE`
Given:
- filter expression: `code LIKE "50#%"`

When:
- the expression is parsed

Then:
- parsing succeeds
- the literal value is `50%` and stands for a literal `%`, not a wildcard

## TC-F0001-014 — Preserved historical error: `\` is not an escape
Given:
- filter expression: `comment == "she said \"hi\""`

When:
- the expression is parsed

Then:
- parsing fails (the `\` has no special status — the first `"` after `\`
  closes the string and the rest is invalid)
- guards against any regression toward the old `\` escape proposal

- **Error message wording**: F0001 specifies the message
  `Invalid escape sequence in string literal at position {char_position}`,
  matching the existing `human_error_message` style (English sentence ending
  with `at position {char_position}`, no `{}` placeholder for the offending
  character). The test cases above only assert the substring
  `Invalid escape sequence in string literal` to keep the position-suffix
  formatting flexible.

# Coverage vs F0001

| F0001 rule                                                | Test cases                       |
|-----------------------------------------------------------|----------------------------------|
| `#"` → `"`                                                | TC-F0001-001, 004, 005           |
| `#%` → `%`                                                | TC-F0001-002, 004, 005, 013      |
| `##` → `#`                                                | TC-F0001-003, 004                |
| `#` not followed by `#`/`%`/`"` is an error               | TC-F0001-007, 008, 009           |
| Trailing `#` → unterminated string                        | TC-F0001-010                     |
| Bare forbidden char (`%` outside `LIKE`)                  | TC-F0001-011                     |
| Regression (plain string, `LIKE` wildcard, `\`)           | TC-F0001-006, 012, 014           |
