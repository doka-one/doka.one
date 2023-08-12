use anyhow::anyhow;
use postgres::{Client, NoTls};
use postgres::error::SqlState;
use commons_error::*;
use crate::{Config, step_println};
use crate::schema_dokaadmin::SCHEMA_DOKAADMIN;
use crate::schema_dokasys::SCHEMA_DOKASYS;
use crate::schema_keymanager::SCHEMA_KEYMANAGER;

pub (crate) fn test_db_connection(config: &Config) -> anyhow::Result<()> {
    let _= step_println("Testing the PostgreSQL connection...");
    let url = format!("postgresql://{}:{}@{}:{}/{}", &config.db_user_name, &config.db_user_password, &config.db_host, &config.db_port, "postgres");
    let _ =  Client::connect(&url, NoTls).map_err(eprint_fwd!("Cannot connect the PG database"))?;
    println!("Connection ok");
    Ok(())
}

fn add_db_routine(config: &Config, db_name: &str) -> anyhow::Result<()> {

    println!("Add routine to the database : {}", db_name);

    // 40_create_function.sql

    // | Extension
    let routine_script = r#"
    CREATE EXTENSION UNACCENT;
    CREATE EXTENSION pg_trgm;

    ALTER TEXT SEARCH DICTIONARY unaccent (RULES='unaccent_default');
    "#;

    let url = format!("postgresql://{}:{}@{}:{}/{}", &config.db_user_name, &config.db_user_password,
                      &config.db_host, &config.db_port, db_name);
    let mut cnx = Client::connect(&url, NoTls).map_err(eprint_fwd!("Cannot connect the database: {}", db_name))?;

    match  cnx.batch_execute(routine_script) {
        Ok(_) => {
            println!("Created extension in database: {}", db_name);
        }
        Err(e) => {
            let code = e.code().ok_or(anyhow!("Cannot read the SQL error"))?;
            if *code == SqlState::DUPLICATE_OBJECT {
                eprintln!("Db extension already exists in database: {}", db_name);
            } else {
                eprintln!("Db extension issue in database : {}, error: {:?} ", db_name,  e);
            }
        }
    }

    // | Extra routine
    let routine_script = r#"
    CREATE OR REPLACE FUNCTION public.unaccent_lower(text)
    RETURNS text AS
    $$
    SELECT CASE
    WHEN $1 IS NULL OR $1 = ''
    THEN NULL
    ELSE lower(unaccent('unaccent', $1))
    END;
    $$
    LANGUAGE SQL IMMUTABLE SET search_path = public, pg_temp;
    "#;

    let _ = cnx.batch_execute(routine_script).map_err(eprint_fwd!("add db routine script error"))?;

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

    let url = format!("postgresql://{}:{}@{}:{}/{}", &config.db_user_name, &config.db_user_password, &config.db_host, &config.db_port, "postgres");
    let mut cnx = Client::connect(&url, NoTls).map_err(eprint_fwd!("Cannot connect the PG database"))?;

    let _ = create_single_database(&mut cnx, & format!("ad_{}", &config.instance_name))?;
    let _ = add_db_routine(&config, & format!("ad_{}", &config.instance_name))?;

    let _ = create_single_database(&mut cnx, & format!("cs_{}", &config.instance_name))?;
    let _ = add_db_routine(&config, & format!("cs_{}", &config.instance_name))?;

    let _ = create_single_database(&mut cnx, & format!("fs_{}", &config.instance_name))?;
    let _ = add_db_routine(&config, & format!("fs_{}", &config.instance_name))?;

    Ok(())
}

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