#![deny(warnings)]

extern crate liner;
extern crate termion;
extern crate userutils;
extern crate arg_parser;

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{self, Command};
use std::env;
use std::str;

use arg_parser::ArgParser;
use termion::input::TermRead;
use userutils::Passwd;

const MAN_PAGE: &'static str = /* @MANSTART{login} */ r#"
NAME
    login - log into the computer

SYNOPSIS
    login

DESCRIPTION
    The login utility logs users (and pseudo-users) into the computer system.

OPTIONS

    -h
    --help
        Display this help and exit.

AUTHOR
    Written by Jeremy Soller.
"#;

pub fn main() {
    let mut stdout = io::stdout();

    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"]);
    parser.parse(env::args());

    // Shows the help
    if parser.found("help") {
        let _ = stdout.write_all(MAN_PAGE.as_bytes());
        let _ = stdout.flush();
        process::exit(0);
    }

    if let Ok(mut issue) = File::open("/etc/issue") {
        io::copy(&mut issue, &mut stdout).unwrap();
        let _ = stdout.flush();
    }

    loop {
        let user = liner::Context::new().read_line("\x1B[1mredox login:\x1B[0m ", &mut |_| {}).unwrap();
        if ! user.is_empty() {
            let stdin = io::stdin();
            let mut stdin = stdin.lock();

            let mut passwd_string = String::new();
            File::open("/etc/passwd").unwrap().read_to_string(&mut passwd_string).unwrap();

            let mut passwd_option = None;
            for line in passwd_string.lines() {
                if let Ok(passwd) = Passwd::parse(line) {
                    if user == passwd.user && "" == passwd.hash {
                        passwd_option = Some(passwd);
                        break;
                    }
                }
            }

            if passwd_option.is_none() {
                stdout.write_all(b"\x1B[1mpassword:\x1B[0m \x1B[?82h").unwrap();
                let _ = stdout.flush();

                if let Some(password) = stdin.read_line().unwrap() {
                    stdout.write(b"\n").unwrap();
                    let _ = stdout.flush();

                    for line in passwd_string.lines() {
                        if let Ok(passwd) = Passwd::parse(line) {
                            if user == passwd.user && passwd.verify(&password) {
                                passwd_option = Some(passwd);
                                break;
                            }
                        }
                    }
                }
                stdout.write(b"\x1B[?82l").unwrap();
                let _ = stdout.flush();
            }

            if let Some(passwd) = passwd_option  {
                if let Ok(mut motd) = File::open("/etc/motd") {
                    io::copy(&mut motd, &mut stdout).unwrap();
                    let _ = stdout.flush();
                }

                let mut command = Command::new(passwd.shell);

                command.uid(passwd.uid);
                command.gid(passwd.gid);

                command.current_dir(passwd.home);

                command.env("USER", &user);
                command.env("UID", format!("{}", passwd.uid));
                command.env("GROUPS", format!("{}", passwd.gid));
                command.env("HOME", passwd.home);
                command.env("SHELL", passwd.shell);

                match command.spawn() {
                    Ok(mut child) => match child.wait() {
                        Ok(_status) => (), //println!("login: waited for {}: {:?}", sh, status.code()),
                        Err(err) => panic!("login: failed to wait for '{}': {}", passwd.shell, err)
                    },
                    Err(err) => panic!("login: failed to execute '{}': {}", passwd.shell, err)
                }

                break;
            } else {
                stdout.write(b"\nLogin failed\n").unwrap();
                let _ = stdout.flush();
            }
        } else {
            stdout.write(b"\n").unwrap();
            let _ = stdout.flush();
        }
    }
}
