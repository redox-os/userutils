#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate liner;
extern crate termion;
extern crate userutils;

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{exit, Command};
use std::env;
use std::str;

use extra::io::fail;
use extra::option::OptionalExt;
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
const ISSUE_FILE: &'static str = "/etc/issue";
const MOTD_FILE: &'static str = "/etc/motd";
const PASSWD_FILE: &'static str = "/etc/passwd";

pub fn main() {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();

    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"]);
    parser.parse(env::args());

    // Shows the help
    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).try(&mut stderr);
        stdout.flush().try(&mut stderr);
        exit(0);
    }

    if let Ok(mut issue) = File::open(ISSUE_FILE) {
        io::copy(&mut issue, &mut stdout).try(&mut stderr);
        stdout.flush().try(&mut stderr);
    }

    loop {
        let user = liner::Context::new()
            .read_line("\x1B[1mredox login:\x1B[0m ", &mut |_| {})
            .try(&mut stderr);

        if ! user.is_empty() {
            let stdin = io::stdin();
            let mut stdin = stdin.lock();

            let mut passwd_string = String::new();
            match File::open(PASSWD_FILE) {
                Ok(mut file) => file.read_to_string(&mut passwd_string).try(&mut stderr),
                Err(err) => {
                    let msg = &format!("login: failed to open passwd file: {}", err);
                    fail(msg, &mut stderr);
                }
            };

            let passwd_file_entries = match Passwd::parse_file(&passwd_string) {
                Ok(entries) => entries,
                Err(_) => fail("login: error parsing passwd file", &mut stderr)
            };

            let mut passwd_option = passwd_file_entries.iter()
                .find(|passwd| user == passwd.user && "" == passwd.hash);

            if passwd_option.is_none() {
                stdout.write_all(b"\x1B[1mpassword:\x1B[0m ").try(&mut stderr);
                stdout.flush().try(&mut stderr);

                if let Some(password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                    stdout.write(b"\n").try(&mut stderr);
                    stdout.flush().try(&mut stderr);;

                    passwd_option = passwd_file_entries.iter()
                        .find(|passwd| user == passwd.user && passwd.verify(&password));
                }
            }

            if let Some(passwd) = passwd_option  {
                if let Ok(mut motd) = File::open(MOTD_FILE) {
                    io::copy(&mut motd, &mut stdout).try(&mut stderr);
                    stdout.flush().try(&mut stderr);
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
                        Ok(_status) => (),
                        Err(err) => panic!("login: failed to wait for '{}': {}", passwd.shell, err)
                    },
                    Err(err) => panic!("login: failed to execute '{}': {}", passwd.shell, err)
                }

                break;
            } else {
                stdout.write(b"\nLogin failed\n").try(&mut stderr);
                stdout.flush().try(&mut stderr);;
            }
        } else {
            stdout.write(b"\n").try(&mut stderr);
            stdout.flush().try(&mut stderr);;
        }
    }
}
