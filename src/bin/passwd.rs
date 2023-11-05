#[macro_use]
extern crate clap;
extern crate extra;
extern crate termion;
extern crate redox_users;

use std::io;
use std::io::Write;
use std::process::exit;

use extra::option::OptionalExt;
use termion::input::TermRead;
use redox_users::{get_uid, All, AllUsers, Config};

const _MAN_PAGE: &'static str = /* @MANSTART{passwd} */ r#"
NAME
    passwd - modify a user's password

SYNOPSIS
    passwd [ LOGIN ]
    passwd [ -h | --help ]

DESCRIPTION
    The passwd utility changes the user's local password. If the user is not
    the super-user, passwd first prompts for the current password and will
    not continue unless the correct password is entered.

OPTIONS

    -h, --help
        Display this help and exit.

    -l, --lock
        Lock the password of the named account. This changes the stored password
        hash so that it matches no encrypted value ("!")

        Users with locked passwords are not allowed to change their password.

AUTHOR
    Written by Jeremy Soller, Jose Narvaez.
"#; /* @MANEND */

fn main() {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut stderr = io::stderr();

    let args = clap_app!(passwd =>
        (author: "Jeremy Soller, Jose Narvaez")
        (about: "Set user passwords")
        (@arg LOGIN: "Apply to login. Sets password for current user if not supplied")
        (@arg LOCK: -l --lock "Lock the password for an account (no login)")
    ).get_matches();

    let uid = get_uid().unwrap_or_exit(1);
    let mut users = AllUsers::authenticator(Config::default().writeable(true)).unwrap_or_exit(1);

    {
        let user = match args.value_of("LOGIN") {
            Some(login) => users.get_mut_by_name(login).unwrap_or_else(|| {
                               eprintln!("passwd: user does not exist: {}", login);
                               exit(1);
                           }),
            None => users.get_mut_by_id(uid).unwrap_or_else(|| {
                        eprintln!("passwd: you do not exist");
                        exit(1);
                    })
        };

        if args.is_present("LOCK") {
            user.unset_passwd();
        } else if user.uid == uid || uid == 0 {
            let msg = format!("changing password for '{}' \n", user.user);
            stdout.write_all(&msg.as_bytes()).try(&mut stderr);
            stdout.flush().try(&mut stderr);

            let mut verified = false;
            if user.is_passwd_blank() {
                verified = true;
            } else if user.is_passwd_unset() && uid != 0 {
                verified = false;
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
                stdout.write_all(b"new password: ").try(&mut stderr);
                stdout.flush().try(&mut stderr);

                if let Some(new_password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                    stdout.write(b"\nconfirm password: ").try(&mut stderr);
                    stdout.flush().try(&mut stderr);

                    if let Some(confirm_password) = stdin.read_passwd(&mut stdout).try(&mut stderr) {
                        stdout.write(b"\n").try(&mut stderr);
                        stdout.flush().try(&mut stderr);

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
