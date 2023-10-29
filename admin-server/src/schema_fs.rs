
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
	is_encrypted bool NOT NULL,
	is_fulltext_parsed bool NULL,
	is_preview_generated bool NULL,
	CONSTRAINT file_reference_pk PRIMARY KEY (id),
	CONSTRAINT file_reference_uk UNIQUE (file_ref)
);

CREATE TABLE file_parts (
	id bigserial NOT NULL,
	file_reference_id int8 NOT NULL,
	part_number int4 NOT NULL,
	part_data text NULL,
	CONSTRAINT file_parts_pkey PRIMARY KEY (id),
	CONSTRAINT file_reference_id_fk FOREIGN KEY (file_reference_id) REFERENCES file_reference(id)
);
CREATE UNIQUE INDEX ref_part_udx ON file_parts USING btree (file_reference_id, part_number);

CREATE TABLE file_uploads (
    session_id varchar(200) NOT NULL,
    start_time_gmt timestamp NOT NULL,
    user_id bigint NOT NULL,
    item_info varchar(50) NOT NULL,
	file_ref varchar(50) NOT NULL,
	part_number int4 NOT NULL,
	original_part_size int8 NULL,
	part_data text NULL
);
CREATE UNIQUE INDEX file_uploads_customer_user_idx ON file_uploads (file_ref, part_number);
CREATE INDEX file_uploads_start_time_idx ON file_uploads (start_time_gmt);

CREATE TABLE file_metadata (
	id bigserial NOT NULL,
	file_reference_id int8 NOT NULL,
	meta_key varchar(50) NOT NULL,
	value varchar(200) NULL,
	CONSTRAINT file_metadata_pkey PRIMARY KEY (id),
	CONSTRAINT file_metadata_id_fk FOREIGN KEY (file_reference_id) REFERENCES file_reference(id)
);
CREATE UNIQUE INDEX ref_meta_udx ON file_metadata USING btree (file_reference_id, meta_key);

    "#;
