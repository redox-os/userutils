#[macro_use]
extern crate clap;

use std::io;
use std::io::Write;
use std::process::exit;

use extra::option::OptionalExt;
use redox_users::{All, AllUsers, Config, get_uid};
use termion::input::TermRead;

const _MAN_PAGE: &'static str = /* @MANSTART{passwd} */
    r#"
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
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr();

    let args = clap_app!(passwd =>
        (author: "Jeremy Soller, Jose Narvaez")
        (about: "Set user passwords")
        (@arg LOGIN: "Apply to login. Sets password for current user if not supplied")
        (@arg LOCK: -l --lock "Lock the password for an account (no login)")
    )
    .get_matches();

    if args.is_present("LOCK") {
        if get_uid().unwrap_or_exit(1) != 0 {
            eprintln!("passwd: only root is allowed to lock accounts");
            exit(1);
        }

        let mut users =
            AllUsers::authenticator(Config::default().writeable(true)).unwrap_or_exit(1);

        let Some(login) = args.value_of("LOGIN") else {
            eprintln!("passwd: no account specified to lock");
            exit(1);
        };

        let user = users.get_mut_by_name(login).unwrap_or_else(|| {
            eprintln!("passwd: user does not exist: {}", login);
            exit(1);
        });

        user.unset_passwd();
        users.save().unwrap_or_exit(1);

        return;
    }

    let uid = get_uid().unwrap_or_exit(1);
    let mut users = AllUsers::authenticator(Config::default().writeable(true)).unwrap_or_exit(1);

    let user = match args.value_of("LOGIN") {
        Some(login) => users.get_mut_by_name(login).unwrap_or_else(|| {
            eprintln!("passwd: user does not exist: {}", login);
            exit(1);
        }),
        None => users.get_mut_by_id(uid).unwrap_or_else(|| {
            eprintln!("passwd: you do not exist");
            exit(1);
        }),
    };

    if user.uid != uid && uid != 0 {
        eprintln!(
            "passwd: you do not have permission to set the password of '{}'",
            user.user
        );
        exit(1);
    }

    let msg = format!("changing password for '{}' \n", user.user);
    stdout.write_all(&msg.as_bytes()).r#try(&mut stderr);
    stdout.flush().r#try(&mut stderr);

    let mut verified = false;
    if uid == 0 {
        verified = true;
    } else if user.is_passwd_blank() {
        verified = true;
    } else if user.is_passwd_unset() {
        verified = false;
    } else {
        stdout.write_all(b"current password: ").r#try(&mut stderr);
        stdout.flush().r#try(&mut stderr);

        if let Some(password) = stdin.read_passwd(&mut stdout).r#try(&mut stderr) {
            stdout.write(b"\n").r#try(&mut stderr);
            stdout.flush().r#try(&mut stderr);

            verified = user.verify_passwd(&password)
        }
    }

    if !verified {
        eprintln!("passwd: incorrect current password");
        exit(1);
    }

    let new_password = ask_new_password(stdin, stdout, stderr);

    user.set_passwd(&new_password).unwrap_or_exit(1);
    users.save().unwrap_or_exit(1);
}

fn ask_new_password(
    mut stdin: io::StdinLock<'_>,
    mut stdout: io::StdoutLock<'_>,
    mut stderr: io::Stderr,
) -> String {
    stdout.write_all(b"new password: ").r#try(&mut stderr);
    stdout.flush().r#try(&mut stderr);
    let Some(new_password) = stdin.read_passwd(&mut stdout).r#try(&mut stderr) else {
        eprintln!("passwd: no new password provided");
        exit(1);
    };

    stdout.write(b"\nconfirm password: ").r#try(&mut stderr);
    stdout.flush().r#try(&mut stderr);
    let Some(confirm_password) = stdin.read_passwd(&mut stdout).r#try(&mut stderr) else {
        eprintln!("\npasswd: no confirm password provided");
        exit(1);
    };

    stdout.write(b"\n").r#try(&mut stderr);
    stdout.flush().r#try(&mut stderr);

    if new_password != confirm_password {
        eprintln!("passwd: new password does not match confirm password");
        exit(1);
    }
    new_password
}
