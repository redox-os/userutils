#[macro_use]
extern crate clap;

use std::io::{self, Write};
use std::process::exit;
use std::str;

use extra::option::OptionalExt;
use libc::O_CLOEXEC;
use redox_users::{All, AllUsers, Config, get_uid};
use syscall::EPERM;
use termion::input::TermRead;
use userutils::spawn_shell;

const _MAN_PAGE: &'static str = /* @MANSTART{su} */
    r#"
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
    )
    .get_matches();

    let target_user = args.value_of("LOGIN").unwrap_or("root");

    let uid = get_uid().unwrap_or_exit(1);

    let users = AllUsers::basic(Config::default()).unwrap_or_exit(1);
    let user = users.get_by_name(&target_user).unwrap_or_exit(1);

    // If the user executing su is root, then they can do anything without a password.
    // Same if the user we're being asked to login as doesn't have a password.
    if uid == 0 {
        writeln!(stdout).unwrap_or_exit(1);
        exit(spawn_shell(user).unwrap_or_exit(1));
    } else {
        let file = libredox::call::open("/scheme/sudo/su", O_CLOEXEC, 0).unwrap();

        write!(stdout, "password: ").unwrap_or_exit(1);
        stdout.flush().unwrap_or_exit(1);

        // Read the password, reading an empty string if CTRL-d is specified
        let password = stdin
            .read_passwd(&mut stdout)
            .r#try(&mut stderr)
            .unwrap_or(String::new());

        match libredox::call::write(file, password.as_bytes()) {
            Ok(_) => exit(spawn_shell(user).unwrap_or_exit(1)),
            Err(err) if err.errno() == EPERM => {
                writeln!(stderr, "su: authentication failed").unwrap_or_exit(1);
                exit(1);
            }
            Err(err) => panic!("{err}"),
        }
    }
}
