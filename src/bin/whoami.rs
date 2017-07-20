#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate syscall;
extern crate userutils;

use std::io::{self, Write};
use std::process::exit;
use std::env;
use arg_parser::ArgParser;
use extra::option::OptionalExt;
use userutils::{get_euid, get_user};

const MAN_PAGE: &'static str = /* @MANSTART{whoami} */ r#"
NAME
     whoami - display effective user id

SYNOPSIS
     whoami [ -h | --help ]

DESCRIPTION
     The whoami utility displays your effective user ID as a name.

OPTIONS
    -h
    --help
        Display this help and exit.

EXIT STATUS
     The whoami utility exits 0 on success, and >0 if an error occurs.

AUTHOR
    Written by Jose Narvaez.
"#; /* @MANEND */

fn main() {
   let stdout = io::stdout();
   let mut stdout = stdout.lock();
   let mut stderr = io::stderr();

   let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"]);
   parser.parse(env::args());

    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).try(&mut stderr);
        stdout.flush().try(&mut stderr);
        exit(0);
    }

    let euid = get_euid(&mut stderr);
    let user = match get_user(euid, &mut stderr) {
        Some(user) => user,
        None => {
            let msg = format!("whoami: no user found for uid: {}", euid);
            stdout.write_all(msg.as_bytes()).try(&mut stderr);
            stdout.flush().try(&mut stderr);
            exit(1);
        }
    };

    stdout.write_all(format!("{}\n", user).as_bytes()).try(&mut stderr);
    stdout.flush().try(&mut stderr);
    exit(0);
}
