INSERT INTO CUSTOMER (ID, NAME, FULL_NAME, DEFAULT_LANGUAGE, LANGUAGES, DEFAULT_TIME_ZONE)
VALUES (0, 'SYSTEM', 'Initial system customer with superadmin user', 'ENG', 'ENG', 'Z');

INSERT INTO DEPARTMENT (ID, NAME, FULL_NAME, CUSTOMER_ID)
VALUES (0, 'SYSTEM_DEPARTMENT', 'Default system department with superadmin user', 0);

--AES-128, key = ? todo
INSERT INTO USERS (ID, CUSTOMER_ID, DEPARTMENT_ID, NAME, FULL_NAME, ADMIN, PASSWORD)
VALUES (0, 0, 0, 'sysadmin', 'Super admin user', true, 'admin1');

-- debug "sysadmin" is  4Sryvg4EFEbUOBkfCUx2FG-1KN29OaXvnou4sBEIp0WwMvnIypO8tJrYVXCU5D7rgRxKNC_VCIS65fwSi8phNA

