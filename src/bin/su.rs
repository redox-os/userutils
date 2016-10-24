extern crate syscall;
extern crate termion;
extern crate userutils;

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::str;

use termion::input::TermRead;
use userutils::Passwd;

pub fn main() {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut user = env::args().nth(1).unwrap_or(String::new());
    if user.is_empty() {
        user = String::from("root");
    }

    let uid = syscall::getuid().unwrap();

    let mut passwd_string = String::new();
    File::open("/etc/passwd").unwrap().read_to_string(&mut passwd_string).unwrap();

    let mut passwd_option = None;
    for line in passwd_string.lines() {
        if let Ok(passwd) = Passwd::parse(line) {
            if user == passwd.user && ("" == passwd.hash || uid == 0) {
                passwd_option = Some(passwd);
                break;
            }
        }
    }

    if passwd_option.is_none() {
        stdout.write_all(b"password: ").unwrap();
        let _ = stdout.flush();

        if let Some(password) = stdin.read_passwd(&mut stdout).unwrap() {
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
    }

    if let Some(passwd) = passwd_option  {
        let mut command = Command::new(passwd.shell);

        command.uid(passwd.uid);
        command.gid(passwd.gid);

        command.env("USER", &user);
        command.env("HOME", passwd.home);

        match command.spawn() {
            Ok(mut child) => match child.wait() {
                Ok(_status) => (), //println!("login: waited for {}: {:?}", sh, status.code()),
                Err(err) => panic!("su: failed to wait for '{}': {}", passwd.shell, err)
            },
            Err(err) => panic!("su: failed to execute '{}': {}", passwd.shell, err)
        }
    } else {
        stdout.write(b"su: authentication failed\n").unwrap();
        let _ = stdout.flush();
    }
}
