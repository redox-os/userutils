#![deny(warnings)]

extern crate arg_parser;
extern crate syscall;
extern crate termion;
extern crate redox_users;

use std::env;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, exit};

use arg_parser::ArgParser;
use termion::input::TermRead;
use redox_users::{get_uid, get_user_by_id, get_group_by_name};

const MAX_ATTEMPTS: u16 = 3;
const MAN_PAGE: &'static str = /* @MANSTART{sudo} */ r#"
NAME
    sudo - execute a command as another user

SYNOPSIS
    sudo command
    sudo [ -h | --help ]

DESCRIPTION
    The sudo utility allows a permitted user to execute a command as the
    superuser or another user, as specified by the security policy.

OPTIONS

    -h
    --help
        Display this help and exit.

EXIT STATUS
    Upon successful execution of a command, the exit status from sudo will
    be the exit status of the program that was executed. In case of error
    the exit status will be >0.

AUTHOR
    Written by Jeremy Soller, Jose Narvaez.
"#; /* @MANEND */

pub fn main() {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"]);
    parser.parse(env::args());

    // Shows the help
    if parser.found("help") {
        let _ = stdout.write_all(MAN_PAGE.as_bytes());
        let _ = stdout.flush();
        exit(0);
    }

    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| {
        eprintln!("sudo: no command provided");
        exit(1);
    });

    let uid = get_uid().unwrap_or_else(|err| {
        eprintln!("sudo: {}", err);
        exit(1);
    });

    let user = get_user_by_id(uid).unwrap_or_else(|err| {
        eprintln!("sudo: {}", err);
        exit(1);
    });

    if uid != 0 {
        let sudo_group = get_group_by_name("sudo").unwrap_or_else(|err| {
            eprintln!("sudo: {}", err);
            exit(1);
        });

        if sudo_group.users.iter().any(|name| name == &user.user) {
            if ! user.hash.is_empty() {
                let max_attempts = MAX_ATTEMPTS;
                let mut attempts = 0;

                loop {
                    print!("[sudo] password for {}: ", user.user);
                    let _ = stdout.flush();

                    match stdin.read_passwd(&mut stdout).unwrap() {
                        Some(password) => {
                            write!(stdout, "\n").unwrap();
                            let _ = stdout.flush();

                            if user.verify_passwd(&password) {
                                break;
                            } else {
                                attempts += 1;
                                eprintln!("sudo: incorrect password ({}/{})", attempts, max_attempts);
                                if attempts >= max_attempts {
                                    exit(1);
                                }
                            }
                        },
                        None => {
                            write!(stdout, "\n").unwrap();
                            exit(1);
                        }
                    }
                }
            } else {
                // FIXME: We should not be doing this as provides access to any
                // user w/o auth to run stuff as root. We should be doing something like:
                // eprintln!("sudo: '{}' is in sudo group but does not have a password set", user.user);
                // exit(1);
                run_command_as_root(&cmd, &args.collect::<Vec<String>>());
                exit(0);
            }
        } else {
            eprintln!("sudo: '{}' not in sudo group", user.user);
            exit(1);
        }
    }

    run_command_as_root(&cmd, &args.collect::<Vec<String>>());
    exit(0);
}

fn run_command_as_root(cmd: &str, args: &Vec<String>) {
    let mut command = Command::new(&cmd);
    for arg in args {
        command.arg(&arg);
    }

    command.uid(0);
    command.gid(0);
    command.env("USER", "root");
    command.env("UID", "0");
    command.env("GROUPS", "0");

    match command.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => exit(status.code().unwrap_or(0)),
            Err(err) => {
                eprintln!("sudo: failed to wait for {}: {}", cmd, err);
                exit(1);
            }
        },
        Err(err) => {
            eprintln!("sudo: failed to execute {}: {}", cmd, err);
            exit(1);
        }
    }
}
