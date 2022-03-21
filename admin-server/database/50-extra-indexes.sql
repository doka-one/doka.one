
-- Search a user when login
CREATE UNIQUE INDEX lower_case_username ON users (customer_id, lower(name));

