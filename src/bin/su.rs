#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate termion;
extern crate userutils;

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{exit, Command};
use std::str;

use arg_parser::ArgParser;
use extra::io::fail;
use extra::option::OptionalExt;
use termion::input::TermRead;
use userutils::Passwd;

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
    Written by Jeremy Soller.
"#;
const PASSWD_FILE: &'static str = "/etc/passwd";

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
        stdout.write_all(MAN_PAGE.as_bytes()).try(&mut stderr);
        stdout.flush().try(&mut stderr);
        exit(0);
    }

    let user = if parser.args.is_empty() {
        String::from("root")
    } else {
        parser.args[0].to_string()
    };

    let uid = userutils::get_uid(&mut stderr);

    let mut passwd_string = String::new();
    let mut passwd_file = match File::open(PASSWD_FILE) {
        Ok(file) => file,
        Err(err) => fail(&format!("su: there was an error opening the passwd file: {}", err), &mut stderr)
    };

    passwd_file.read_to_string(&mut passwd_string).try(&mut stderr);

    let passwd_file_entries = match Passwd::parse_file(&passwd_string) {
        Ok(entries) => entries,
        Err(_) => fail(&format!("su: there was an error parsing the passwd file."), &mut stderr)
    };

    let mut passwd_option = passwd_file_entries.iter()
        .find(|passwd| { user == passwd.user && ("" == passwd.hash || uid == 0) });

    if passwd_option.is_none() {
        stdout.write_all(b"password: ").try(&mut stderr);
        stdout.flush().try(&mut stderr);

        if let Some(password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
            stdout.write(b"\n").try(&mut stderr);
            stdout.flush().try(&mut stderr);

            passwd_option = passwd_file_entries.iter()
                .find(|passwd| user == passwd.user && passwd.verify(&password));
        }
    }

    if let Some(passwd) = passwd_option {
        let mut command = Command::new(passwd.shell);

        command.uid(passwd.uid);
        command.gid(passwd.gid);

        command.env("USER", &user);
        command.env("UID", format!("{}", passwd.uid));
        command.env("GROUPS", format!("{}", passwd.gid));
        command.env("HOME", passwd.home);
        command.env("SHELL", passwd.shell);

        match command.spawn() {
            Ok(mut child) => match child.wait() {
                Ok(_status) => (),
                Err(err) => fail(&format!("su: failed to wait for '{}': {}", passwd.shell, err), &mut stderr)
            },
            Err(err) => fail(&format!("su: failed to execute '{}': {}", passwd.shell, err), &mut stderr)
        }
    } else {
        stdout.write(b"su: authentication failed\n").try(&mut stderr);
        stdout.flush().try(&mut stderr);
    }
}
