use std::collections::HashMap;

use anyhow::anyhow;
use axum::http::StatusCode;
use axum::Json;
use log::*;
use postgres::{Client, NoTls};
use rs_uuid::iso::uuid_v4;

use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use commons_services::property_name::{KEY_MANAGER_HOSTNAME_PROPERTY, KEY_MANAGER_PORT_PROPERTY};
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::error_codes::{
    CUSTOMER_NAME_ALREADY_TAKEN, CUSTOMER_NOT_REMOVABLE, INTERNAL_DATABASE_ERROR,
    INTERNAL_TECHNICAL_ERROR, INVALID_PASSWORD, INVALID_TOKEN, USER_NAME_ALREADY_TAKEN,
};
use dkdto::{
    AddKeyRequest, CreateCustomerReply, CreateCustomerRequest, SimpleMessage, WebType,
    WebTypeBuilder,
};
use doka_cli::request_client::{KeyManagerClient, TokenType};

use crate::dk_password::valid_password;
use crate::schema_cs::CS_SCHEMA;
use crate::schema_fs::FS_SCHEMA;

struct DbServerInfo {
    host: String,
    port: u16,
    db_name: String,
    db_user: String,
    password: String,
}

// TODO improve error propagation
impl DbServerInfo {
    pub fn for_cs() -> Self {
        Self {
            host: get_prop_value("cs_db.hostname").map_err(tr_fwd!()).unwrap(),
            port: get_prop_value("cs_db.port")
                .map_err(tr_fwd!())
                .unwrap()
                .parse()
                .map_err(tr_fwd!())
                .unwrap(),
            db_name: get_prop_value("cs_db.name").map_err(tr_fwd!()).unwrap(),
            db_user: get_prop_value("cs_db.user").map_err(tr_fwd!()).unwrap(),
            password: get_prop_value("db.password").map_err(tr_fwd!()).unwrap(), // Careful, it's not cs_db
        }
    }

    pub fn for_fs() -> Self {
        Self {
            host: get_prop_value("fs_db.hostname").map_err(tr_fwd!()).unwrap(),
            port: get_prop_value("fs_db.port")
                .map_err(tr_fwd!())
                .unwrap()
                .parse()
                .map_err(tr_fwd!())
                .unwrap(),
            db_name: get_prop_value("fs_db.name").map_err(tr_fwd!()).unwrap(),
            db_user: get_prop_value("fs_db.user").map_err(tr_fwd!()).unwrap(),
            password: get_prop_value("db.password").map_err(tr_fwd!()).unwrap(), // Careful, it's not fs_db
        }
    }
}

fn generate_cs_schema_script(customer_code: &str) -> String {
    let template = CS_SCHEMA.to_string();
    let script = template.replace(
        "{customer_schema}",
        format!("cs_{}", customer_code).as_str(),
    );
    script
}

fn generate_fs_schema_script(customer_code: &str) -> String {
    let template = FS_SCHEMA.to_string();
    let script = template.replace(
        "{customer_schema}",
        format!("fs_{}", customer_code).as_str(),
    );
    script
}

fn warning_cs_schema(customer_code: &str) -> anyhow::Result<()> {
    // we don't drop the schema automatically, it could lead to user data loss.
    let dbi = DbServerInfo::for_cs();
    log_warn!(
        "Please verify if the schema cs_{} is not in the database=[{}]",
        customer_code,
        dbi.db_name
    );
    Ok(())
}

fn warning_fs_schema(customer_code: &str) -> anyhow::Result<()> {
    // we don't drop the schema automatically, it could lead to user data loss.
    let dbi = DbServerInfo::for_fs();
    log_warn!(
        "Please verify if the schema fs_{} is not in the database=[{}]",
        customer_code,
        dbi.db_name
    );
    Ok(())
}

fn set_removable_flag_customer_from_db(
    trans: &mut SQLTransaction,
    customer_code: &str,
) -> anyhow::Result<bool> {
    let mut params = HashMap::new();
    params.insert(
        "p_customer_code".to_owned(),
        CellValue::from_raw_string(customer_code.to_string()),
    );

    let query = SQLChange {
        sql_query:
            r"UPDATE dokaadmin.customer SET is_removable = TRUE  WHERE code = :p_customer_code"
                .to_string(),
        params,
        sequence_name: "".to_string(),
    };
    let nb = query.update(trans).map_err(err_fwd!("Query failed"))?;

    if nb == 0 {
        return Err(anyhow::anyhow!(
            "We did not set any removable flag for any customer"
        ));
    }

    Ok(true)
}

pub(crate) struct CustomerDelegate {
    pub security_token: SecurityToken,
    pub follower: Follower,
}

impl CustomerDelegate {
    pub fn new(security_token: SecurityToken, x_request_id: XRequestID) -> Self {
        Self {
            security_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            },
        }
    }

    /// Delegate routine for create customer
    pub async fn create_customer(
        mut self,
        customer_request: Json<CreateCustomerRequest>,
    ) -> WebType<CreateCustomerReply> {
        log_info!(
            "🚀 Start create_customer api, customer name=[{}], follower=[{}]",
            &customer_request.customer_name,
            &self.follower
        );

        // Check if the token is valid
        if !self.security_token.is_valid() {
            log_error!(
                "💣 Invalid security token, token=[{:?}], follower=[{}]",
                &self.security_token,
                &self.follower
            );
            return WebType::from_errorset(&INVALID_TOKEN);
        }

        self.follower.token_type = TokenType::Token(self.security_token.0.clone());

        log_info!("😎 Security token is valid, follower=[{}]", &self.follower);
        // Check password validity

        // | length >= 8  + 1 symbol + 1 digit + 1 capital letter
        // | All chars are symbol OR [0-9, a-z, A-Z]
        if !valid_password(&customer_request.admin_password) {
            return WebType::from_errorset(&INVALID_PASSWORD);
        };

        log_info!(
            "😎 User password is compliant, follower=[{}]",
            &self.follower
        );

        // Open the transaction
        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!(
            "Open transaction error, follower=[{}]",
            &self.follower
        ));
        let Ok(mut trans) = r_trans else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        // Verify if the customer name is not taken
        if self
            .check_customer_name_not_taken(&mut trans, &customer_request.customer_name)
            .is_err()
        {
            log_error!(
                "The customer name is already taken, follower=[{}]",
                &self.follower
            );
            return WebType::from_errorset(&CUSTOMER_NAME_ALREADY_TAKEN);
        };

        log_info!(
            "😎 Customer name is available, customer name=[{}], follower=[{}]",
            &customer_request.customer_name,
            &self.follower
        );

        // Verify if the customer's admin user is not taken
        if self
            .check_user_name_not_taken(&mut trans, &customer_request.email)
            .is_err()
        {
            log_error!(
                "The customer name is already taken, follower=[{}]",
                &self.follower
            );
            return WebType::from_errorset(&USER_NAME_ALREADY_TAKEN);
        };

        log_info!(
            "😎 Admin user name is available, user name=[{}], follower=[{}]",
            &customer_request.email,
            &self.follower
        );

        // Generate the customer code
        let customer_code: String;
        loop {
            let big_code = uuid_v4();
            let code_parts: Vec<&str> = big_code.split('-').collect();
            let customer_code_str = *code_parts.get(0).unwrap();

            // Verify if the customer code is unique in the table (loop)

            let Ok(not_taken) = self
                .check_code_not_taken(&mut trans, customer_code_str)
                .map_err(err_fwd!(
                    "Cannot verify the customer code uniqueness, follower=[{}]",
                    &self.follower
                ))
            else {
                return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
            };

            if not_taken {
                customer_code = String::from(customer_code_str);
                break;
            }
        }

        log_info!(
            "😎 Generated a free customer=[{}], follower=[{}]",
            &customer_code,
            &self.follower
        );

        // Create the schema

        if let Err(e) = self.run_cs_script(&customer_code) {
            log_error!(
                "CS schema batch failed, error [{}], follower=[{}]",
                e,
                &self.follower
            );
            trans.rollback();
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!(
            "😎 Created the CS schema, customer=[{}], follower=[{}]",
            &customer_code,
            &self.follower
        );

        if let Err(e) = self.run_fs_script(&customer_code) {
            log_error!(
                "FS schema batch failed, error [{}], follower=[{}]",
                e,
                &self.follower
            );
            trans.rollback();
            let _ = warning_cs_schema(&customer_code);
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!(
            "😎 Created the FS schema, customer=[{}], follower=[{}]",
            &customer_code,
            &self.follower
        );

        // Call the "key-manager" micro-service to create a secret master key
        let add_key_request = AddKeyRequest {
            customer_code: customer_code.clone(),
        };

        let Ok(km_host) = get_prop_value(KEY_MANAGER_HOSTNAME_PROPERTY)
            .map_err(err_fwd!("Cannot read the key manager hostname"))
        else {
            log_error!("💣 Create customer failed, follower=[{}]", &self.follower);
            return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };
        let Ok(km_port) = get_prop_value(KEY_MANAGER_PORT_PROPERTY)
            .unwrap_or("".to_string())
            .parse()
            .map_err(err_fwd!("Cannot read the key manager port"))
        else {
            log_error!("💣 Create customer failed, follower=[{}]", &self.follower);
            return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };
        let kmc = KeyManagerClient::new(&km_host, km_port);
        let response = kmc.add_key(&add_key_request, &self.follower.token_type);

        if let Err(e) = response {
            log_error!(
                "💣 Key Manager failed with status=[{:?}], follower=[{}]",
                e,
                &self.follower
            );
            let _ = warning_cs_schema(&customer_code);
            let _ = warning_fs_schema(&customer_code);
            return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        }

        // Insert the customer in the table

        let mut params: HashMap<String, CellValue> = HashMap::new();
        params.insert(
            "p_code".to_owned(),
            CellValue::from_raw_string(customer_code.clone()),
        );
        params.insert(
            "p_full_name".to_owned(),
            CellValue::from_raw_string(customer_request.customer_name.clone()),
        );
        params.insert(
            "p_default_language".to_owned(),
            CellValue::from_raw_string("ENG".to_owned()),
        );
        params.insert(
            "p_default_time_zone".to_owned(),
            CellValue::from_raw_string("Europe/Paris".to_owned()),
        );

        let sql_insert = SQLChange {
            sql_query: r#"INSERT INTO dokaadmin.customer (code, full_name, default_language, default_time_zone)
                        VALUES (:p_code, :p_full_name, :p_default_language, :p_default_time_zone) "#.to_string(),
            params,
            sequence_name: "dokaadmin.customer_id_seq".to_string(),
        };

        let Ok(customer_id) = sql_insert.insert(&mut trans).map_err(err_fwd!(
            "💣 Insertion of a new customer failed, follower=[{}]",
            &self.follower
        )) else {
            let _ = warning_cs_schema(&customer_code);
            let _ = warning_fs_schema(&customer_code);
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 Inserted new customer, customer id=[{}], follower=[{}]",
            customer_id,
            &self.follower
        );

        // Insert the admin user in the table

        // | Compute the hashed password
        let password_hash = DkEncrypt::hash_password(&customer_request.admin_password);

        let mut params: HashMap<String, CellValue> = HashMap::new();
        params.insert(
            "p_login".to_owned(),
            CellValue::from_raw_string(customer_request.email.clone()),
        );
        params.insert(
            "p_full_name".to_owned(),
            CellValue::from_raw_string(customer_request.email.clone()),
        );
        params.insert(
            "p_password_hash".to_owned(),
            CellValue::from_raw_string(password_hash.clone()),
        );
        params.insert(
            "p_default_language".to_owned(),
            CellValue::from_raw_string("ENG".to_owned()),
        );
        params.insert(
            "p_default_time_zone".to_owned(),
            CellValue::from_raw_string("Europe/Paris".to_owned()),
        );
        params.insert("p_admin".to_owned(), CellValue::from_raw_bool(true));
        params.insert(
            "p_customer_id".to_owned(),
            CellValue::from_raw_int(customer_id),
        );

        let sql_insert = SQLChange {
            sql_query: r#"INSERT INTO dokaadmin.appuser(
        login, full_name, password_hash, default_language, default_time_zone, admin, customer_id)
        VALUES (:p_login, :p_full_name, :p_password_hash, :p_default_language, :p_default_time_zone, :p_admin, :p_customer_id)"#.to_string(),
            params,
            sequence_name: "dokaadmin.appuser_id_seq".to_string(),
        };

        let Ok(user_id) = sql_insert.insert(&mut trans).map_err(err_fwd!(
            "💣 Insertion of a new admin user failed, follower=[{}]",
            &self.follower
        )) else {
            let _ = warning_cs_schema(&customer_code);
            let _ = warning_fs_schema(&customer_code);
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 Inserted new user, user id=[{}], follower=[{}]",
            user_id,
            &self.follower
        );

        // Close the transaction
        if trans
            .commit()
            .map_err(err_fwd!("Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            let _ = warning_cs_schema(&customer_code);
            let _ = warning_fs_schema(&customer_code);
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!(
            "😎 Committed. Customer created with success, follower=[{}]",
            &self.follower
        );

        log_info!("🏁 End create_customer, follower=[{}]", &self.follower);

        WebType::from_item(
            StatusCode::OK.as_u16(),
            CreateCustomerReply {
                customer_code,
                customer_id,
                admin_user_id: user_id,
            },
        )
    }

    ///
    /// Check if the customer code is not taken (true if it is not)
    ///
    fn check_code_not_taken(
        &self,
        mut trans: &mut SQLTransaction,
        customer_code: &str,
    ) -> anyhow::Result<bool> {
        let p_customer_code = CellValue::from_raw_string(customer_code.to_owned());
        let mut params = HashMap::new();
        params.insert("p_customer_code".to_owned(), p_customer_code);
        let sql_query =
            r#" SELECT 1 FROM dokaadmin.customer WHERE code = :p_customer_code"#.to_owned();

        let query = SQLQueryBlock {
            sql_query,
            params,
            start: 0,
            length: Some(1),
        };

        let sql_result: SQLDataSet = query.execute(&mut trans).map_err(err_fwd!(
            "Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        Ok(sql_result.len() == 0)
    }

    ///
    /// Check if the customer name is not taken
    ///
    fn check_customer_name_not_taken(
        &self,
        mut trans: &mut SQLTransaction,
        customer_name: &str,
    ) -> anyhow::Result<()> {
        let p_customer_name = CellValue::from_raw_string(customer_name.to_owned());
        let mut params = HashMap::new();
        params.insert("p_customer_name".to_owned(), p_customer_name);
        let sql_query =
            r#" SELECT 1 FROM dokaadmin.customer WHERE full_name = :p_customer_name"#.to_owned();

        let query = SQLQueryBlock {
            sql_query,
            params,
            start: 0,
            length: Some(1),
        };

        let sql_result: SQLDataSet = query.execute(&mut trans).map_err(err_fwd!(
            "Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;

        match sql_result.len() {
            0 => Ok(()),
            _ => Err(anyhow!("Customer name already taken")),
        }
    }

    ///
    /// Check if the user name is not taken
    ///
    fn check_user_name_not_taken(
        &self,
        mut trans: &mut SQLTransaction,
        user_name: &str,
    ) -> anyhow::Result<()> {
        let p_login = CellValue::from_raw_string(user_name.to_owned());
        let mut params = HashMap::new();
        params.insert("p_login".to_owned(), p_login);
        let sql_query = r#" SELECT 1 FROM dokaadmin.appuser WHERE login = :p_login"#.to_owned();

        let query = SQLQueryBlock {
            sql_query,
            params,
            start: 0,
            length: Some(1),
        };

        let sql_result: SQLDataSet = query.execute(&mut trans).map_err(err_fwd!(
            "Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;

        match sql_result.len() {
            0 => Ok(()),
            _ => Err(anyhow!("User name already taken")),
        }
    }

    /// If the customer is "removable",
    /// this routine drops all the cs_{} and fs_{} and also delete the customer from the db
    // TODO implement a backup procedure for the customer
    pub async fn delete_customer(mut self, customer_code: &str) -> WebType<SimpleMessage> {
        log_info!(
            "🚀 Start delete_customer api, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        );

        // Check if the token is valid
        if !self.security_token.is_valid() {
            log_error!(
                "💣 Invalid security token, token=[{:?}], follower=[{}]",
                &self.security_token,
                &self.follower
            );
            return WebType::from_errorset(&INVALID_TOKEN);
        }

        // Change the token type to "Token"
        self.follower.token_type = TokenType::Token(self.security_token.0.clone());

        // let customer_code = match customer_code.percent_decode().map_err(err_fwd!(
        //     "💣 Invalid input parameter [{}], follower=[{}]",
        //     customer_code,
        //     &self.follower
        // )) {
        //     Ok(s) => s.to_string(),
        //     Err(_) => {
        //         return WebType::from_errorset(&INVALID_REQUEST);
        //     }
        // };

        // | Open the transaction
        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!(
            "💣 Open transaction error, follower=[{}]",
            &self.follower
        ));
        let Ok(mut trans) = r_trans else {
            return WebType::from_errorset(&&INTERNAL_DATABASE_ERROR);
        };

        // Check if the customer is removable (flag is_removable)

        match self.search_customer(&mut trans, &customer_code) {
            Ok((_customer_id, is_removable)) => {
                if is_removable {
                    log_info!(
                        "😎 We found a removable customer in the system, follower=[{}]",
                        &self.follower
                    );

                    // Clear the customer table and user

                    if self
                        .delete_user_from_db(&mut trans, &customer_code)
                        .map_err(err_fwd!(
                            "💣 Cannot delete user, follower=[{}]",
                            &self.follower
                        ))
                        .is_err()
                    {
                        return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
                    }

                    log_info!(
                        "😎 We removed the users for the customer, follower=[{}]",
                        &self.follower
                    );

                    if self
                        .delete_customer_from_db(&mut trans, &customer_code)
                        .map_err(err_fwd!(
                            "💣 Cannot delete customer, follower=[{}]",
                            &self.follower
                        ))
                        .is_err()
                    {
                        return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
                    }

                    log_info!("😎 We removed the customer, follower=[{}]", &self.follower);
                } else {
                    log_error!(
                        "💣 The customer is not removable, follower=[{}]",
                        &self.follower
                    );
                    return WebType::from_errorset(&CUSTOMER_NOT_REMOVABLE);
                }
            }
            Err(_) => {
                log_warn!(
                    "⛔ We did not find the customer in the system, follower=[{}]",
                    &self.follower
                );
            }
        }

        // Remove the db schema

        if self
            .drop_cs_schema_from_db(&customer_code)
            .map_err(err_fwd!(
                "💣 Cannot delete the CS schema, follower=[{}]",
                &self.follower
            ))
            .is_err()
        {
            trans.rollback();
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        if self
            .drop_fs_schema_from_db(&customer_code)
            .map_err(err_fwd!(
                "💣 Cannot delete the FS schema, follower=[{}]",
                &self.follower
            ))
            .is_err()
        {
            trans.rollback();
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        // Close the transaction
        if trans
            .commit()
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!(
            "😎 Customer delete created with success, follower=[{}]",
            &self.follower
        );

        log_info!(
            "🏁 End delete_customer, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        );

        WebType::from_item(
            StatusCode::OK.as_u16(),
            SimpleMessage {
                message: "Ok".to_string(),
            },
        )
    }

    fn run_fs_script(&self, customer_code: &str) -> anyhow::Result<()> {
        // | Open a transaction on the cs database
        let dbi = DbServerInfo::for_fs();
        let url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            dbi.db_user, dbi.password, dbi.host, dbi.port, dbi.db_name
        );

        let mut fs_cnx =
            Client::connect(&url, NoTls).map_err(err_fwd!("Cannot connect the FS database"))?;

        // Run the commands to create the tables & co
        log_info!(
            "Generating the db script for FS schema, follower=[{}]",
            &self.follower
        );
        let batch_script = generate_fs_schema_script(customer_code);

        fs_cnx
            .batch_execute(&batch_script)
            .map_err(err_fwd!("FS batch script error"))?;

        Ok(())
    }

    fn run_cs_script(&self, customer_code: &str) -> anyhow::Result<()> {
        // | Open a transaction on the cs database
        let dbi = DbServerInfo::for_cs();
        let url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            dbi.db_user, dbi.password, dbi.host, dbi.port, dbi.db_name
        );

        let mut cs_cnx =
            Client::connect(&url, NoTls).map_err(err_fwd!("💣 Cannot connect the CS database"))?;

        // Run the commands to create the tables & co
        log_info!(
            "Generating the db script for CS schema, follower=[{}]",
            &self.follower
        );
        let batch_script = generate_cs_schema_script(customer_code);
        cs_cnx
            .batch_execute(&batch_script)
            .map_err(err_fwd!("💣 CS batch script error"))?;

        Ok(())
    }

    ///
    /// Find the customer in the db if it exists
    /// Return its id and the removable flag
    /// Or Err if not found
    ///
    fn search_customer(
        &self,
        trans: &mut SQLTransaction,
        customer_code: &str,
    ) -> anyhow::Result<(i64, bool)> {
        let mut params = HashMap::new();
        params.insert(
            "p_customer_code".to_owned(),
            CellValue::from_raw_string(customer_code.to_string()),
        );

        let query = SQLQueryBlock {
            sql_query:
                "SELECT id, is_removable FROM dokaadmin.customer WHERE code = :p_customer_code"
                    .to_string(),
            start: 0,
            length: None,
            params,
        };
        let mut data_set = query.execute(trans).map_err(err_fwd!("Query failed"))?;

        if data_set.len() == 0 {
            return Err(anyhow::anyhow!("Customer code not found"));
        }
        let _ = data_set.next();

        let customer_id = data_set.get_int("id").ok_or(anyhow!("Wrong column id"))?;
        let flag = data_set
            .get_bool("is_removable")
            .ok_or(anyhow!("Wrong column is_removable"))?;

        // let Some(customer_id) = data_set.get_int("id") else {
        //     return Err(anyhow!("Cannot read the id of the customer found"));
        // };

        Ok((customer_id, flag))
    }

    ///
    ///
    ///
    fn drop_cs_schema_from_db(&self, customer_code: &str) -> anyhow::Result<bool> {
        // | Open a transaction on the cs database
        let dbi = DbServerInfo::for_cs();
        let url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            dbi.db_user, dbi.password, dbi.host, dbi.port, dbi.db_name
        );

        let mut cs_cnx = Client::connect(&url, NoTls).map_err(err_fwd!(
            "💣 Cannot connect the CS database, follower=[{}]",
            &self.follower
        ))?;

        // Run the commands to create the tables & co
        let batch_script = format!(r"DROP SCHEMA cs_{} CASCADE", customer_code);
        cs_cnx.batch_execute(&batch_script).map_err(err_fwd!(
            "Dropping the CS schema failed, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        ))?;

        Ok(true)

        // let query = SQLChange {
        //     sql_query: format!( r"DROP SCHEMA cs_{} CASCADE", customer_code ),
        //     params : Default::default(),
        //     sequence_name: "".to_string(),
        // };
        // let _ = query.batch(trans).map_err(err_fwd!("Dropping the CS schema failed, customer_code=[{}]", customer_code))?;
        // Ok(true)
    }

    fn drop_fs_schema_from_db(&self, customer_code: &str) -> anyhow::Result<bool> {
        // | Open a transaction on the cs database
        let dbi = DbServerInfo::for_fs();
        let url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            dbi.db_user, dbi.password, dbi.host, dbi.port, dbi.db_name
        );

        let mut cs_cnx = Client::connect(&url, NoTls).map_err(err_fwd!(
            "💣 Cannot connect the FS database, follower=[{}]",
            &self.follower
        ))?;

        // Run the commands to create the tables & co
        let batch_script = format!(r"DROP SCHEMA fs_{} CASCADE", customer_code);
        cs_cnx.batch_execute(&batch_script).map_err(err_fwd!(
            "Dropping the FS schema failed, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        ))?;

        Ok(true)

        // let query = SQLChange {
        //     sql_query: format!( r"DROP SCHEMA fs_{} CASCADE", customer_code ),
        //     params : Default::default(),
        //     sequence_name: "".to_string(),
        // };
        // let _ = query.batch(trans).map_err(err_fwd!("Dropping the FS schema failed, customer_code=[{}]", customer_code))?;
        // Ok(true)
    }

    fn delete_user_from_db(
        &self,
        trans: &mut SQLTransaction,
        customer_code: &str,
    ) -> anyhow::Result<bool> {
        let mut params = HashMap::new();
        params.insert(
            "p_customer_code".to_owned(),
            CellValue::from_raw_string(customer_code.to_string()),
        );

        let query = SQLChange {
            sql_query: r"DELETE FROM dokaadmin.appuser WHERE customer_id IN
        (SELECT id FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE)"
                .to_string(),
            params,
            sequence_name: "".to_string(),
        };
        let nb_delete = query.delete(trans).map_err(err_fwd!("Query failed"))?;

        if nb_delete == 0 {
            return Err(anyhow::anyhow!(
                "We did not delete any user for the customer"
            ));
        }

        Ok(true)
    }

    fn delete_customer_from_db(
        &self,
        trans: &mut SQLTransaction,
        customer_code: &str,
    ) -> anyhow::Result<bool> {
        let mut params = HashMap::new();
        params.insert(
            "p_customer_code".to_owned(),
            CellValue::from_raw_string(customer_code.to_string()),
        );

        let query = SQLChange {
            sql_query: r"DELETE FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE".to_string(),
            params,
            sequence_name: "".to_string(),
        };
        let nb = query.delete(trans).map_err(err_fwd!("Query failed"))?;

        if nb == 0 {
            return Err(anyhow::anyhow!("We did not delete any customer"));
        }

        Ok(true)
    }

    ///
    /// 🔑 Set the flag to removable on a customer
    ///
    pub async fn set_removable_flag_customer(
        mut self,
        customer_code: &str,
    ) -> WebType<SimpleMessage> {
        log_info!(
            "🚀 Start set_removable_flag_customer api, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        );

        // Check if the token is valid
        if !self.security_token.is_valid() {
            log_error!(
                "💣 Invalid security token, token=[{:?}], follower=[{}]",
                &self.security_token,
                &self.follower
            );
            return WebType::from_errorset(&INVALID_TOKEN);
        }

        self.follower.token_type = TokenType::Token(self.security_token.0.clone());

        // | Open the transaction
        let mut r_cnx = SQLConnection::new();
        let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!(
            "💣 Open transaction error, follower=[{}]",
            &self.follower
        )) {
            Ok(x) => x,
            Err(_) => {
                return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
            }
        };

        if set_removable_flag_customer_from_db(&mut trans, &customer_code)
            .map_err(err_fwd!(
                "💣 Cannot set the removable flag, follower=[{}]",
                &self.follower
            ))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        // Close the transaction
        if trans
            .commit()
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!(
            "😎 Set removable flag with success,follower=[{}]",
            &self.follower
        );

        log_info!(
            "🏁 End set_removable_flag_customer, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        );

        WebType::from_item(
            StatusCode::OK.as_u16(),
            SimpleMessage {
                message: "OK".to_string(),
            },
        )
    }
}
