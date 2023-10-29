CREATE TABLE connection_history (
	id bigserial NOT NULL,
	timer timestamp NOT NULL,
	description varchar(50) NOT NULL,
	status varchar(50) NULL,
	CONSTRAINT connection_history_pkey PRIMARY KEY (id)
);

-- Permissions

ALTER TABLE connection_history OWNER TO postgres;
GRANT ALL ON TABLE connection_history TO postgres;