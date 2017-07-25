#![deny(warnings)]

extern crate redox_termios;
extern crate syscall;
extern crate arg_parser;
extern crate extra;

use std::{env, process, str};
use std::fs::File;
use std::io::{self, ErrorKind, Read, Write, Stderr};
use std::os::unix::io::{FromRawFd, RawFd};
use std::process::{exit, Child, Command, Stdio};

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
const DEFAULT_COLS: u32 = 80;
const DEFAULT_LINES: u32 = 30;

pub fn handle(event_file: &mut File, tty_fd: RawFd, master_fd: RawFd, process: &mut Child) {
    let handle_event = |event_id: usize, event_count: usize| -> bool {
        if event_id == tty_fd {
            let mut packet = [0; 4096];
            let count = syscall::read(tty_fd, &mut packet).expect("getty: failed to read from TTY");
            if count == 0 {
                if event_count == 0 {
                    return false;
                }
            } else {
                syscall::write(master_fd, &packet[..count]).expect("getty: failed to write master PTY");
            }
        } else if event_id == master_fd {
            let mut packet = [0; 4096];
            let count = syscall::read(master_fd, &mut packet).expect("getty: failed to read master PTY");
            if count == 0 {
                if event_count == 0 {
                    return false;
                }
            } else {
                syscall::write(tty_fd, &packet[1..count]).expect("getty: failed to write to TTY");
                if packet[0] & 1 == 1 {
                    let _ = syscall::fsync(tty_fd);
                }
            }
        } else {
            println!("Unknown event {}", event_id);
        }

        true
    };

    handle_event(tty_fd, 0);
    handle_event(master_fd, 0);

    'events: loop {
        let mut sys_event = syscall::Event::default();
        event_file.read(&mut sys_event).expect("getty: failed to read event file");
        if ! handle_event(sys_event.id, sys_event.data) {
            break 'events;
        }

        match process.try_wait() {
            Ok(status) => match status {
                Some(_code) => break 'events,
                None => ()
            },
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => panic!("getty: failed to wait on child: {:?}", err)
            }
        }
    }

    let _ = process.kill();
    process.wait().expect("getty: failed to wait on login");
}

pub fn getpty(columns: u32, lines: u32) -> (RawFd, String) {
    use redox_termios;
    use syscall;

    let master = syscall::open("pty:", syscall::O_CLOEXEC | syscall::O_RDWR | syscall::O_CREAT | syscall::O_NONBLOCK).expect("getty: failed to create PTY");

    if let Ok(winsize_fd) = syscall::dup(master, b"winsize") {
        let _ = syscall::write(winsize_fd, &redox_termios::Winsize {
            ws_row: lines as u16,
            ws_col: columns as u16
        });
        let _ = syscall::close(winsize_fd);
    }

    let mut buf: [u8; 4096] = [0; 4096];
    let count = syscall::fpath(master, &mut buf).unwrap();
    (master, unsafe { String::from_utf8_unchecked(Vec::from(&buf[..count])) })
}

fn daemon(tty_fd: RawFd, clear: bool, stderr: &mut Stderr) {
    let (columns, lines) = {
        let mut path = [0; 4096];
        if let Ok(count) = syscall::fpath(tty_fd, &mut path) {
            let path_str = str::from_utf8(&path[..count]).unwrap_or("");
            let reference = path_str.split(':').nth(1).unwrap_or("");
            let mut parts = reference.split('/').skip(1);
            let columns = parts.next().unwrap_or("").parse().unwrap_or(DEFAULT_COLS);
            let lines = parts.next().unwrap_or("").parse().unwrap_or(DEFAULT_LINES);
            (columns, lines)
        } else {
            (DEFAULT_COLS, DEFAULT_LINES)
        }
    };

    let (master_fd, pty) = getpty(columns, lines);

    let mut event_file = File::open("event:").expect("getty: failed to open event file");

    syscall::fevent(tty_fd, syscall::flag::EVENT_READ).expect("getty: failed to fevent TTY");
    syscall::fevent(master_fd, syscall::flag::EVENT_READ).expect("getty: failed to fevent master PTY");

    loop {
        if clear {
            let _ = syscall::write(tty_fd, b"\x1Bc");
        }
        let _ = syscall::fsync(tty_fd);

        let slave_stdin = syscall::open(&pty, syscall::O_CLOEXEC | syscall::O_RDONLY).expect("getty: failed to open slave stdin");
        let slave_stdout = syscall::open(&pty, syscall::O_CLOEXEC | syscall::O_WRONLY).expect("getty: failed to open slave stdout");
        let slave_stderr = syscall::open(&pty, syscall::O_CLOEXEC | syscall::O_WRONLY).expect("getty: failed to open slave stderr");

        let mut command = Command::new("login");
        unsafe {
            command
            .stdin(Stdio::from_raw_fd(slave_stdin))
            .stdout(Stdio::from_raw_fd(slave_stdout))
            .stderr(Stdio::from_raw_fd(slave_stderr))
            .env("COLUMNS", format!("{}", columns))
            .env("LINES", format!("{}", lines))
            .env("TERM", "xterm-256color")
            .env("TTY", &pty);
        }

        match command.spawn() {
            Ok(mut process) => {
                let _ = syscall::close(slave_stderr);
                let _ = syscall::close(slave_stdout);
                let _ = syscall::close(slave_stdin);

                handle(&mut event_file, tty_fd, master_fd, &mut process);
            },
            Err(err) => {
                fail(&format!("getty: failed to execute login: {}", err), stderr)
            }
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
    let tty_fd = match syscall::open(tty, syscall::O_CLOEXEC | syscall::flag::O_RDWR | syscall::flag::O_NONBLOCK) {
        Ok(fd) => fd,
        Err(err) => fail(&format!("getty: failed to open TTY {}: {}", tty, err), &mut stderr),
    };

    match unsafe { syscall::clone(0) } {
        Ok(0) => daemon(tty_fd, clear, &mut stderr),
        Ok(_) => (),
        Err(err) => fail(&format!("getty: failed to fork login: {}", err), &mut stderr)
    }
}
