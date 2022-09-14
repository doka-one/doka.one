pub (crate) const SCHEMA_DOKAADMIN : &str = r#"
CREATE SCHEMA dokaadmin
    AUTHORIZATION postgres;

SET statement_timeout = 0;
SET lock_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET client_min_messages = warning;

--
-- Name: plpgsql; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS plpgsql WITH SCHEMA pg_catalog;


--
-- Name: EXTENSION plpgsql; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION plpgsql IS 'PL/pgSQL procedural language';


SET search_path = dokaadmin, pg_catalog;

SET default_with_oids = false;


--
-- TOC entry 175 (class 1259 OID 569934)
-- Name: customer; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE dokaadmin.customer (
    id bigserial,
    code character varying(255) NOT NULL, -- Ex : 2fa6a8d8 , used for key manager and database schema
    full_name character varying(255),
    default_language character(3) NOT NULL,
    default_time_zone character varying(50) NOT NULL,
    is_removable boolean NOT NULL DEFAULT false
);

ALTER TABLE ONLY dokaadmin.customer
    ADD CONSTRAINT customer_pk PRIMARY KEY (id);


ALTER TABLE ONLY dokaadmin.customer
    ADD CONSTRAINT CUSTOMER_NAME_UK UNIQUE ( code );

--
--

CREATE TABLE dokaadmin.appuser (
    id bigserial NOT NULL,
    login character varying(255) NOT NULL, -- email address of the user
    full_name character varying(255),
    password_hash character varying(255) NOT NULL,
    default_language character(3),
    default_time_zone character varying(50),
    admin boolean NOT NULL,
    customer_id bigint NOT NULL
);


ALTER TABLE ONLY dokaadmin.appuser
    ADD CONSTRAINT appuser_pk PRIMARY KEY (id);

-- The login must be absolutely unique (email address of the user)
ALTER TABLE ONLY dokaadmin.appuser
    ADD CONSTRAINT appuser_uk UNIQUE (login);
"#;