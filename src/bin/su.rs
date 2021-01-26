#[macro_use]
extern crate clap;
extern crate extra;
extern crate termion;
extern crate redox_users;
extern crate userutils;

use std::io::{self, Write};
use std::process::exit;
use std::str;

use extra::option::OptionalExt;
use termion::input::TermRead;
use redox_users::{get_uid, All, AllUsers, Config};
use userutils::spawn_shell;

const _MAN_PAGE: &'static str = /* @MANSTART{su} */ r#"
NAME
    su - substitute user identity

SYNOPSIS
    su [ user ]
    su [ -h | --help ]

DESCRIPTION
    The su utility requests appropriate user credentials via PAM and switches to
    that user ID (the default user is the superuser).  A shell is then executed.

OPTIONS

    -h, --help
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

    let args = clap_app!(su =>
        (author: "Jeremy Soller, Jose Narvaez")
        (about: "substitue user identity")
        (@arg LOGIN: "Login as LOGIN. Default is \'root\'")
    ).get_matches();

    let target_user = args
        .value_of("LOGIN")
        .unwrap_or("root");

    let uid = get_uid().unwrap_or_exit(1);

    let users = AllUsers::authenticator(Config::default()).unwrap_or_exit(1);
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
