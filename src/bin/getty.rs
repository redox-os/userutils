#[macro_use]
extern crate clap;
extern crate redox_termios;
extern crate syscall;
extern crate extra;

use std::str;
use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write, Stderr};
use std::os::unix::io::{FromRawFd, RawFd};
use std::process::{Child, Command, Stdio};

use extra::io::fail;

const _MAN_PAGE: &'static str = /* @MANSTART{getty} */ r#"
NAME
    getty - set terminal mode

SYNOPSIS
    getty [-J | --noclear] tty
    getty [ -h | --help ]

DESCRIPTION
    The getty utility is called by init(8) to open and initialize the tty line,
    read a login name, and invoke login(1).

OPTIONS

    -h, --help
        Display this help and exit.

    -J, --noclear
        Do not clear the screen before forking login(1).

AUTHOR
    Written by Jeremy Soller.
"#; /* @MANEND */

const DEFAULT_COLS: u32 = 80;
const DEFAULT_LINES: u32 = 30;

pub fn handle(event_file: &mut File, tty_fd: RawFd, master_fd: RawFd, process: &mut Child) {
    let handle_event = |event_id: usize| {
        if event_id as RawFd == tty_fd {
            let mut packet = [0; 4096];
            loop {
                let count = match syscall::read(tty_fd as usize, &mut packet) {
                    Ok(0) => return,
                    Ok(count) => count,
                    Err(ref err) if err.errno == syscall::EAGAIN => break,
                    Err(_) => panic!("getty: failed to read from TTY")
                };
                syscall::write(master_fd as usize, &packet[..count]).expect("getty: failed to write master PTY");
            }
        } else if event_id as RawFd == master_fd {
            let mut packet = [0; 4096];
            loop {
                let count = match syscall::read(master_fd as usize, &mut packet) {
                    Ok(0) => return,
                    Ok(count) => count,
                    Err(ref err) if err.errno == syscall::EAGAIN => break,
                    Err(_) => panic!("getty: failed to read from master TTY")
                };
                syscall::write(tty_fd as usize, &packet[1..count]).expect("getty: failed to write to TTY");
                if packet[0] & 1 == 1 {
                    let _ = syscall::fsync(tty_fd as usize);
                }
            }
        } else {
            println!("Unknown event {}", event_id);
        }
    };

    handle_event(tty_fd as usize);
    handle_event(master_fd as usize);

    'events: loop {
        let mut sys_event = syscall::Event::default();
        event_file.read(&mut sys_event).expect("getty: failed to read event file");
        handle_event(sys_event.id);

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
    (master as RawFd, unsafe { String::from_utf8_unchecked(Vec::from(&buf[..count])) })
}

fn daemon(tty_fd: RawFd, clear: bool, stderr: &mut Stderr) {
    let (columns, lines) = {
        let mut path = [0; 4096];
        if let Ok(count) = syscall::fpath(tty_fd as usize, &mut path) {
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

    let mut event_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("event:")
        .expect("getty: failed to open event file");

    event_file.write(&syscall::Event {
        id: tty_fd as usize,
        flags: syscall::flag::EVENT_READ,
        data: 0
    }).expect("getty: failed to fevent TTY");

    event_file.write(&syscall::Event {
        id: master_fd as usize,
        flags: syscall::flag::EVENT_READ,
        data: 0
    }).expect("getty: failed to fevent master PTY");

    loop {
        if clear {
            let _ = syscall::write(tty_fd as usize, b"\x1Bc");
        }
        let _ = syscall::fsync(tty_fd as usize);

        let slave_stdin = syscall::open(&pty, syscall::O_CLOEXEC | syscall::O_RDONLY).expect("getty: failed to open slave stdin");
        let slave_stdout = syscall::open(&pty, syscall::O_CLOEXEC | syscall::O_WRONLY).expect("getty: failed to open slave stdout");
        let slave_stderr = syscall::open(&pty, syscall::O_CLOEXEC | syscall::O_WRONLY).expect("getty: failed to open slave stderr");

        let mut command = Command::new("login");
        unsafe {
            command
            .stdin(Stdio::from_raw_fd(slave_stdin as RawFd))
            .stdout(Stdio::from_raw_fd(slave_stdout as RawFd))
            .stderr(Stdio::from_raw_fd(slave_stderr as RawFd))
            .env("COLUMNS", format!("{}", columns))
            .env("LINES", format!("{}", lines))
            .env("TERM", "xterm-256color")
            .env("TTY", &pty);
        }

        match command.spawn() {
            Ok(mut process) => {
                handle(&mut event_file, tty_fd, master_fd, &mut process);
            },
            Err(err) => {
                fail(&format!("getty: failed to execute login: {}", err), stderr)
            }
        }
    }
}

pub fn main() {
    let mut stderr = io::stderr();

    let args = clap_app!(getty =>
        (author: "Jeremy Soller")
        (about: "Set terminal mode")
        (@arg TTY: +required "")
        (@arg NO_CLEAR: -J --("no-clear") "Do not clear the screen before forking")
    ).get_matches();

    let clear = !args.is_present("NO_CLEAR");

    let tty = args.value_of("TTY").unwrap();
    let tty_fd = match syscall::open(tty, syscall::O_CLOEXEC | syscall::flag::O_RDWR | syscall::flag::O_NONBLOCK) {
        Ok(fd) => fd,
        Err(err) => fail(&format!("getty: failed to open TTY {}: {}", tty, err), &mut stderr),
    };

    redox_daemon::Daemon::new(|d| {
        d.ready().expect("getty: failed to notify ");
        daemon(tty_fd as RawFd, clear, &mut stderr);
        std::process::exit(0);
    }).unwrap_or_else(|err| fail(&format!("getty: failed to fork login: {}", err), &mut stderr));
}
