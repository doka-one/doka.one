CREATE SCHEMA {customer_schema} AUTHORIZATION doka;

SET search_path = {customer_schema}, pg_catalog;

-- Drop table

-- DROP TABLE "document";

CREATE TABLE "document" (
	id bigserial NOT NULL,
	file_ref varchar(50) NOT NULL,
	part_no int4 NOT NULL,
	doc_text text NOT NULL,
	tsv tsvector NOT NULL,
	lang varchar(20) NOT NULL,
	CONSTRAINT document_file_ident_uk UNIQUE (file_ref, part_no),
	CONSTRAINT document_pk PRIMARY KEY (id)
);
CREATE INDEX document_file_ident_idx ON document USING btree (file_ref, part_no);
CREATE INDEX document_ftsv_idx ON document USING gin (tsv);
CREATE INDEX document_id_idx ON document USING btree (id);
CREATE INDEX document_language_idx ON document USING btree (lang);


-- item definition

-- Drop table

-- DROP TABLE item;

CREATE TABLE item (
	id bigserial NOT NULL,
	"name" varchar(255) NOT NULL,
	created_gmt timestamp(0) NOT NULL,
	last_modified_gmt timestamp(0) NOT NULL,
	file_ref varchar(50) NULL,
	CONSTRAINT item_pk PRIMARY KEY (id)
);
CREATE INDEX item_created_idx ON item USING btree (created_gmt);
CREATE UNIQUE INDEX item_file_ref_idx ON item USING btree (file_ref);
CREATE INDEX item_last_modified_idx ON item USING btree (last_modified_gmt);
CREATE INDEX item_name_btree_idx ON item USING btree (public.unaccent_lower((name)::text) COLLATE "C");
CREATE INDEX item_name_gin_idx ON item USING gin (public.unaccent_lower((name)::text) public.gin_trgm_ops);


-- preview definition

-- Drop table

-- DROP TABLE preview;

CREATE TABLE preview (
	id bigserial NOT NULL,
	file_reference_id int8 NOT NULL,
	file_identifier varchar(50) NOT NULL,
	sort_order int2 NOT NULL,
	CONSTRAINT prev_f_ident_and_f_ref_uk UNIQUE (file_identifier, file_reference_id),
	CONSTRAINT prev_f_ref_and_sort_uk UNIQUE (file_identifier, sort_order),
	CONSTRAINT prev_f_ref_uk UNIQUE (file_identifier),
	CONSTRAINT preview_pk PRIMARY KEY (id)
);
CREATE INDEX preview_file_ref_id_idx ON preview USING btree (file_reference_id);


-- tag_definition definition

-- Drop table

-- DROP TABLE tag_definition;

CREATE TABLE tag_definition (
	id bigserial NOT NULL,
	"name" varchar(25) NOT NULL,
	"type" varchar(25) NOT NULL,
	string_tag_length int4 NULL,
	default_value varchar(255) NULL,
	CONSTRAINT length_limit CHECK (((string_tag_length >= 0) AND (string_tag_length <= 10000000))),
	CONSTRAINT tag_name_uk UNIQUE (name),
	CONSTRAINT tag_pk PRIMARY KEY (id)
);


-- tag_value definition

-- Drop table

-- DROP TABLE tag_value;

CREATE TABLE tag_value (
	id bigserial NOT NULL,
	tag_id int8 NOT NULL,
	item_id int8 NOT NULL,
	value_string varchar(2000) NULL,
	value_integer int8 NULL,
	value_double float8 NULL,
	value_date date NULL,
	value_datetime timestamp(0) NULL,
	value_boolean bool NULL,
	CONSTRAINT tag_value_pk PRIMARY KEY (id),
	CONSTRAINT fk_tag_value_item_id FOREIGN KEY (item_id) REFERENCES item(id)
);
CREATE INDEX tag_value_date_idx ON tag_value USING btree (value_date);
CREATE INDEX tag_value_datetime_idx ON tag_value USING btree (value_datetime);
CREATE INDEX tag_value_double_idx ON tag_value USING btree (value_double);
CREATE INDEX tag_value_integer_idx ON tag_value USING btree (value_integer);
CREATE INDEX tag_value_str_like_gin_idx ON tag_value USING gin (public.unaccent_lower((value_string)::text) public.gin_trgm_ops);
CREATE INDEX tag_value_str_sort_btree_idx ON tag_value USING btree (public.unaccent_lower((value_string)::text) COLLATE "C");

CREATE UNIQUE INDEX tag_value_tag_item_udx ON tag_value  USING btree (tag_id, item_id);

CREATE TABLE compiled_query (
	id bigserial NOT NULL,
	query_name varchar(255) NOT NULL,
	description varchar(1000) NOT NULL,
	filters text NOT NULL,
	order_tag varchar(25) NULL,
	extra_properties_json text NULL,
	compiled_sql text NOT NULL,
	matching_item_count int8 NULL,
	created_gmt timestamp(0) NOT NULL,
	last_modified_gmt timestamp(0) NOT NULL,
	CONSTRAINT compiled_query_pk PRIMARY KEY (id),
	CONSTRAINT compiled_query_name_uk UNIQUE (query_name)
);

CREATE TABLE compiled_query_condition (
	id bigserial NOT NULL,
	compiled_query_id int8 NOT NULL,
	condition_key varchar(64) NOT NULL,
	attribute_name varchar(25) NOT NULL,
	operator varchar(20) NOT NULL,
	value_text varchar(2000) NOT NULL,
	occurrence int4 NOT NULL,
	count_sql text NOT NULL,
	matching_item_count int8 NULL,
	is_ic bool NULL,
	superfilter_sql text NULL,
	created_gmt timestamp(0) NOT NULL,
	last_modified_gmt timestamp(0) NOT NULL,
	CONSTRAINT compiled_query_condition_pk PRIMARY KEY (id),
	CONSTRAINT compiled_query_condition_fk FOREIGN KEY (compiled_query_id) REFERENCES compiled_query(id),
	CONSTRAINT compiled_query_condition_key_uk UNIQUE (compiled_query_id, condition_key),
	CONSTRAINT compiled_query_condition_occ_uk UNIQUE (compiled_query_id, attribute_name, occurrence)
);
CREATE INDEX compiled_query_condition_query_idx ON compiled_query_condition USING btree (compiled_query_id);


CREATE OR REPLACE PROCEDURE insert_document(file_ref character varying, part_no integer, doc_text character varying, tsv character varying, lang character varying)
 LANGUAGE sql
AS $procedure$
   INSERT INTO {customer_schema}.document  ( FILE_REF,  PART_NO, DOC_TEXT, TSV, LANG )
        VALUES ( FILE_REF, PART_NO, DOC_TEXT,
				TSV :: TSVECTOR
				,  LANG );
$procedure$
;
