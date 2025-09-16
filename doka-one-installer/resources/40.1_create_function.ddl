-- Must be executed with the doka user on every single database we will create (ex: ad_test_02)

CREATE EXTENSION UNACCENT;
CREATE EXTENSION pg_trgm;

ALTER TEXT SEARCH DICTIONARY unaccent (RULES='unaccent_default');

