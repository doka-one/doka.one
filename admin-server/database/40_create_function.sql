-- Must be executed with the doka user on every single database we will create (ex: ad_test_02)

CREATE EXTENSION UNACCENT;
CREATE EXTENSION pg_trgm;

ALTER TEXT SEARCH DICTIONARY unaccent (RULES='unaccent_default');

CREATE OR REPLACE FUNCTION public.unaccent_lower(text)
RETURNS text AS
$$
 SELECT CASE
        WHEN $1 IS NULL OR $1 = ''
         THEN NULL
        ELSE lower(unaccent('unaccent', $1))
        END;
$$
LANGUAGE SQL IMMUTABLE SET search_path = public, pg_temp;
