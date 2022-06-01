#![feature(let_else)]

use std::collections::HashMap;

fn main() -> () /*Result<u32, String>*/ {

    let mut m = HashMap::new();
    m.insert(12,"12A".to_owned());

    let a = m.get(&12).unwrap();

    let i : Result<u32, _> = a.parse();

    let Ok(_id) = i else {
        println!("Boom!");
        return ();
    };

    ()
}
