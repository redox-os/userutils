#![deny(warnings)]

extern crate syscall;
extern crate arg_parser;
extern crate extra;

use std::{env, process, str};
use std::io::{self, Write, Stderr};
use std::process::{exit, Command};

use arg_parser::ArgParser;
use extra::io::fail;
use extra::option::OptionalExt;

const MAN_PAGE: &'static str = /* @MANSTART{getty} */ r#"
NAME
    getty - set terminal mode

SYNOPSIS
    getty [-J | --noclear] tty
    getty [ -h | --help ]

DESCRIPTION
    The getty utility is called by init(8) to open and initialize the tty line,
    read a login name, and invoke login(1).

OPTIONS

    -h
    --help
        Display this help and exit.

    -J
    --noclear
        Do not clear the screen before forking login(1).

AUTHOR
    Written by Jeremy Soller.
"#;

const HELP_INFO: &'static str = "Try ‘getty --help’ for more information.\n";
const DEFAULT_COLS: &'static str = "80";
const DEFAULT_LINES: &'static str = "30";

fn set_tty(tty: &str) -> syscall::Result<()> {
    let stdin = syscall::open(tty, syscall::flag::O_RDONLY)?;
    let stdout = syscall::open(tty, syscall::flag::O_WRONLY)?;
    let stderr = syscall::open(tty, syscall::flag::O_WRONLY)?;

    syscall::dup2(stdin, 0, &[])?;
    syscall::dup2(stdout, 1, &[])?;
    syscall::dup2(stderr, 2, &[])?;

    let _ = syscall::close(stdin);
    let _ = syscall::close(stdout);
    let _ = syscall::close(stderr);

    Ok(())
}

fn daemon(clear: bool, stderr: &mut Stderr) {
    loop {
        if clear {
            let _ = syscall::write(1, b"\x1Bc");
        }

        let _ = syscall::fsync(1);
        match Command::new("login").spawn() {
            Ok(mut child) => match child.wait() {
                Ok(_status) => (),
                Err(err) => fail(&format!("getty: failed to wait for login: {}", err), stderr)
            },
            Err(err) => fail(&format!("getty: failed to execute login: {}", err), stderr)
        }
    }
}

pub fn main() {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();

    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"])
        .add_flag(&["J", "noclear"]);
    parser.parse(env::args());

    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).try(&mut stderr);
        stdout.flush().try(&mut stderr);;
        exit(0);
    }

    if let Err(err) = parser.found_invalid() {
        stderr.write_all(err.as_bytes()).try(&mut stderr);
        stdout.write_all(HELP_INFO.as_bytes()).try(&mut stderr);
        stderr.flush().try(&mut stderr);
        process::exit(1);
    }

    let mut clear = true;
    if parser.found("noclear") {
        clear = false
    }

    if parser.args.len() < 1 {
        fail("getty: no TTY provided", &mut stderr);
    }

    let tty = &parser.args[0];
    if let Err(err) = set_tty(&tty) {
        fail(&format!("getty: failed to open TTY {}: {}", tty, err), &mut stderr);
    }

    env::set_var("TTY", &tty);
    {
        let mut path = [0; 4096];
        if let Ok(count) = syscall::fpath(0, &mut path) {
            let path_str = str::from_utf8(&path[..count]).unwrap_or("");
            let reference = path_str.split(':').nth(1).unwrap_or("");
            let mut parts = reference.split('/').skip(1);
            env::set_var("COLUMNS", parts.next().unwrap_or(DEFAULT_COLS));
            env::set_var("LINES", parts.next().unwrap_or(DEFAULT_LINES));
        } else {
            env::set_var("COLUMNS", DEFAULT_COLS);
            env::set_var("LINES", DEFAULT_LINES);
        }
    }

    match unsafe { syscall::clone(0) } {
        Ok(0) => daemon(clear, &mut stderr),
        Ok(_) => (),
        Err(err) => fail(&format!("getty: failed to fork login: {}", err), &mut stderr)
    }
}
