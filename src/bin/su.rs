#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate termion;
extern crate redox_users;
extern crate userutils;

use std::env;
use std::io::{self, Write};
use std::process::exit;
use std::str;

use arg_parser::ArgParser;
use extra::option::OptionalExt;
use termion::input::TermRead;
use redox_users::{get_euid, get_uid, AllUsers};
use userutils::spawn_shell;

const MAN_PAGE: &'static str = /* @MANSTART{su} */ r#"
NAME
    su - substitute user identity

SYNOPSIS
    su [ user ]
    su [ -h | --help ]

DESCRIPTION
    The su utility requests appropriate user credentials via PAM and switches to
    that user ID (the default user is the superuser).  A shell is then executed.

OPTIONS

    -h
    --help
        Display this help and exit.

AUTHOR
    Written by Jeremy Soller, Jose Narvaez.
"#; /* @MANEND */

pub fn main() {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut stderr = io::stderr();

    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"]);
    parser.parse(env::args());

    // Shows the help
    if parser.found("help") {
        write!(stdout, "{}", MAN_PAGE).unwrap_or_exit(1);
        exit(0);
    }

    let target_user = if parser.args.is_empty() {
        String::from("root")
    } else {
        parser.args[0].to_string()
    };

    let uid = get_uid().unwrap_or_exit(1);
    
    let users = AllUsers::new().unwrap_or_exit(1);
    let user = users.get_by_name(&target_user).unwrap_or_exit(1);

    // If the user executing su is root, then they can do anything without a password.
    // Same if the user we're being asked to login as doesn't have a password.
    if uid == 0 || user.is_passwd_blank() {
        writeln!(stdout).unwrap_or_exit(1);
        exit(spawn_shell(user).unwrap_or_exit(1));
    } else {
        write!(stdout, "password: ").unwrap_or_exit(1);
        stdout.flush().unwrap_or_exit(1);

        // Read the password, reading an empty string if CTRL-d is specified
        let password = stdin.read_passwd(&mut stdout).try(&mut stderr).unwrap_or(String::new());

        writeln!(stderr, "\n").unwrap_or_exit(1);

        if user.verify_passwd(&password) {
            exit(spawn_shell(user).unwrap_or_exit(1));
        } else {
            writeln!(stderr, "su: authentication failed").unwrap_or_exit(1);
            exit(1);
        }
    }
}
