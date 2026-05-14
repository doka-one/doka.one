
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
