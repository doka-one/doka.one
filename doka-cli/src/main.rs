#![feature(let_else)]

use std::collections::HashMap;
use std::env;
use anyhow::anyhow;

struct Params {
    object: String,
    action: String,
    options : Vec<(String, String)>,
}

fn parse(args : &Vec<String>) -> anyhow::Result<Params> {

    let object = args.get(1).ok_or(anyhow!(""))?.clone();
    let action = args.get(2).ok_or(anyhow!(""))?.clone();
    let mut options : Vec<(String, String)> = vec![];
    let mut i = 3;

    loop {
        let option_name = args.get(i).ok_or(anyhow!(""))?.clone();
        let option_value = args.get(i+1).ok_or(anyhow!(""))?.clone();
        options.push((option_name, option_value));
        i += 2;
        if i > args.len() {
            break;
        }
    }

    Ok(Params {
        object,
        action,
        options,
    })
}


///
/// dk [object] [action] [options]
///
fn main() -> () /*Result<u32, String>*/ {
    let args: Vec<String> = env::args().collect();
    let params = parse(&args);

    ()
}
