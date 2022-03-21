

CREATE SCHEMA dokasys
    AUTHORIZATION postgres;

DROP TABLE IF EXISTS dokasys.sessions CASCADE;

CREATE TABLE dokasys.sessions
(
    id SERIAL,
    customer_code varchar(100) NOT NULL,
    customer_id bigint NOT NULL,
    user_name varchar(100) NOT NULL,
    user_id bigint NOT NULL,
    session_id varchar(200) NOT NULL,
    start_time_gmt timestamp NOT NULL,
    PRIMARY KEY (id)
)
    WITH (
        OIDS = FALSE
        )
    TABLESPACE pg_default;

ALTER TABLE dokasys.sessions
    OWNER to denis;


CREATE INDEX idx_customer_code
    ON dokasys.sessions USING btree (customer_code ASC NULLS LAST, user_name ASC NULLS LAST, start_time_gmt DESC)
    TABLESPACE pg_default;


CREATE UNIQUE INDEX idx_session_id
    ON dokasys.sessions USING btree (session_id)
    TABLESPACE pg_default;


ALTER TABLE dokasys.sessions ADD COLUMN renew_time_gmt timestamp;

ALTER TABLE dokasys.sessions ADD COLUMN termination_time_gmt timestamp;