CREATE SCHEMA keymanager AUTHORIZATION postgres;

CREATE TABLE keymanager.customer_keys (
	id bigserial NOT NULL,
	customer_code varchar(100) NOT NULL,
	ciphered_key varchar(200) NOT NULL,
	CONSTRAINT customer_keys_pkey PRIMARY KEY (id)
);
CREATE UNIQUE INDEX idx_customer_keys_code ON keymanager.customer_keys USING btree (customer_code);
