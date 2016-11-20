extern crate termion;
extern crate userutils;

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

    if let Ok(mut issue) = File::open("/etc/issue") {
        io::copy(&mut issue, &mut stdout).unwrap();
        let _ = stdout.flush();
    }

    loop {
        stdout.write_all(b"\x1B[1mredox login:\x1B[0m ").unwrap();
        let _ = stdout.flush();

        let user = (&mut stdin as &mut Read).read_line().unwrap().unwrap_or(String::new());
        if ! user.is_empty() {
            let mut passwd_string = String::new();
            File::open("/etc/passwd").unwrap().read_to_string(&mut passwd_string).unwrap();

            let mut passwd_option = None;
            for line in passwd_string.lines() {
                if let Ok(passwd) = Passwd::parse(line) {
                    if user == passwd.user && "" == passwd.hash {
                        passwd_option = Some(passwd);
                        break;
                    }
                }
            }

            if passwd_option.is_none() {
                stdout.write_all(b"\x1B[1mpassword:\x1B[0m \x1B[?82h").unwrap();
                let _ = stdout.flush();

                if let Some(password) = stdin.read_line().unwrap() {
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
                stdout.write(b"\x1B[?82l").unwrap();
                let _ = stdout.flush();
            }

            if let Some(passwd) = passwd_option  {
                if let Ok(mut motd) = File::open("/etc/motd") {
                    io::copy(&mut motd, &mut stdout).unwrap();
                    let _ = stdout.flush();
                }

                let mut command = Command::new(passwd.shell);

                command.uid(passwd.uid);
                command.gid(passwd.gid);

                command.current_dir(passwd.home);

                command.env("USER", &user);
                command.env("HOME", passwd.home);

                match command.spawn() {
                    Ok(mut child) => match child.wait() {
                        Ok(_status) => (), //println!("login: waited for {}: {:?}", sh, status.code()),
                        Err(err) => panic!("login: failed to wait for '{}': {}", passwd.shell, err)
                    },
                    Err(err) => panic!("login: failed to execute '{}': {}", passwd.shell, err)
                }

                break;
            } else {
                stdout.write(b"\nLogin failed\n").unwrap();
                let _ = stdout.flush();
            }
        } else {
            stdout.write(b"\n").unwrap();
            let _ = stdout.flush();
        }
    }
}
