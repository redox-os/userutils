#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate termion;
extern crate redox_users;

use std::{env, io};
use std::io::Write;
use std::process::exit;

use arg_parser::ArgParser;
use extra::option::OptionalExt;
use termion::input::TermRead;
use redox_users::{get_uid, AllUsers};

const MAN_PAGE: &'static str = /* @MANSTART{passwd} */ r#"
NAME
    passwd - modify a user's password

SYNOPSIS
    passwd [ user ]
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
    Written by Jeremy Soller, Jose Narvaez.
"#; /* @MANEND */

fn main() {
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

    let uid = get_uid().unwrap_or_exit(1);
    let mut users = AllUsers::new().unwrap_or_exit(1);
    
    {
        let user = if parser.args.is_empty() {
            users.get_mut_by_id(uid).unwrap_or_exit(1)
        } else {
            let username = &parser.args[0];
            users.get_mut_by_name(username).unwrap_or_exit(1)
        };

        if user.uid == uid || uid == 0 {
            let msg = format!("changing password for '{}' \n", user.user);
            stdout.write_all(&msg.as_bytes()).try(&mut stderr);
            stdout.flush().try(&mut stderr);

            let mut verified = false;
            if user.is_passwd_blank() {
                verified = true;
            } else if user.uid == uid || uid != 0 {
                stdout.write_all(b"current password: ").try(&mut stderr);
                stdout.flush().try(&mut stderr);

                if let Some(password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                    stdout.write(b"\n").try(&mut stderr);
                    stdout.flush().try(&mut stderr);

                    verified = user.verify_passwd(&password)
                }
            } else {
                verified = true;
            }

            if verified {
                stdout.write_all(b"new password: ").try(&mut stderr);;
                stdout.flush().try(&mut stderr);;

                if let Some(new_password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                    stdout.write(b"\nconfirm password: ").try(&mut stderr);
                    stdout.flush().try(&mut stderr);

                    if let Some(confirm_password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                        stdout.write(b"\n").try(&mut stderr);
                        stdout.flush().try(&mut stderr);;

                        if new_password == confirm_password {
                            user.set_passwd(&new_password).unwrap_or_exit(1);
                        } else {
                            eprintln!("passwd: new password does not match confirm password");
                            exit(1);
                        }
                    } else {
                        eprintln!("passwd: no confirm password provided");
                        exit(1);
                    }
                } else {
                    eprintln!("passwd: no new password provided");
                    exit(1);
                }
            } else {
                eprintln!("passwd: incorrect current password");
                exit(1);
            }
        } else {
            eprintln!("passwd: you do not have permission to set the password of '{}'", user.user);
            exit(1);
        }
    }
    users.save().unwrap_or_exit(1);
}
