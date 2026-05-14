# Doka Main Search Requests

## Reference

- `REF_TAG`: `DOKA_SEARCH_SQL`
- Description: `Real query model`
- Environment: `cs_test_03`
- Database schema: `cs_93f71785`

## Solution

```sql
--- Solution 2

-- (active == true and ( my_email like %inc.com or title LIKE 'ba%' ))

SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_my_email.value  AS my_email,
       ot_title.value     AS title,
       ot_active.value    as active,
       ot_birthdate.value as birthdate
FROM item i
-- subquery
         LEFT OUTER JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_value tv
                          WHERE tv.tag_id = 1
                            AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%inc.com')) ot_my_email
                         ON ot_my_email.item_id = i.id
-- subquery
         LEFT OUTER JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'title' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ba%')) ot_title
                         ON ot_title.item_id = i.id
-- subquery
         LEFT OUTER JOIN (SELECT tv.item_id, tv.value_boolean as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true
                              -- super filter
                              and tv.item_id in (
                              SELECT tv.item_id
                              FROM tag_definition td
                              JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'my_email' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%inc.com')
                              union
                              SELECT tv.item_id
                              FROM tag_definition td
                              JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'title' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ba%')
                              )) ot_active ON ot_active.item_id = i.id
-- subquery
         LEFT OUTER join (SELECT tv.item_id, tv.value_date as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'birthdate') ot_birthdate ON ot_birthdate.item_id = i.id
WHERE ot_active.value = true
  and (ot_my_email.value is not null or ot_title.value is not null) -- composable filter
ORDER BY ot_my_email.value
offset 0 limit 10;
```

## IC and NIC

In this document, `IC/NIC` means:

- `IC`: `Indexed Condition`
- `NIC`: `Non Indexed Condition`

The idea is not whether a condition is true or false. It is whether the condition can be used as an efficient database access/filtering driver.

In practice, in Doka, a condition is more likely to be an `IC` when it can reduce the working set early and efficiently, for example:

- `postal_code = 30099`
- `active = true`
- sometimes `LIKE 'ab%'` when an appropriate index exists and the predicate is selective enough

A condition is more likely to be a `NIC` when it does not help much as a driving filter, for example:

- `LIKE '%h%'`
- a weakly selective condition
- a condition on a type or column without a useful supporting index
- a condition that still forces a large scan of `tag_value`

Why this distinction matters in the algorithm:

- the engine must build a logically correct query
- but it also wants to optimize execution by injecting some conditions as `superfilters`
- only some conditions are good candidates for this
- in general, `IC`s are worth injecting, while `NIC`s often cost more than they save

Simple example:

```text
active = true AND lastname LIKE 'a%'
```

If `active = true` is very selective, we may consider:

- `active = true` as an `IC`
- `lastname LIKE 'a%'` as a weaker `IC`, or even a `NIC`

Then the `lastname` subquery may be written as:

```sql
... lastname LIKE 'a%'
AND item_id IN ( ... active = true ... )
```

This is exactly the point of a `superfilter`:

- inject a strong condition into another subquery
- reduce the amount of work done there

Another example:

```text
lastname LIKE 'ab%' OR (postal_code = 30099 AND lastname LIKE '%h%')
```

In the right branch:

- `postal_code = 30099` is a strong `IC`
- `lastname LIKE '%h%'` is more likely a `NIC`

So the engine may choose to restrict the `%h%` branch with:

```sql
item_id IN ( ... postal_code = 30099 ... )
```

This is why the design note says:

- `search for the IC we can inject through an AND`
- `if the condition is not indexed, it's not productive to inject it as a super filter`

In simpler words:

- `IC` = a condition that is useful to cut down the data set early
- `NIC` = a condition that is logically valid, but not a good optimization driver
- the algorithm wants to detect `IC`s so they can be propagated into subqueries when this is semantically safe

Important note:

- in this design note, `IC/NIC` is an optimization concept
- it is not yet a fully formalized classification implemented in the current Rust generator
- today, the Rust code mostly focuses on generating the correct logic first; systematic `IC/NIC`-driven `superfilter` injection is still a design direction

## Algorithm Notes

- Parse the input conditions in an AST; implement the `AND` precedence over `OR`.
- Identify the indexed conditions `(IC)` and not indexed conditions `(NIC)`.
- Search for the IC we can inject through an `AND`.
- Create a subquery for each condition with injected super filters.
- Add the composable filter to the main query.
- Generate the final PostgreSQL SQL.

Notes:

1. There is the IC/NIC concept but also the Filtering or not filtering conditions and the Injection into the ANDs can
   help.
2. The fact we plan to store the Doka Queries could allow us to estimate the NIC/NF of each condition and then to
   pre-compute the SQL query.
3. This algo gives an efficient solution to retrieve the fields explicitly mentioned in the filter or the order, but it
   does not bring all the tag values of each item.

## Exploration Queries

```sql
SELECT count(*)
FROM tag_definition td
         JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099 OR
                              tv.value_integer = 30098;

select *
from tag_definition;

SELECT count(*) as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'my_email' AND tv.value_string LIKE '%inc.com';

SELECT count(*)
FROM tag_definition td
         JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true;

SELECT count(*)
FROM tag_definition td
         JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'birthdate';

-- Création de l'index sur la colonne "name" de la table "tag_definition"
CREATE INDEX idx_tag_definition_name ON tag_definition ("name");

-- Création de l'index sur les colonnes "tag_id" et "value_string" de la table "tag_value"
CREATE INDEX idx_tag_value_tag_id_value_string ON tag_value (tag_id, value_string);

SELECT tv.item_id, tv.value_string as value
FROM tag_value tv
where tv.tag_id = (
    select id
    FROM tag_definition td
    where td.name = 'my_email'
    )
  AND tv.value_string LIKE 'me@inc.com';

select id
FROM tag_definition td
where td.name = 'title';

select tv.item_id, tv.value_string as value
FROM tag_value tv
WHERE tv.tag_id = 1 AND tv.value_string like 'me@inc.%';

SELECT *
FROM tag_value
WHERE unaccent_lower((value_string)::text) LIKE unaccent_lower('%inc.com');

select tv.item_id, tv.value_string as value
FROM tag_value tv
WHERE tv.value_string like 'me@inc.%';

select tv.item_id, tv.value_string as value
FROM tag_value tv
WHERE tv.tag_id = 7
  AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ba%');

SELECT *
FROM tag_value
WHERE unaccent_lower((value_string)::text) LIKE unaccent_lower('ba%');
```

## CASE 1

CASE 1 `(active == true AND postal_code = 30500)`

Count queries:

```sql
SELECT count(*) as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099;

SELECT count(*) as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'active' AND tv.value_boolean = true;
```

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value    AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value      as active
FROM item i
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' /*AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('abb%')   */
) ot_lastname ON ot_lastname.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099) ot_postal_code
                         ON ot_postal_code.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_boolean as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true) ot_active
                         ON ot_active.item_id = i.id
where ot_active.value is not null
  and ot_postal_code.value is not null
ORDER BY ot_lastname.value
offset 0 limit 10;
```

## CASE 2

CASE 2 `(active == true AND lastname LIKE 'ao%')`

Count query:

```sql
SELECT count(*) as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ao%');
```

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value    AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value      as active
FROM item i
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ao%')) ot_lastname
                         ON ot_lastname.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'postal_code') ot_postal_code
                         ON ot_postal_code.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_boolean as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true) ot_active
                         ON ot_active.item_id = i.id
where ot_active.value is not null
  and ot_lastname.value is not null
ORDER BY ot_lastname.value
offset 0 limit 10;
```

## CASE 3

CASE 3 `(active == true AND lastname LIKE 'a%')`

Count query:

```sql
SELECT count(*) as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('a%');
```

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value    AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value      as active
FROM item i
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('a%')
                              and tv.item_id in (
                              SELECT tv.item_id
                              FROM tag_definition td
                              JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true
                              )) ot_lastname ON ot_lastname.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'postal_code') ot_postal_code
                         ON ot_postal_code.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_boolean as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'active' and tv.value_boolean = true) ot_active
                         ON ot_active.item_id = i.id
where ot_active.value is not null
  and ot_lastname.value is not null
ORDER BY ot_lastname.value
offset 10 limit 10;
```

## CASE 4

CASE 4 `lastname LIKE 'ab%'`

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value    AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value      as active
FROM item i
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')) ot_lastname
                         ON ot_lastname.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'postal_code') ot_postal_code
                         ON ot_postal_code.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_boolean as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'active') ot_active ON ot_active.item_id = i.id
where ot_lastname.value is not null
ORDER BY ot_lastname.value
offset 0 limit 20;
```

## CASE 5

CASE 5 `lastname LIKE 'h%' and postal_code = 30099`

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       ot_lastname.value    AS lastname,
       ot_postal_code.value AS postal_code,
       ot_active.value      as active
FROM item i
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('h%')) ot_lastname
                         ON ot_lastname.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099) ot_postal_code
                         ON ot_postal_code.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_boolean as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'active') ot_active ON ot_active.item_id = i.id
where ot_lastname.value is not null
  and ot_postal_code.value is not null
ORDER BY ot_lastname.value
offset 0 limit 50;
```

## CASE 6

CASE 6 `lastname LIKE 'ab%' OR  (postal_code = 30099 AND lastname LIKE 'h%')`

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       COALESCE(ot_lastname.value, ot_lastname_2.value) AS lastname3,
       ot_lastname.value                                AS lastname,
       ot_lastname_2.value                              as lastname2,
       ot_postal_code.value                             AS postal_code
FROM item i
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')) ot_lastname
                         ON ot_lastname.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099) ot_postal_code
                         ON ot_postal_code.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('h%')
    /* and tv.item_id in (
                  SELECT tv.item_id
                  FROM tag_definition td
                  JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099
     ) */ -- super filtre 0.7 --> 0.2
) ot_lastname_2 ON ot_lastname_2.item_id = i.id
where ot_lastname.value is not null
   or (ot_lastname_2.value is not null and ot_postal_code.value is not null)
ORDER BY COALESCE(ot_lastname.value, ot_lastname_2.value) DESC
offset 400 limit 50;
```

## CASE 7

CASE 7 `lastname LIKE "ab%" OR  (postal_code == 30099 AND lastname LIKE "%h%")`

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       COALESCE(ot_lastname.value, ot_lastname_2.value) AS lastname3,
       ot_lastname.value                                AS lastname,
       ot_lastname_2.value                              as lastname2,
       ot_postal_code.value                             AS postal_code
FROM cs_93f71785.item i
         LEFT OUTER JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM cs_93f71785.tag_definition td
                              JOIN cs_93f71785.tag_value tv
                          ON
                              tv.tag_id = td.id
                              AND td."name" = 'lastname'
                              AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')) ot_lastname
                         ON ot_lastname.item_id = i.id
         LEFT OUTER JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM cs_93f71785.tag_definition td
                              JOIN cs_93f71785.tag_value tv
                          ON
                              tv.tag_id = td.id
                              AND td."name" = 'postal_code'
                              AND tv.value_integer = 30099
                              AND tv.item_id IN (
                              SELECT tv.item_id
                              FROM cs_93f71785.tag_definition td
                              JOIN cs_93f71785.tag_value tv ON
                              tv.tag_id = td.id
                              AND td."name" = 'lastname'
                              AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')
                              )) ot_postal_code ON ot_postal_code.item_id = i.id
         LEFT OUTER JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM cs_93f71785.tag_definition td
                              JOIN cs_93f71785.tag_value tv
                          ON
                              tv.tag_id = td.id
                              AND td."name" = 'lastname'
                              AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')
                              AND tv.item_id IN (
                              SELECT tv.item_id
                              FROM cs_93f71785.tag_definition td
                              JOIN cs_93f71785.tag_value tv ON tv.tag_id = td.id AND td."name" = 'postal_code' AND tv.value_integer = 30099
                              ) -- super filtre 4s --> 0.2
) ot_lastname_2 ON ot_lastname_2.item_id = i.id
WHERE ot_lastname.value IS NOT NULL
   OR (ot_lastname_2.value IS NOT NULL AND ot_postal_code.value IS NOT NULL)
ORDER BY COALESCE(ot_lastname.value, ot_lastname_2.value) DESC
OFFSET 400 LIMIT 500;
```

## CASE 8

CASE 8
`lastname LIKE 'ab%' AND  (postal_code = 30099 OR lastname LIKE '%h%') : : 0.038 s`

Note : il n'est intéressant de distribuer la condition 1 : (lastname LIKE 'ab%' AND postal_code = 30099) OR (lastname
LIKE 'ab%' AND lastname LIKE '%h%') : 0.366 s`

Count queries:

```sql
SELECT count(*) as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%');

SELECT count(*) as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%');

SELECT tv.item_id, tv.value_string as value
FROM tag_definition td
    JOIN tag_value tv
ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%') AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
    and tv.item_id in (
    SELECT tv.item_id
    FROM tag_definition td
    JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
    ) -- super filtre 4s --> 0.2
```

Main query:

```sql
SELECT i.id,
       name,
       file_ref,
       created_gmt,
       last_modified_gmt,
       COALESCE(ot_lastname_ab.value, ot_lastname_h.value) AS lastname,
       ot_lastname_ab.value                                AS lastname_ab,
       ot_lastname_h.value                                 as lastname_h,
       ot_postal_code.value                                AS postal_code
FROM item i
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')) ot_lastname_ab
                         ON ot_lastname_ab.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_integer as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'postal_code' and tv.value_integer = 30099
                              and tv.item_id in (
                              SELECT tv.item_id
                              FROM tag_definition td
                              JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
                              ) -- super filtre 4s --> 0.2
) ot_postal_code ON ot_postal_code.item_id = i.id
         left outer JOIN (SELECT tv.item_id, tv.value_string as value
                          FROM tag_definition td
                              JOIN tag_value tv
                          ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('%h%')
                              and tv.item_id in (
                              SELECT tv.item_id
                              FROM tag_definition td
                              JOIN tag_value tv ON tv.tag_id = td.id AND td."name" = 'lastname' AND unaccent_lower((tv.value_string)::text) LIKE unaccent_lower('ab%')
                              ) -- super filtre 4s --> 0.2
) ot_lastname_h ON ot_lastname_h.item_id = i.id
-- lastname LIKE 'ab%' AND  (postal_code = 30099 OR lastname LIKE '%h%')
where (ot_lastname_ab.value is not null)
  and (ot_lastname_h.value is not null or ot_postal_code.value is not null)
ORDER BY COALESCE(ot_lastname_ab.value, ot_lastname_h.value) DESC
offset 400 limit 10;
```

## Bulk Data Generator

The bulk data setup and generators were extracted to:

- [Routines for Bulk Data.md](/home/denis/Projects/wks-doka-one/doka.one/document-server/Routines%20for%20Bulk%20Data.md)
