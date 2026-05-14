use log::*;

//
// TODO
//
// A) *** Logger
//
// The best solution is to use log4rs with log4rs-fluentd or a custom adapter if the ealier is
// not fast enough
//
// The custom adapter could  store the "records" in a Mutex<HashMap>, open a thread, read the HashMap
//    and send the records to a File/Graylog
//
// https://docs.rs/crate/log4rs/latest/source/src/append/file.rs
//
// A very good example of adapter is log4rs-fluentd
// https://github.com/Devolutions/log4rs-fluentd/blob/master/src/fluentd.rs
//
//
// B) *** Log management
//
// We could use tools Graylog or Grafana loki to import the logs from the pods
//
//

//
//Encapsulation for the logger routines
//
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        info!("[{}:{}] {}",  file!(), line!(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        debug!("[{}:{}] {}",  file!(), line!(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        error!("[{}:{}] {}",  file!(), line!(), format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_error_simple {
    ($($arg:tt)*) => {
        error!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        warn!("[{}:{}] {}",  file!(), line!(), format!($($arg)*));
    };
}

#[macro_export]
macro_rules! tr_fwd {
    () => {
        err_closure_fwd(format!("[{}:{}]", file!(), line!()).as_str())
    };
}

#[macro_export]
macro_rules! err_fwd {
    ($($arg:tt)*) => {
        err_closure_fwd(format!("{} [{}:{}]", format!($($arg)*).as_str(), file!(), line!()).as_str())
    };
}

pub fn err_closure_fwd<'a, T: std::fmt::Display>(msg: &'a str) -> Box<dyn Fn(T) -> T + 'a> {
    let lambda = move |e: T| {
        log_error_simple!("[{}] - {}", e, msg);
        e
    };
    Box::new(lambda)
}

#[macro_export]
macro_rules! eprint_fwd {
    ($($arg:tt)*) => {
        eprint_closure_fwd(format!("{} [{}:{}]", format!($($arg)*).as_str(), file!(), line!()).as_str())
    };
}

pub fn eprint_closure_fwd<'a, T: std::fmt::Display>(msg: &'a str) -> Box<dyn Fn(T) -> T + 'a> {
    let lambda = move |e: T| {
        eprintln!("[{}] - {}", e, msg);
        e
    };
    Box::new(lambda)
}

///
/// cargo test -- --nocapture --test-threads=1
///
#[cfg(test)]
mod tests {
    use crate::*;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::env;
    use std::fs::{read_to_string, File};
    use std::path::{Path, PathBuf};
    use std::process::exit;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn replace_value_with_constants(value: &str, constants: &HashMap<String, String>) -> String {
        let mut resolved_value = value.to_string();

        for (const_key, const_value) in constants {
            let placeholder = format!("${{{}}}", const_key);
            if resolved_value.contains(&placeholder) {
                resolved_value = resolved_value.replace(&placeholder, const_value);
            }
        }

        resolved_value
    }

    fn session_manager_log_config_from_file(config_file: &Path) -> anyhow::Result<PathBuf> {
        let json_data = read_to_string(config_file)?;
        let root: Value = serde_json::from_str(&json_data)?;
        let clusters = root
            .get("clusters")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("Missing clusters array"))?;

        let profile = env::var("DOKA_CLUSTER_PROFILE").expect("DOKA_CLUSTER_PROFILE must be set for commons-error tests");
        let selected_cluster = clusters
            .iter()
            .find(|cluster| cluster.get("name").and_then(Value::as_str) == Some(profile.as_str()))
            .ok_or_else(|| anyhow::anyhow!("Cluster profile not found: {}", profile))?;

        let mut constants = HashMap::new();
        if let Some(obj) = selected_cluster.get("constants").and_then(Value::as_object) {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    constants.insert(k.clone(), s.to_string());
                }
            }
        }
        let os_key = if cfg!(target_os = "linux") {
            Some("constants_linux")
        } else if cfg!(target_os = "windows") {
            Some("constants_windows")
        } else {
            None
        };
        if let Some(os_key) = os_key {
            if let Some(obj) = selected_cluster.get(os_key).and_then(Value::as_object) {
                for (k, v) in obj {
                    if let Some(s) = v.as_str() {
                        constants.insert(k.clone(), s.to_string());
                    }
                }
            }
        }

        let service = selected_cluster
            .get("services")
            .and_then(Value::as_array)
            .and_then(|services| services.iter().find(|service| service.get("name").and_then(Value::as_str) == Some("session-manager")))
            .ok_or_else(|| anyhow::anyhow!("session-manager service not found"))?;

        let log_config = service
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|props| props.get("log4rs.config"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("session-manager.log4rs.config not found"))?;

        Ok(PathBuf::from(replace_value_with_constants(log_config, &constants)))
    }

    fn init() {
        println!("Init tests");

        INIT.call_once(|| {
            let doka_env = match env::var("DOKA_ENV") {
                Ok(env) => env,
                Err(e) => {
                    eprintln!("💣 Cannot find the DOKA_ENV system variable, {}", e);
                    exit(-99);
                }
            };

            println!("Found doka_env=[{}]", &doka_env);

            let log_config_path = match session_manager_log_config_from_file(Path::new(&doka_env)) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("💣 Cannot resolve session-manager log4rs.config from {:?}: {:?}", &doka_env, e);
                    exit(-59);
                }
            };

            match log4rs::init_file(&log_config_path, Default::default()) {
                Err(e) => {
                    eprintln!("log config path : {:?} {:?}", &log_config_path, e);
                    exit(-59);
                }
                Ok(_) => {}
            }
        });
    }

    fn open_file_with_err() -> anyhow::Result<()> {
        let filename = "bar.txt";
        let _f =
            File::open(filename).map_err(err_fwd!("First level error managed by anyhow, filename=[{}]", filename))?;
        Ok(())
    }

    #[test]
    fn test_two_level_of_error() {
        init();
        println!("----------- Start test test_two_level_of_error ----------");
        let var = 125;
        let txt = "sample text";
        let session_number = 123456;
        let _res = open_file_with_err().map_err(err_fwd!(
            "Session number : {} - Second level of error by anyhow [{}] [{}]",
            session_number,
            &var,
            &txt
        ));
        println!("----------- End test test_two_level_of_error ----------");
    }

    fn meant_to_crash() -> anyhow::Result<i32> {
        let mut m: HashMap<i32, i32> = HashMap::new();
        m.insert(0, 6);
        let r = m.get(&0).ok_or(anyhow::anyhow!("Error : Missing item 0"))?;
        let _ = m.get(&1).ok_or(anyhow::anyhow!("Error : Missing item 1"))?;
        Ok(*r)
    }

    fn middle_level_routine() -> anyhow::Result<i32> {
        // middle level routine can just forward the error with err_fwd to log the program line number.
        // no message is required
        let r = meant_to_crash().map_err(tr_fwd!())?;
        Ok(r)
    }

    #[test]
    fn multi_level_error() {
        init();
        println!("----------- Start crash_error ----------");
        let session_number = 123456;
        let r = middle_level_routine().map_err(err_fwd!("Session : {} - Cannot read the internal map", session_number));
        log_error!("Last return = [{:?}]", r);
        println!("----------- End crash_error ----------");
    }
}
