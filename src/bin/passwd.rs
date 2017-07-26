#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate rand;
extern crate syscall;
extern crate termion;
extern crate userutils;

use rand::{Rng, OsRng};
use std::{env, io};
use std::fs::File;
use std::io::{Read, Write};
use std::process::exit;

use arg_parser::ArgParser;
use termion::input::TermRead;
use userutils::Passwd;
use extra::option::OptionalExt;
use extra::io::fail;

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
    Written by Jeremy Soller.
"#;
const PASSWD_FILE: &'static str = "/etc/passwd";

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

    let uid = userutils::get_uid(&mut stderr) as u32;

    let mut passwd_string = String::new();
    let mut passwd_file = File::open(PASSWD_FILE).try(&mut stderr);
    passwd_file.read_to_string(&mut passwd_string).try(&mut stderr);
    let passwd_file_entries = match Passwd::parse_file(&passwd_string) {
        Ok(entries) => entries,
        Err(_) => fail("passwd: error parsing passwd file", &mut stderr)
    };

    let passwd = if parser.args.is_empty() {
        let passwd_option = passwd_file_entries.iter()
            .find(|passwd| passwd.uid == uid);

        if let Some(passwd) = passwd_option {
            passwd
        } else {
            fail(&format!("passwd: current user id {} does not exist", uid), &mut stderr);
        }
    } else {
        let user = &parser.args[0];
        let passwd_option = passwd_file_entries.iter()
            .find(|passwd| passwd.user == user);

        if let Some(passwd) = passwd_option {
            passwd
        } else {
            fail(&format!("passwd: user '{}' does not exist", user), &mut stderr);
        }
    };

    if passwd.uid == uid || uid == 0 {
        let msg = format!("changing password for '{}' \n", passwd.user);
        stdout.write_all(&msg.as_bytes()).try(&mut stderr);
        stdout.flush().try(&mut stderr);

        let mut verified = false;
        if passwd.hash == "" {
            verified = true;
        } else if passwd.uid == uid || uid != 0 {
            stdout.write_all(b"current password: ").try(&mut stderr);
            stdout.flush().try(&mut stderr);

            if let Some(password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                stdout.write(b"\n").try(&mut stderr);
                stdout.flush().try(&mut stderr);

                if passwd.verify(&password) {
                    verified = true;
                }
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
                        let salt = format!("{:X}", OsRng::new().try(&mut stderr).next_u64());
                        let encoded_passwd = userutils::Passwd::encode(&new_password, &salt);
                        //TODO: Actually persist the new password to PASSWD_FILE
                        let msg = format!("{}\n", encoded_passwd);
                        stdout.write_all(&msg.as_bytes()).try(&mut stderr);
                        stdout.flush().try(&mut stderr);
                    } else {
                        fail("passwd: new password does not match confirm password", &mut stderr);
                    }
                } else {
                    fail("passwd: no confirm password provided", &mut stderr);
                }
            } else {
                fail("passwd: no new password provided", &mut stderr);
            }
        } else {
            fail("passwd: incorrect current password", &mut stderr);
        }
    } else {
        let msg = &format!("passwd: you do not have permission to set the password of '{}'", passwd.user);
        fail(msg, &mut stderr);
    }
}
