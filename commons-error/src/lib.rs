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
        // TODO Maybe we can do file and line with config settings
        info!("[{}:{}] {}",  file!(), line!(), format!($($arg)*))
        //info!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        debug!("[{}:{}] {}",  file!(), line!(), format!($($arg)*))
        //debug!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        error!("{} [{}:{}]", format!($($arg)*), file!(), line!());
        //error!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        warn!("{} [{}:{}]", format!($($arg)*), file!(), line!());
        //warn!($($arg)*)
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

pub fn err_closure_fwd<'a, T: std::fmt::Display>(msg : &'a str) -> Box<dyn Fn(T) -> T + 'a>
{
    let lambda = move |e : T | {
        log_error!("[{}] - {}", e, msg);
        e
    };
    Box::new(lambda)
}



///
/// cargo test -- --nocapture --test-threads=1
///
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::env;
    use std::fs::File;
    use std::path::Path;
    use std::process::exit;
    use std::sync::Once;
    use crate::*;

    static INIT: Once = Once::new();

    fn init() {
        println!("Init tests");

        INIT.call_once(|| {
            let doka_env = match env::var("DOKA_ENV") {
                Ok(env) => env,
                Err(e) => {
                    eprintln!("💣 Cannot find the DOKA_ENV system variable, {}", e);
                    exit(-99);
                },
            };

            println!("Found doka_env=[{}]", &doka_env);

            // let log_config: String = "E:/doka-configs/dev/ppm/config/log4rs.yaml".to_string();
            let log_config: String = format!("{}/commons-error/config/log4rs.yaml", doka_env);
            let log_config_path = Path::new(&log_config);

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
        let _f = File::open(filename).map_err(
            err_fwd!("First level error managed by anyhow, filename=[{}]", filename)
        )?;
        Ok(())
    }

    #[test]
    fn test_two_level_of_error() {
        init();
        println!("----------- Start test test_two_level_of_error ----------");
        let var = 125;
        let txt = "sample text";
        let session_number = 123456;
        let _res = open_file_with_err().map_err(err_fwd!("Session number : {} - Second level of error by anyhow [{}] [{}]",
                            session_number, &var, &txt) );
        println!("----------- End test test_two_level_of_error ----------");
    }

    fn meant_to_crash() -> anyhow::Result<i32> {
        let mut m : HashMap<i32,i32> = HashMap::new();
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
        let _r = middle_level_routine().map_err(err_fwd!("Session : {} - Cannot read the internal map", session_number));
        // println!("Q = {:?}", r);
        println!("----------- End crash_error ----------");
    }

}
