extern crate rand;
extern crate userutils;

use rand::{Rng, OsRng};
use std::env;

fn main() {
    let passwd = env::args().nth(1).unwrap();
    let salt = format!("{:X}", OsRng::new().unwrap().next_u64());
    println!("{}", userutils::Passwd::encode(&passwd, &salt));
}
