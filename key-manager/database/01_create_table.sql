CREATE TABLE public.keys
(
    id SERIAL,
    customer_name varchar(100) NOT NULL,
    ciphered_password varchar(200) NOT NULL,
    CONSTRAINT keys_pkey PRIMARY KEY (id)
)
    WITH (
        OIDS = FALSE
        )
    TABLESPACE pg_default;

ALTER TABLE public.keys
    OWNER to denis;
-- Index: idx_keys

-- DROP INDEX public.idx_keys;

CREATE UNIQUE INDEX idx_keys
    ON public.keys USING btree
    (customer_name ASC NULLS LAST)
    TABLESPACE pg_default;