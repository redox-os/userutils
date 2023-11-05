#[macro_use]
extern crate clap;
extern crate extra;
extern crate liner;
extern crate termion;
extern crate redox_users;
extern crate userutils;

use std::fs::File;
use std::io::{self, Write};
use std::str;

use extra::option::OptionalExt;
use termion::input::TermRead;
use redox_users::{All, AllUsers, Config};
use userutils::spawn_shell;

const _MAN_PAGE: &'static str = /* @MANSTART{login} */ r#"
NAME
    login - log into the computer

SYNOPSIS
    login

DESCRIPTION
    The login utility logs users (and pseudo-users) into the computer system.

OPTIONS

    -h --help
        Display help info and exit.

AUTHOR
    Written by Jeremy Soller, Jose Narvaez.
"#; /* @MANEND */

const ISSUE_FILE: &'static str = "/etc/issue";
const MOTD_FILE: &'static str = "/etc/motd";

pub fn main() {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();

    let _args = clap_app!(login =>
        (author: "Jeremy Soller, Jose Narvaez")
        (about: "Login as a user")
    ).get_matches();

    if let Ok(mut issue) = File::open(ISSUE_FILE) {
        io::copy(&mut issue, &mut stdout).try(&mut stderr);
        stdout.flush().try(&mut stderr);
    }

    loop {
        let user = liner::Context::new()
            .read_line(
                liner::Prompt::from("\x1B[1mredox login:\x1B[0m "),
                None,
                &mut liner::BasicCompleter::new(Vec::<String>::new())
            )
            .try(&mut stderr);

        if !user.is_empty() {
            let stdin = io::stdin();
            let mut stdin = stdin.lock();
            let sys_users = AllUsers::authenticator(Config::default()).unwrap_or_exit(1);

            match sys_users.get_by_name(user) {
                None => {
                    stdout.write(b"\nLogin incorrect\n").try(&mut stderr);
                    stdout.write(b"\n").try(&mut stderr);
                    stdout.flush().try(&mut stderr);
                    continue;
                },
                Some(user) => {
                    if user.is_passwd_blank() {
                        if let Ok(mut motd) = File::open(MOTD_FILE) {
                            io::copy(&mut motd, &mut stdout).try(&mut stderr);
                            stdout.flush().try(&mut stderr);
                        }

                        spawn_shell(user).unwrap_or_exit(1);
                        break;
                    }

                    stdout.write_all(b"\x1B[1mpassword:\x1B[0m ").try(&mut stderr);
                    stdout.flush().try(&mut stderr);
                    if let Some(password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                        stdout.write(b"\n").try(&mut stderr);
                        stdout.flush().try(&mut stderr);

                        if user.verify_passwd(&password) {
                            if let Ok(mut motd) = File::open(MOTD_FILE) {
                                io::copy(&mut motd, &mut stdout).try(&mut stderr);
                                stdout.flush().try(&mut stderr);
                            }

                            spawn_shell(user).unwrap_or_exit(1);
                            break;
                        }
                    }
                }
            }
        } else {
            stdout.write(b"\n").try(&mut stderr);
            stdout.flush().try(&mut stderr);
        }
    }
}
