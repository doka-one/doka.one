
pub (crate) const FS_SCHEMA: &str =  r#"

CREATE SCHEMA {customer_schema}
       AUTHORIZATION postgres;

SET search_path = {customer_schema}, pg_catalog;

CREATE TABLE file_reference (
	id bigserial NOT NULL,
	file_ref varchar(50) NOT NULL,
	mime_type varchar(256) NULL,
	checksum varchar(64) NULL,
	original_file_size int8 NULL,
	encrypted_file_size int8 NULL,
	total_part int4 NULL,
	is_fulltext_parsed bool NULL,
	is_preview_generated bool NULL,
	CONSTRAINT file_reference_pk PRIMARY KEY (id),
	CONSTRAINT file_reference_uk UNIQUE (file_ref)
);


CREATE TABLE file_parts (
	id bigserial NOT NULL,
	file_reference_id int8 NOT NULL,
	part_number int4 NOT NULL,
	is_encrypted bool NOT NULL,
	part_data text NULL,
	CONSTRAINT file_parts_pkey PRIMARY KEY (id),
	CONSTRAINT file_reference_id_fk FOREIGN KEY (file_reference_id) REFERENCES file_reference(id)
);
CREATE UNIQUE INDEX ref_part_udx ON file_parts USING btree (file_reference_id, part_number);


    "#;