extern crate arg_parser;
extern crate rand;
extern crate syscall;
extern crate termion;
extern crate userutils;

use rand::{Rng, OsRng};
use std::{env, io};
use std::fs::File;
use std::io::{Read, Write};
use std::process;

use arg_parser::ArgParser;
use termion::input::TermRead;
use userutils::Passwd;

const MAN_PAGE: &'static str = /* @MANSTART{passwd} */ r#"
NAME
    passwd - modify a user's password

SYNOPSIS
    passwd
    passwd [ -h | --help ]

DESCRIPTION
    The passwd utility changes the user's local password. If the user is not
    the super-user, passwd first prompts for the current password and will
    not continue unless the correct password is entered.

OPTIONS

    -h
    --help
        Display this help and exit.

AUTHOR
    Written by Jeremy Soller.
"#;

fn main() {
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
        process::exit(0);
    }

    let uid = syscall::getuid().unwrap() as u32;

    let mut passwd_string = String::new();
    File::open("/etc/passwd").unwrap().read_to_string(&mut passwd_string).unwrap();

    let passwd = if let Some(user) = env::args().nth(1) {
        let mut passwd_option = None;
        for line in passwd_string.lines() {
            if let Ok(passwd) = Passwd::parse(line) {
                if passwd.user == user {
                    passwd_option = Some(passwd);
                    break;
                }
            }
        }
        if let Some(passwd) = passwd_option {
            passwd
        } else {
            panic!("passwd: user '{}' does not exist", user);
        }
    } else {
        let mut passwd_option = None;
        for line in passwd_string.lines() {
            if let Ok(passwd) = Passwd::parse(line) {
                if passwd.uid == uid {
                    passwd_option = Some(passwd);
                    break;
                }
            }
        }
        if let Some(passwd) = passwd_option {
            passwd
        } else {
            panic!("passwd: current user id {} does not exist", uid);
        }
    };

    if passwd.uid == uid || uid == 0 {
        writeln!(stdout, "changing password for '{}'", passwd.user).unwrap();
        let _ = stdout.flush();

        let mut verified = false;
        if passwd.hash == "" {
            verified = true;
        } else if passwd.uid == uid || uid != 0 {
            stdout.write_all(b"current password: ").unwrap();
            let _ = stdout.flush();

            if let Some(password) = stdin.read_passwd(&mut stdout).unwrap() {
                stdout.write(b"\n").unwrap();
                let _ = stdout.flush();

                if passwd.verify(&password) {
                    verified = true;
                }
            }
        } else {
            verified = true;
        }

        if verified {
            stdout.write_all(b"new password: ").unwrap();
            let _ = stdout.flush();

            if let Some(new_password) = stdin.read_passwd(&mut stdout).unwrap() {
                stdout.write(b"\nconfirm password: ").unwrap();
                let _ = stdout.flush();

                if let Some(confirm_password) = stdin.read_passwd(&mut stdout).unwrap() {
                    stdout.write(b"\n").unwrap();
                    let _ = stdout.flush();

                    if new_password == confirm_password {
                        let salt = format!("{:X}", OsRng::new().unwrap().next_u64());
                        writeln!(stdout, "{}", userutils::Passwd::encode(&new_password, &salt)).unwrap();
                    } else {
                        panic!("passwd: new password does not match confirm password");
                    }
                } else {
                    panic!("passwd: no confirm password provided");
                }
            } else {
                panic!("passwd: no new password provided");
            }
        } else {
            panic!("passwd: incorrect current password");
        }
    } else {
        panic!("passwd: you do not have permission to set the password of '{}'", passwd.user);
    }
}
