# Document Search Condition to SQL

This note documents the `document-server` feature that converts a document search condition expression into a SQL query.

## Where it starts

The request entry point is [`document-server/src/item.rs`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\item.rs). `ItemDelegate::search_item`:

- parses the incoming `filter_expression` with `analyse_expression`
- resolves tag definitions for all tags used in the filter and `ORDER BY`
- generates SQL with `generate_search_sql`
- executes the generated SQL against the customer schema

Relevant code:

- [`document-server/src/item.rs:55`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\item.rs:55)
- [`document-server/src/item.rs:77`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\item.rs:77)
- [`document-server/src/item.rs:97`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\item.rs:97)

## High-level pipeline

The feature is split into two subsystems:

1. Filter analysis in [`document-server/src/filter`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter)
2. SQL generation in [`document-server/src/engine/generator.rs`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs)

The effective flow is:

1. `lex3(expression)` tokenizes the expression.
2. `normalize_lexeme(tokens)` rewrites the token stream into a normalized binary form.
3. `parse_tokens(tokens)` builds an AST made of conditions and binary logical nodes.
4. `generate_search_sql(ast, ...)` extracts terminal conditions, validates tag types and operators, generates one SQL join per terminal condition, then rebuilds the boolean expression as `...value is not null`.

Relevant code:

- [`document-server/src/filter/mod.rs:18`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\mod.rs:18)
- [`document-server/src/filter/filter_lexer.rs:270`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:270)
- [`document-server/src/filter/filter_normalizer.rs:9`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_normalizer.rs:9)
- [`document-server/src/filter/filter_ast.rs:102`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:102)
- [`document-server/src/engine/generator.rs:325`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:325)

## Expression language

### Supported values

- integer values
- quoted string values
- boolean values `TRUE` and `FALSE`

Booleans are recognized by the lexer as uppercase keywords:

- [`document-server/src/filter/filter_lexer.rs:237`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:237)
- [`document-server/src/filter/filter_lexer.rs:238`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:238)

### Supported comparison operators

The parser supports:

- `==`
- `!=`
- `>`
- `>=`
- `=>`
- `<`
- `<=`
- `=<`
- `LIKE`

Notes:

- `=>` is normalized as `>=`
- `=<` is normalized as `<=`

Relevant code:

- [`document-server/src/filter/filter_lexer.rs:243`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:243)
- [`document-server/src/filter/filter_lexer.rs:246`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:246)
- [`document-server/src/filter/filter_lexer.rs:248`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:248)
- [`document-server/src/filter/filter_ast.rs:20`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:20)

### Supported logical operators

- `AND`
- `OR`

These are always converted to a strictly binary tree during normalization.

Relevant code:

- [`document-server/src/filter/filter_lexer.rs:240`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:240)
- [`document-server/src/filter/filter_lexer.rs:241`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:241)
- [`document-server/src/filter/filter_normalizer.rs:19`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_normalizer.rs:19)

### Informal grammar

The effective grammar is:

```text
expression := condition
            | "(" expression logical_operator expression ")"

condition  := attribute comparison_operator value
```

Normalization makes the input more permissive than the AST grammar:

- a simple condition does not need explicit parentheses
- conditions inside larger expressions are wrapped automatically
- chained logical expressions are regrouped into binary nodes
- `AND` has higher precedence than `OR`

Example:

```text
country == "FR" AND science >= 40 OR is_open == TRUE
```

is normalized conceptually as:

```text
((country == "FR" AND science >= 40) OR is_open == TRUE)
```

Relevant code:

- [`document-server/src/filter/filter_normalizer.rs:20`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_normalizer.rs:20)
- [`document-server/src/filter/filter_normalizer.rs:264`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_normalizer.rs:264)
- [`document-server/src/filter/filter_ast.rs:125`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:125)
- [`document-server/src/filter/filter_ast.rs:193`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:193)

## AST model

The parser builds an AST with only two node types:

- `Condition(FilterCondition)`
- `Logical { operator, leaves }`

Each `FilterCondition` receives a generated unique `key`. That key is important later because SQL generation maps each leaf condition to a dedicated join alias, even when multiple leaves target the same tag.

Relevant code:

- [`document-server/src/filter/filter_ast.rs:51`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:51)
- [`document-server/src/filter/filter_ast.rs:58`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:58)
- [`document-server/src/filter/filter_ast.rs:93`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:93)

## SQL generation strategy

### Why the SQL is join-based

The generator does not translate the boolean expression directly into predicates on a single `tag_value` table alias.

Instead it:

1. extracts all terminal conditions from the AST
2. assigns an occurrence index per tag name
3. creates one `LEFT OUTER JOIN` subquery per terminal condition
4. rebuilds the original boolean expression using `ot_<tag>_<occurrence>.value is not null`

This is what allows expressions such as:

```text
country == "US" OR country == "FR"
```

to work correctly. Each condition gets its own alias, for example `ot_country_0` and `ot_country_1`.

Relevant code:

- [`document-server/src/engine/generator.rs:56`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:56)
- [`document-server/src/engine/generator.rs:77`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:77)
- [`document-server/src/engine/generator.rs:96`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:96)
- [`document-server/src/engine/generator.rs:473`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:473)

### Per-condition SQL fragment

Each terminal condition becomes a derived-table join with this structure:

```sql
LEFT OUTER JOIN (
    SELECT tv.item_id, tv.<value_column> AS value
    FROM cs_<customer>.tag_definition td
    JOIN cs_<customer>.tag_value tv ON
        tv.tag_id = td.id
        AND td."name" = '<tag_name>'
        AND <typed_value_filter>
) ot_<tag_name>_<occurrence> ON ot_<tag_name>_<occurrence>.item_id = i.id
```

Then the boolean filter becomes something like:

```sql
(ot_country_0.value is not null OR (ot_postal_code_0.value is not null AND ot_country_1.value is not null))
```

Relevant code:

- [`document-server/src/engine/generator.rs:200`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:200)
- [`document-server/src/engine/generator.rs:482`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:482)

### Final query shape

The generated query always starts from the item table:

```sql
SELECT i.id, name, file_ref, created_gmt, last_modified_gmt
FROM cs_<customer>.item i
<one left join per terminal condition>
WHERE <rebuilt boolean filter>
ORDER BY <derived order columns>
```

Relevant code:

- [`document-server/src/engine/generator.rs:429`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:429)

## Type-aware filtering

Before SQL is emitted, the generator loads tag definitions and validates each operator against the tag type.

Currently supported operator sets are:

- `Bool`: `EQ`, `NEQ`
- `Int`: `EQ`, `NEQ`, `GT`, `GTE`, `LT`, `LTE`
- `Double`: `EQ`, `NEQ`, `GT`, `GTE`, `LT`, `LTE`
- `Text`: `EQ`, `NEQ`, `LIKE`

Relevant code:

- [`document-server/src/engine/generator.rs:16`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:16)
- [`document-server/src/engine/generator.rs:248`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:248)

### SQL emitted per tag type

- `Text`
  - Uses `unaccent_lower((tv.value_string)::text) <op> unaccent_lower('<value>')`
  - This makes string matching case-insensitive and accent-insensitive.
- `Int`
  - Uses `tv.value_integer <op> <value>`
- `Double`
  - Uses `tv.value_double <op> <value>`
- `Bool`
  - Special-cased:
    - `== TRUE` becomes `tv.value_boolean`
    - all other bool cases currently become `NOT tv.value_boolean`

Relevant code:

- [`document-server/src/engine/generator.rs:200`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:200)

## Ordering

`ORDER BY` columns are derived from the same join aliases used by the filter.

If the same tag appears multiple times, the generator nests `COALESCE(...)` across all occurrences for that tag so ordering can still use a single expression.

Relevant code:

- [`document-server/src/engine/generator.rs:272`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:272)

## Example

Input expression:

```text
lastname LIKE "%ab%" OR (postal_code == 30099 AND lastname LIKE "%h%")
```

The generator will:

1. build three leaf conditions
2. assign two occurrences to `lastname`
3. emit three `LEFT OUTER JOIN` blocks
4. produce a `WHERE` clause equivalent to:

```sql
(ot_lastname_0.value is not null OR (ot_postal_code_0.value is not null AND ot_lastname_1.value is not null))
```

There is already a test for this case:

- [`document-server/src/engine/generator.rs:671`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:671)

## Current limitations and caveats

- Only `Text`, `Bool`, `Int`, and `Double` are implemented in `build_tag_value_filter`.
- `Date`, `DateTime`, and `Link` are present in `TagType` but still `todo!()` in SQL generation.
- Bool SQL generation is asymmetric: `== TRUE` maps to `tv.value_boolean`, while every other bool case maps to `NOT tv.value_boolean`. That means `!= TRUE`, `== FALSE`, and `!= FALSE` are not distinguished yet.
- Text values are interpolated directly into the generated SQL string in this layer. This deserves review if the expression can contain untrusted content.
- The generated SQL is assembled as a plain string and then executed, not as a prepared statement with bound filter values.
- The `Persisted` generation mode exists but the "super filter" branch is not implemented yet.

Relevant code:

- [`document-server/src/engine/generator.rs:220`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:220)
- [`document-server/src/engine/generator.rs:421`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:421)

## Useful tests

The existing tests are the quickest way to understand intended behavior:

- lexer examples:
  - [`document-server/src/filter/filter_lexer.rs:931`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_lexer.rs:931)
- normalization and precedence:
  - [`document-server/src/filter/filter_normalizer.rs:490`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_normalizer.rs:490)
  - [`document-server/src/filter/filter_normalizer.rs:704`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_normalizer.rs:704)
- AST parsing:
  - [`document-server/src/filter/filter_ast.rs:323`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\filter\filter_ast.rs:323)
- SQL generation:
  - [`document-server/src/engine/generator.rs:628`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:628)
  - [`document-server/src/engine/generator.rs:671`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:671)
  - [`document-server/src/engine/generator.rs:757`](C:\Users\gcres\Projects\wks-doka-one\doka.one\document-server\src\engine\generator.rs:757)
