use anyhow::anyhow;
use postgres::{Client, NoTls};
use postgres::error::SqlState;

use commons_error::*;

use crate::{Config, step_println};

const SYSTEM_DATABASE : &str = "postgres";

pub (crate) fn test_db_connection(config: &Config) -> anyhow::Result<()> {
    let _= step_println("Testing the PostgreSQL connection...");
    let url = format!("postgresql://{}:{}@{}:{}/{}", &config.db_user_name, &config.db_user_password, &config.db_host, &config.db_port, SYSTEM_DATABASE);
    let _ =  Client::connect(&url, NoTls).map_err(eprint_fwd!("Cannot connect the PG database"))?;
    println!("Connection ok");
    Ok(())
}

const SQL_CREATE_DOKA_USER: &str   = include_str!("../resources/create_doka_user.ddl");

pub (crate) fn create_doka_user(config: &Config) -> anyhow::Result<()> {
    println!("Create doka user : {}", &config.db_user_name);

    let url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        "postgres",
        &config.db_system_password, // usually the "postgres" user password
        &config.db_host,
        &config.db_port,
        SYSTEM_DATABASE
    );

    let sql = SQL_CREATE_DOKA_USER
        .replace("{{doka_user}}", &config.db_user_name)
        .replace("{{doka_password}}", &config.db_user_password);

    let mut cnx = Client::connect(&url, NoTls)
        .map_err(eprint_fwd!("Cannot connect the database: {}", SYSTEM_DATABASE))?;

    cnx.batch_execute(&sql)
        .map_err(eprint_fwd!("Create doka user error"))?;

    println!("Done. Routine added to the database : {}", &config.db_user_name);
    Ok(())
}

// Compile-time embedded SQL (UTF-8)
// Adjust the ../ as needed so the path is correct relative to this source file.
const SQL_CREATE_EXTENSIONS: &str = include_str!("../resources/40.1_create_function.ddl");
const SQL_EXTRA_ROUTINES: &str   = include_str!("../resources/40.2_create_function.ddl");

fn add_db_routine(config: &Config, db_name: &str) -> anyhow::Result<()> {
    println!("Add routine to the database : {}", db_name);

    let url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        &config.db_user_name,
        &config.db_user_password,
        &config.db_host,
        &config.db_port,
        db_name
    );
    let mut cnx = Client::connect(&url, NoTls)
        .map_err(eprint_fwd!("Cannot connect the database: {}", db_name))?;

    // 40.1_create_function.ddl — extensions & related setup
    match cnx.batch_execute(SQL_CREATE_EXTENSIONS) {
        Ok(_) => {
            println!("Created extension in database: {}", db_name);
        }
        Err(e) => {
            let code = e.code().ok_or(anyhow!("Cannot read the SQL error"))?;
            if *code == SqlState::DUPLICATE_OBJECT {
                eprintln!("Db extension already exists in database: {}", db_name);
            } else {
                eprintln!("Db extension issue in database : {}, error: {:?} ", db_name, e);
            }
        }
    }

    // 40.2_create_function.ddl — extra routines
    cnx.batch_execute(SQL_EXTRA_ROUTINES)
        .map_err(eprint_fwd!("add db routine script error"))?;

    println!("Done. Routine added to the database : {}", db_name);
    Ok(())
}


fn create_single_database(cnx: &mut Client, db_name : &str) -> anyhow::Result<()> {

    println!("Create the database : {}", db_name);

    let sql_test_existence = r#"SELECT datname FROM pg_database where datname = '{DB_NAME}' "#;
    let batch_script = sql_test_existence.replace("{DB_NAME}", db_name);
    let result = cnx.query(&batch_script, &[])?;

    if result.is_empty() {
        let create_databases_script = r#"
            CREATE DATABASE {DB_NAME}
                WITH ENCODING = 'UTF8';
            "#;

        let batch_script = create_databases_script.replace("{DB_NAME}", db_name);

        // dbg!(&batch_script);

        // Run the commands to create the databases
        cnx.batch_execute(&batch_script).map_err(eprint_fwd!("create database script error"))?;

        println!("Done. Database created : {}", db_name);

    } else {
        println!("Database {db_name} already exists, skip the process");
    }

    Ok(())
}



pub (crate) fn create_databases(config: &Config) -> anyhow::Result<()> {
    let _ = step_println("Creating the databases...");

    let url = format!("postgresql://{}:{}@{}:{}/{}", &config.db_user_name, &config.db_user_password, &config.db_host, &config.db_port, SYSTEM_DATABASE);
    let mut cnx = Client::connect(&url, NoTls).map_err(eprint_fwd!("Cannot connect the PG database"))?;

    let _ = create_single_database(&mut cnx, & format!("ad_{}", &config.instance_name))?;
    let _ = add_db_routine(&config, & format!("ad_{}", &config.instance_name))?;

    let _ = create_single_database(&mut cnx, & format!("cs_{}", &config.instance_name))?;
    let _ = add_db_routine(&config, & format!("cs_{}", &config.instance_name))?;

    let _ = create_single_database(&mut cnx, & format!("fs_{}", &config.instance_name))?;
    let _ = add_db_routine(&config, & format!("fs_{}", &config.instance_name))?;

    Ok(())
}


const SCHEMA_DOKAADMIN: &str   = include_str!("../resources/schema_dokaadmin.ddl");
const SCHEMA_DOKASYS: &str   = include_str!("../resources/schema_dokasys.ddl");
const SCHEMA_KEYMANAGER: &str   = include_str!("../resources/schema_keymanager.ddl");

/// Build the 3 admin schemas
pub (crate) fn create_all_admin_schemas(config: &Config) -> anyhow::Result<()> {
    let _ = step_println("Initialize admin schemas");

    println!("Schema dokaadmin...");

    let db_name = format!("ad_{}", &config.instance_name);

    let url = format!("postgresql://{}:{}@{}:{}/{}", &config.db_user_name, &config.db_user_password,
                      &config.db_host, &config.db_port, &db_name);
    let mut cnx = Client::connect(&url, NoTls).map_err(eprint_fwd!("Cannot connect the database: {}", db_name))?;

    // 10_dokaadmin_schema.sql
    let _ = create_ad_schema(&mut cnx, SCHEMA_DOKAADMIN, "dokaadmin")?;

    // 20_dokasys_schema.sql
    let _ = create_ad_schema(&mut cnx, SCHEMA_DOKASYS, "dokasys")?;

    // 30_keymanager_schema.sql
    let _ = create_ad_schema(&mut cnx, SCHEMA_KEYMANAGER, "keymanager")?;

    Ok(())

}


///
/// Create a schema for the ad_<instance_name> database
///
pub fn create_ad_schema(cnx: &mut Client, schema_script : &str, schema_name: &str) -> anyhow::Result<()> {

    let sql_test_existence = r#"SELECT nspname
                        FROM pg_catalog.pg_namespace where nspname = '{SCHEMA_NAME}' "#;


    let batch_script = sql_test_existence.replace("{SCHEMA_NAME}", schema_name);
    let result = cnx.query(&batch_script, &[])?;

    if result.is_empty() {
        // Run the commands to create the databases
        cnx.batch_execute(schema_script).map_err(eprint_fwd!("create schema script error"))?;
        println!("Done. Schema created : {}", schema_name);
    } else {
        println!("⚠ Schema {schema_name} already exists, skip the process");
    }

    Ok(())

}