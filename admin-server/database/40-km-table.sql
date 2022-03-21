CREATE TABLE public.customer_keys (
                             id SERIAL,
                             customer_name bigint NOT NULL,
                             ciphered_password VARCHAR(60) NOT NULL,
                             PRIMARY KEY(id)
);

CREATE UNIQUE INDEX idx_keys ON public.keys(customer_id);

ALTER TABLE public.keys OWNER to denis;