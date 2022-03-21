--
-- PostgreSQL database dump
--

-- Dumped from database version 9.4.4
-- Dumped by pg_dump version 9.4.0
-- Started on 2015-07-21 19:51:39

SET statement_timeout = 0;
SET lock_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET client_min_messages = warning;

--
-- TOC entry 186 (class 3079 OID 11855)
-- Name: plpgsql; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS plpgsql WITH SCHEMA pg_catalog;


--
-- TOC entry 2117 (class 0 OID 0)
-- Dependencies: 186
-- Name: EXTENSION plpgsql; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION plpgsql IS 'PL/pgSQL procedural language';


SET search_path = public, pg_catalog;

SET default_with_oids = false;

--
-- TOC entry 172 (class 1259 OID 569925)
-- Name: action; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE action (
    id bigint NOT NULL,
    name character varying(255) NOT NULL
);


--
-- TOC entry 173 (class 1259 OID 569928)
-- Name: action_action_group_link; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE action_action_group_link (
    id bigint NOT NULL,
    action_id bigint NOT NULL,
    action_group_id bigint NOT NULL
);

ALTER TABLE action_action_group_link ADD CONSTRAINT action_action_group_link_pk PRIMARY KEY ( ID ) ;

--
-- TOC entry 174 (class 1259 OID 569931)
-- Name: action_group; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE action_group (
    id bigint NOT NULL,
    name character varying(255) NOT NULL
);


--
-- TOC entry 184 (class 1259 OID 570094)
-- Name: ar_customer_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE ar_customer_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- TOC entry 185 (class 1259 OID 570096)
-- Name: ar_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE ar_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- TOC entry 175 (class 1259 OID 569934)
-- Name: customer; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE customer (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    full_name character varying(255),
    default_language character(3) NOT NULL,
    languages character varying(255) NOT NULL,
    default_time_zone character varying(50) NOT NULL
);


--
-- TOC entry 176 (class 1259 OID 569940)
-- Name: department; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE department (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    full_name character varying(255),
    default_language character(3),
    default_time_zone character varying(50),
    customer_id bigint NOT NULL
);


--
-- TOC entry 177 (class 1259 OID 569946)
-- Name: department_profile_link; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE department_profile_link (
    id bigint NOT NULL,
    department_id bigint NOT NULL,
    profile_id bigint NOT NULL,
    sort_order bigint NOT NULL
);
ALTER TABLE department_profile_link ADD CONSTRAINT department_profile_link_pk PRIMARY KEY ( ID ) ;

--
-- TOC entry 178 (class 1259 OID 569949)
-- Name: expression; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE expression (
    id bigint NOT NULL,
    field character varying(255) NOT NULL,
    operator character varying(255) NOT NULL,
    value character varying(4000) NOT NULL,
    rule_id bigint NOT NULL,
    customer_id bigint NOT NULL,
    type character varying(50) NOT NULL
);


--
-- TOC entry 179 (class 1259 OID 569955)
-- Name: profile; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE profile (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    description character varying(255),
    customer_id bigint NOT NULL
);

--
-- TOC entry 181 (class 1259 OID 569964)
-- Name: rule; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE rule (
    id bigint NOT NULL,
    permission smallint NOT NULL,
    denial_message character varying(255),
    action_id bigint,
    action_group_id bigint,
    profile_id bigint NOT NULL,
    customer_id bigint NOT NULL,
    sort_order bigint NOT NULL,
    CONSTRAINT rule_action_ck CHECK ((((action_id IS NULL) AND (action_group_id IS NOT NULL)) OR ((action_id IS NOT NULL) AND (action_group_id IS NULL)))),
    CONSTRAINT rule_permision_ck CHECK (((permission = 1) OR (permission = 0)))
);


--
-- TOC entry 182 (class 1259 OID 569969)
-- Name: user_profile_link; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE user_profile_link (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    profile_id bigint NOT NULL,
    sort_order bigint NOT NULL
);
ALTER TABLE user_profile_link ADD CONSTRAINT user_profile_link_pk PRIMARY KEY ( ID ) ;

--
-- TOC entry 183 (class 1259 OID 569972)
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE users (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    password character varying(255) NOT NULL,
    full_name character varying(255),
    default_language character(3),
    default_time_zone character varying(50),
    admin boolean NOT NULL,
    department_id bigint NOT NULL,
    customer_id bigint NOT NULL
);


--
-- TOC entry 1941 (class 2606 OID 569979)
-- Name: action_group_action__un; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY action_action_group_link
    ADD CONSTRAINT action_group_action__un UNIQUE (action_group_id, action_id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1943 (class 2606 OID 569981)
-- Name: action_group_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY action_group
    ADD CONSTRAINT action_group_pk PRIMARY KEY (id);


--
-- TOC entry 1945 (class 2606 OID 569983)
-- Name: action_group_uk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY action_group
    ADD CONSTRAINT action_group_uk UNIQUE (name) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1935 (class 2606 OID 569985)
-- Name: action_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY action
    ADD CONSTRAINT action_pk PRIMARY KEY (id);


--
-- TOC entry 1937 (class 2606 OID 569987)
-- Name: action_uk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY action
    ADD CONSTRAINT action_uk UNIQUE (name) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1947 (class 2606 OID 569989)
-- Name: customer_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY customer
    ADD CONSTRAINT customer_pk PRIMARY KEY (id);


ALTER TABLE ONLY customer ADD CONSTRAINT CUSTOMER_NAME_UK UNIQUE ( NAME ) DEFERRABLE INITIALLY DEFERRED ;
--
-- TOC entry 1950 (class 2606 OID 569991)
-- Name: department_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY department
    ADD CONSTRAINT department_pk PRIMARY KEY (id) ;


--
-- TOC entry 1956 (class 2606 OID 569993)
-- Name: department_profiles_un; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY department_profile_link
    ADD CONSTRAINT department_profiles_un UNIQUE (department_id, profile_id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1952 (class 2606 OID 569995)
-- Name: department_uk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY department
    ADD CONSTRAINT department_uk UNIQUE (customer_id, name) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1963 (class 2606 OID 569997)
-- Name: profile_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY profile
    ADD CONSTRAINT profile_pk PRIMARY KEY (id);


--
-- TOC entry 1965 (class 2606 OID 570001)
-- Name: profile_uk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY profile
    ADD CONSTRAINT profile_uk UNIQUE (name, customer_id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1972 (class 2606 OID 570003)
-- Name: rule_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY rule
    ADD CONSTRAINT rule_pk PRIMARY KEY (id);


--
-- TOC entry 1975 (class 2606 OID 570273)
-- Name: rule_uk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY rule
    ADD CONSTRAINT rule_uk UNIQUE (profile_id, sort_order) DEFERRABLE INITIALLY DEFERRED;


--
-- TOC entry 1960 (class 2606 OID 570005)
-- Name: rules_expression_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY expression
    ADD CONSTRAINT rules_expression_pk PRIMARY KEY (id);


--
-- TOC entry 1981 (class 2606 OID 570007)
-- Name: user_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT user_pk PRIMARY KEY (id);


--
-- TOC entry 1979 (class 2606 OID 570009)
-- Name: user_profile_un; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY user_profile_link
    ADD CONSTRAINT user_profile_un UNIQUE (user_id, profile_id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1985 (class 2606 OID 570011)
-- Name: users_uk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT users_uk UNIQUE (name, customer_id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1938 (class 1259 OID 570099)
-- Name: act_act_gr_lnk_ac_gr_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX act_act_gr_lnk_ac_gr_id_idx ON action_action_group_link USING btree (action_group_id);


--
-- TOC entry 1939 (class 1259 OID 570098)
-- Name: act_act_gr_lnk_act_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX act_act_gr_lnk_act_id_idx ON action_action_group_link USING btree (action_id);


--
-- TOC entry 1953 (class 1259 OID 570101)
-- Name: dep_prof_link_prof_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX dep_prof_link_prof_id_idx ON department_profile_link USING btree (profile_id);


--
-- TOC entry 1954 (class 1259 OID 570102)
-- Name: dep_prof_lnk_dep_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX dep_prof_lnk_dep_id_idx ON department_profile_link USING btree (department_id);


--
-- TOC entry 1948 (class 1259 OID 570100)
-- Name: department_customer_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX department_customer_id_idx ON department USING btree (customer_id);


--
-- TOC entry 1957 (class 1259 OID 570104)
-- Name: expression_customer_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX expression_customer_id_idx ON expression USING btree (customer_id);


--
-- TOC entry 1958 (class 1259 OID 570103)
-- Name: expression_rule_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX expression_rule_id_idx ON expression USING btree (rule_id);


--
-- TOC entry 1961 (class 1259 OID 570105)
-- Name: profile_customer_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX profile_customer_id_idx ON profile USING btree (customer_id);


--
-- TOC entry 1968 (class 1259 OID 570108)
-- Name: rule_action_group_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX rule_action_group_id_idx ON rule USING btree (action_group_id);


--
-- TOC entry 1969 (class 1259 OID 570107)
-- Name: rule_action_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX rule_action_id_idx ON rule USING btree (action_id);


--
-- TOC entry 1970 (class 1259 OID 570109)
-- Name: rule_customer_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX rule_customer_id_idx ON rule USING btree (customer_id);


--
-- TOC entry 1973 (class 1259 OID 570106)
-- Name: rule_profile_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX rule_profile_id_idx ON rule USING btree (profile_id);


--
-- TOC entry 1976 (class 1259 OID 570112)
-- Name: user_prof_link_prof_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_prof_link_prof_id_idx ON user_profile_link USING btree (profile_id);


--
-- TOC entry 1977 (class 1259 OID 570113)
-- Name: user_prof_lnk_user_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_prof_lnk_user_id_idx ON user_profile_link USING btree (user_id);


--
-- TOC entry 1982 (class 1259 OID 570111)
-- Name: users_customer_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX users_customer_id_idx ON users USING btree (customer_id);


--
-- TOC entry 1983 (class 1259 OID 570110)
-- Name: users_dep_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX users_dep_id_idx ON users USING btree (department_id);


--
-- TOC entry 1986 (class 2606 OID 570012)
-- Name: act_ga_link_to_action_group_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY action_action_group_link
    ADD CONSTRAINT act_ga_link_to_action_group_fk FOREIGN KEY (action_group_id) REFERENCES action_group(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1988 (class 2606 OID 570017)
-- Name: dep_prof_to_department_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY department_profile_link
    ADD CONSTRAINT dep_prof_to_department_fk FOREIGN KEY (department_id) REFERENCES department(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1989 (class 2606 OID 570022)
-- Name: dep_prof_to_profile_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY department_profile_link
    ADD CONSTRAINT dep_prof_to_profile_fk FOREIGN KEY (profile_id) REFERENCES profile(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1987 (class 2606 OID 570027)
-- Name: department_to_customer_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY department
    ADD CONSTRAINT department_to_customer_fk FOREIGN KEY (customer_id) REFERENCES customer(id) DEFERRABLE INITIALLY DEFERRED ;

--
-- TOC entry 1992 (class 2606 OID 570042)
-- Name: profile_to_customer_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY profile
    ADD CONSTRAINT profile_to_customer_fk FOREIGN KEY (customer_id) REFERENCES customer(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1995 (class 2606 OID 570047)
-- Name: rule_to_action_group_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY rule
    ADD CONSTRAINT rule_to_action_group_fk FOREIGN KEY (action_group_id) REFERENCES action_group(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1996 (class 2606 OID 570054)
-- Name: rule_to_customer_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY rule
    ADD CONSTRAINT rule_to_customer_fk FOREIGN KEY (customer_id) REFERENCES customer(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1997 (class 2606 OID 570059)
-- Name: rule_to_profile_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY rule
    ADD CONSTRAINT rule_to_profile_fk FOREIGN KEY (profile_id) REFERENCES profile(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1990 (class 2606 OID 570064)
-- Name: rules_exp_to_customer_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY expression
    ADD CONSTRAINT rules_exp_to_customer_fk FOREIGN KEY (customer_id) REFERENCES customer(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1991 (class 2606 OID 570069)
-- Name: rules_expression_rule_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY expression
    ADD CONSTRAINT rules_expression_rule_fk FOREIGN KEY (rule_id) REFERENCES rule(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1998 (class 2606 OID 570074)
-- Name: user_profile_to_profile_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY user_profile_link
    ADD CONSTRAINT user_profile_to_profile_fk FOREIGN KEY (profile_id) REFERENCES profile(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 1999 (class 2606 OID 570079)
-- Name: user_profile_to_user_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY user_profile_link
    ADD CONSTRAINT user_profile_to_user_fk FOREIGN KEY (user_id) REFERENCES users(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 2000 (class 2606 OID 570084)
-- Name: user_to_customer_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT user_to_customer_fk FOREIGN KEY (customer_id) REFERENCES customer(id) DEFERRABLE INITIALLY DEFERRED ;


--
-- TOC entry 2001 (class 2606 OID 570089)
-- Name: user_to_department_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY users
    ADD CONSTRAINT user_to_department_fk FOREIGN KEY (department_id) REFERENCES department(id) DEFERRABLE INITIALLY DEFERRED ;


-- Completed on 2015-07-21 19:51:40

--
-- PostgreSQL database dump complete
--

