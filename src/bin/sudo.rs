use std::env;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, exit};

use extra::option::OptionalExt;
use redox_users::{All, AllGroups, AllUsers, Config, get_uid};
use termion::input::TermRead;

const MAX_ATTEMPTS: u16 = 3;
const _MAN_PAGE: &'static str = /* @MANSTART{sudo} */
    r#"
NAME
    sudo - execute a command as another user

SYNOPSIS
    sudo command
    sudo [ -h | --help ]

DESCRIPTION
    The sudo utility allows a permitted user to execute a command as the
    superuser or another user, as specified by the security policy.

EXIT STATUS
    Upon successful execution of a command, the exit status from sudo will
    be the exit status of the program that was executed. In case of error
    the exit status will be >0.

AUTHOR
    Written by Jeremy Soller, Jose Narvaez.
"#; /* @MANEND */

pub fn main() {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| {
        eprintln!("sudo: no command provided");
        exit(1);
    });

    let users = AllUsers::authenticator(Config::default()).unwrap_or_exit(1);
    let groups = AllGroups::new(Config::default()).unwrap_or_exit(1);

    let uid = get_uid().unwrap_or_exit(1);

    let user = users.get_by_id(uid).unwrap_or_exit(1);

    if uid == 0 {
        run_command_as_root(&cmd, &args.collect::<Vec<String>>());
        exit(0);
    }

    let sudo_group = groups.get_by_name("sudo").unwrap_or_exit(1);
    if !sudo_group.users.iter().any(|name| name == &user.user) {
        eprintln!("sudo: '{}' not in sudo group", user.user);
        exit(1);
    }

    if user.is_passwd_blank() {
        // FIXME: We should not be doing this as provides access to any
        // user w/o auth to run stuff as root. We should be doing something like:
        // eprintln!("sudo: '{}' is in sudo group but does not have a password set", user.user);
        // exit(1);
        run_command_as_root(&cmd, &args.collect::<Vec<String>>());
        exit(0);
    }

    let mut attempts = 0;

    loop {
        print!("[sudo] password for {}: ", user.user);
        let _ = io::stdout().flush();

        match io::stdin().read_passwd(&mut io::stdout()).unwrap() {
            Some(password) => {
                println!();

                if user.verify_passwd(&password) {
                    break;
                } else {
                    attempts += 1;
                    eprintln!("sudo: incorrect password ({}/{})", attempts, MAX_ATTEMPTS);
                    if attempts >= MAX_ATTEMPTS {
                        exit(1);
                    }
                }
            }
            None => {
                println!();
                exit(1);
            }
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
