extern crate syscall;
extern crate userutils;

use std::env;
use std::fs::File;
use std::io::{stderr, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{self, Command};

use userutils::{Passwd, Group};

pub fn main() {
    let mut args = env::args().skip(1);
    match args.next() {
        None => {
            writeln!(stderr(), "sudo: no command provided").unwrap();
            process::exit(1);
        },
        Some(cmd) => {
            let uid = syscall::getuid().unwrap() as u32;

            if uid != 0 {
                let mut passwd_string = String::new();
                if let Ok(mut file) = File::open(userutils::FILE_PASSWD) {
                    let _ = file.read_to_string(&mut passwd_string);
                }

                let mut passwd_option = None;
                for line in passwd_string.lines() {
                    if let Ok(passwd) = Passwd::parse(line) {
                        if uid == passwd.uid {
                            passwd_option = Some(passwd);
                            break;
                        }
                    }
                }

                match passwd_option {
                    None => {
                        writeln!(stderr(), "sudo: user not found in passwd").unwrap();
                        process::exit(1);
                    },
                    Some(passwd) => {
                        let mut group_string = String::new();
                        if let Ok(mut file) = File::open(userutils::FILE_GROUP) {
                            let _ = file.read_to_string(&mut group_string);
                        }

                        let mut group_option = None;
                        for line in group_string.lines() {
                            if let Ok(group) = Group::parse(line) {
                                if group.group == "sudo" && group.users.split(',').any(|name| name == passwd.user) {
                                    group_option = Some(group);
                                    break;
                                }
                            }
                        }

                        if group_option.is_none() {
                            writeln!(stderr(), "sudo: '{}' not in sudo group", passwd.user).unwrap();
                            process::exit(1);
                        }
                    }
                }
            }

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
                    Ok(status) => process::exit(status.code().unwrap_or(0)),
                    Err(err) => {
                        writeln!(stderr(), "sudo: failed to wait for {}: {}", cmd, err).unwrap();
                        process::exit(1);
                    }
                },
                Err(err) => {
                    writeln!(stderr(), "sudo: failed to execute {}: {}", cmd, err).unwrap();
                    process::exit(1);
                }
            }
        }
    }
}
