#![deny(warnings)]

extern crate syscall;

use std::process::Command;
use std::{env, str};

pub fn main() {
    let mut tty_option = None;
    let mut clear = true;
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "-J" | "--noclear" => {
                clear = false;
            },
            _ => {
                tty_option = Some(arg);
            }
        }
    }

    if let Some(tty) = tty_option {
        let _ = syscall::close(2);
        let _ = syscall::close(1);
        let _ = syscall::close(0);

        let _ = syscall::open(&tty, syscall::flag::O_RDWR);
        let _ = syscall::open(&tty, syscall::flag::O_RDWR);
        let _ = syscall::open(&tty, syscall::flag::O_RDWR);

        env::set_var("TTY", &tty);
        {
            let mut path = [0; 4096];
            if let Ok(count) = syscall::fpath(0, &mut path) {
                let path_str = str::from_utf8(&path[..count]).unwrap_or("");
                let reference = path_str.split(':').nth(1).unwrap_or("");
                let mut parts = reference.split('/').skip(1);
                env::set_var("COLUMNS", parts.next().unwrap_or("80"));
                env::set_var("LINES", parts.next().unwrap_or("30"));
            } else {
                env::set_var("COLUMNS", "80");
                env::set_var("LINES", "30");
            }
        }

        if unsafe { syscall::clone(0).unwrap() } == 0 {
            loop {
                if clear {
                    syscall::write(1, b"\x1Bc").unwrap();
                }
                syscall::fsync(1).unwrap();
                match Command::new("login").spawn() {
                    Ok(mut child) => match child.wait() {
                        Ok(_status) => (), //println!("getty: waited for login: {:?}", status.code()),
                        Err(err) => panic!("getty: failed to wait for login: {}", err)
                    },
                    Err(err) => panic!("getty: failed to execute login: {}", err)
                }
            }
        }
    } else {
        panic!("getty: no tty provided");
    }
}
