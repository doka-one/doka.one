# Doka filter syntax

The filter language is used to select items in `doka-cli item search -f "<filter>"`
and in the underlying `document-server` search API. A filter is a boolean
expression made of **conditions** — `attribute operator value` — combined with
`AND` / `OR` and grouped with parentheses.

> Part 2 (query building, CLI usage, error troubleshooting) will follow — see §9.

## Part 1 — Pure definition

### 1. Data types

A value in a condition is always one of three types.

| Type    | Literal syntax                 | Examples                              |
|---------|--------------------------------|---------------------------------------|
| Integer | bare decimal digits            | `10`, `40`, `2001`                    |
| String  | double-quoted text             | `"bonjour"`, `"FR"`, `"2001-01-01"`   |
| Boolean | `TRUE` / `FALSE` (uppercase)   | `TRUE`, `FALSE`                       |

Notes:

- There is no `NULL` / missing value — every condition must supply a value.
- Dates are written as strings in ISO 8601 form (`"YYYY-MM-DD"`) and compared
  lexicographically. There is no dedicated date type.
- Today, a literal `"`, `%` or `\` cannot appear inside a quoted value. A
  backslash escape (`\"`, `\%`, `\\`) is planned — see §6.

### 2. Operators by type

Seven comparison operators are recognised. The matrix below shows which value
types each one accepts.

| Operator | Aliases | Integer | String | Boolean | Meaning            |
|----------|---------|:-------:|:------:|:-------:|--------------------|
| `==`     |         |   ✔     |   ✔    |   ✔     | equal              |
| `!=`     |         |   ✔     |   ✔    |   ✔     | not equal          |
| `>`      |         |   ✔     |   ✔    |         | greater than       |
| `>=`     | `=>`    |   ✔     |   ✔    |         | greater or equal   |
| `<`      |         |   ✔     |   ✔    |         | less than          |
| `<=`     | `=<`    |   ✔     |   ✔    |         | less or equal      |
| `LIKE`   |         |         |   ✔    |         | pattern match (`%`)|

- A single `=` is **not** valid — use `==`.
- `LIKE` is the **only** operator that interprets meta-characters in its value.
- Order comparisons (`>`, `>=`, `<`, `<=`) on strings are lexicographic — this
  is what makes date-as-string comparisons work.

### 3. Logical operators and grouping

- `AND` and `OR` are the only logical operators. They are case-insensitive
  keywords (`AND`, `and`, `And` all parse).
- There is **no `NOT` operator**. Express negation by choosing the opposite
  comparison (typically `!=`).
- Parentheses `( ... )` group logical expressions and may be nested freely.
- **Precedence:** `AND` binds tighter than `OR`. The expression

  ```
  age < 40 OR age > 21 AND detail == "bonjour"
  ```

  is parsed as

  ```
  (age < 40) OR ( (age > 21) AND (detail == "bonjour") )
  ```

  Use explicit `()` whenever you want a different grouping.

### 4. Attribute names

Attribute names are made of letters, digits and underscores: `[a-zA-Z0-9_]`.
No spaces, no dashes, no dots. Valid examples: `age`, `attribut1`,
`birthdate`, `country`, `tag_name_1`.

Whitespace around the comparison operator is optional — `age<40`, `age < 40`
and `age <40` are all accepted.

### 5. Meta-characters

| Char    | Role                                                                    |
|---------|-------------------------------------------------------------------------|
| `"`     | Delimits a string literal. Required for every string value, including dates. No escape syntax. |
| `%`     | Wildcard — **only meaningful inside a string used with `LIKE`**. Matches zero or more characters. Example: `name LIKE "den%"`. Outside a `LIKE` value it is just a literal `%`. |
| `(` `)` | Logical grouping (see §3).                                              |

### 6. Escaping in text constants (planned — not yet implemented)

> **Status:** specified but not yet implemented in the parser. The current
> lexer does **not** recognise `\` as an escape character; until this is
> released, the rules below are forward-looking and the characters listed
> simply cannot appear in a string value.

Inside a string constant `"..."`, three characters will be forbidden as
literals because they collide with the filter syntax or with the SQL
wildcard. They must be escaped with a leading backslash:

| Character | Why it's forbidden            | Escape sequence |
|-----------|-------------------------------|-----------------|
| `"`       | Ends the string literal       | `\"`            |
| `%`       | `LIKE` wildcard               | `\%`            |
| `\`       | The escape character itself   | `\\`            |

Examples (future syntax):

- `comment == "she said \"hi\""` — embedded double quote.
- `code LIKE "50\%"` — literal `%` inside a `LIKE` pattern (does *not* act
  as a wildcard).
- `path == "C:\\Users\\denis"` — literal backslashes.

Any other character following `\` will be an error.

### 7. Parenthesis combinations — worked examples

The right-hand column shows the canonical form the server prints for the same
input (see §8). The examples come straight from the parser test suite, so the
shapes shown are exact.

| Input                                                                 | Canonical form                                                                                  |
|-----------------------------------------------------------------------|-------------------------------------------------------------------------------------------------|
| `(age < 40)`                                                          | `([age<LT>40])`                                                                                 |
| `age < 40 AND (birthdate >= "2001-01-01")`                            | `([age<LT>40]AND[birthdate<GTE>2001-01-01])`                                                    |
| `(age < 40) OR (question == TRUE)`                                    | `([age<LT>40]OR[question<EQ>TRUE])`                                                             |
| `age < 40 OR age > 21 AND detail == "bonjour"`                        | `(([age<LT>40]OR[age<GT>21])AND[detail<EQ>bonjour])` *(implicit precedence)*                    |
| `(attribut3 LIKE "den%")`                                             | `([attribut3<LIKE>den%])`                                                                       |
| `((country == "FR" AND (science >= 40)) OR (lost_in_hell == "TRUE"))` | `(([country<EQ>FR]AND[science<GTE>40])OR[lost_in_hell<EQ>TRUE])`                                |

### 8. Canonical / debug form

When the server echoes a filter back — in logs, traces or error messages — it
uses a normalised form, not your original string. Reading it is easy once you
know the conventions:

- A single condition is written `[ attr <OP> value ]`, where `<OP>` is one of
  `EQ`, `NEQ`, `GT`, `GTE`, `LT`, `LTE`, `LIKE`.
- Logical groupings are always **binary** after normalisation: `( X AND Y )` or
  `( X OR Y )`. If your original expression had three or more operands at the
  same level, the server inserts parentheses to make precedence explicit.
- String values appear **without** their surrounding quotes.
- Booleans appear as `TRUE` / `FALSE`.

This is the bridge between what you typed and what the server reports.

---

## Part 2 — Usage (to be written)

This part is a placeholder. The following points are known open questions and
must be specified before the section is filled in:

### 9. `doka-cli` filter syntax — still to be defined

How a filter string is passed on the command line is **not yet fully
specified**. Open points include:

- How the shell's own quoting (PowerShell vs. Bash vs. `cmd.exe`) interacts
  with the filter's double-quoted string constants — i.e. what the user must
  actually type around `-f "..."`.
- Whether the CLI accepts a filter from a file (`-f @path/to/filter.txt`) or
  from stdin, to avoid shell-quoting issues entirely.
- The exact error reporting the CLI surfaces when the parser rejects a
  filter (position, error code, hint).

### 10. CLI-level escaping — still to be defined

Independent of the parser-level escaping described in §6, the CLI needs its
own rules for getting `"`, `%`, `\` and shell meta-characters from the
keyboard into the filter string. Open points:

- Whether the CLI does any pre-processing of `-f` (e.g. strip outer quotes,
  unescape `\"`) or passes the argument through verbatim to the parser.
- Recommended cross-shell idioms for each supported shell.
- Interaction with the parser's planned `\` escape (§6) — do users have to
  double-escape on the command line?

Both items above need a design decision before this section can be written.
