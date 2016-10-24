extern crate termion;
extern crate userutils;

use std::env;

fn main() {
    let passwd = env::args().nth(1).unwrap();
    let salt = env::args().nth(2).unwrap();
    println!("{}", userutils::Passwd::encode(&passwd, &salt));
}
