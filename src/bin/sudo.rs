extern crate syscall;
extern crate userutils;

use std::env;
use std::fs::File;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{self, Command};

use userutils::{Passwd, Group};

pub fn main() {
    let mut args = env::args().skip(1);
    let cmd = args.next().expect("sudo: no command provided");

    let uid = syscall::getuid().unwrap() as u32;

    if uid != 0 {
        let mut passwd_string = String::new();
        File::open("/etc/passwd").unwrap().read_to_string(&mut passwd_string).unwrap();

        let mut passwd_option = None;
        for line in passwd_string.lines() {
            if let Ok(passwd) = Passwd::parse(line) {
                if uid == passwd.uid {
                    passwd_option = Some(passwd);
                    break;
                }
            }
        }

        let passwd = passwd_option.expect("sudo: user not found in passwd");

        let mut group_string = String::new();
        File::open("/etc/group").unwrap().read_to_string(&mut group_string).unwrap();

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
            panic!("sudo: '{}' not in sudo group", passwd.user);
        }
    }

    let mut command = Command::new(&cmd);
    for arg in args {
        command.arg(&arg);
    }

    command.uid(0);
    command.gid(0);
    command.env("USER", "root");

    match command.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => process::exit(status.code().unwrap_or(0)),
            Err(err) => panic!("sudo: failed to wait for {}: {}", cmd, err)
        },
        Err(err) => panic!("sudo: failed to execute {}: {}", cmd, err)
    }
}
