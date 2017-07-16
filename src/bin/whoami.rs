#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate syscall;
extern crate userutils;

use std::io::{self, Read, Write};
use std::process::exit;
use std::fs::File;
use std::env;
use arg_parser::ArgParser;
use extra::option::OptionalExt;
use userutils::Passwd;

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

const PASSWD_FILE: &'static str = "/etc/passwd";

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

    match syscall::geteuid() {
        Ok(euid) => {
            let mut passwd_string = String::new();
            if let Ok(mut file) = File::open(PASSWD_FILE) {
                let _ = file.read_to_string(&mut passwd_string);
            }

            for line in passwd_string.lines() {
                if let Ok(passwd) = Passwd::parse(line) {
                    if euid == passwd.uid as usize {
                        stdout.write_all(format!("{}\n", passwd.name).as_bytes()).try(&mut stderr);
                        stdout.flush().try(&mut stderr);
                        exit(0);
                    }
                }
            }
        },
        Err(_) => exit(1)
    };
}
