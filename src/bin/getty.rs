#[macro_use]
extern crate clap;
extern crate event;
extern crate extra;
extern crate libredox;
extern crate orbclient;
extern crate redox_termios;

use std::io::{self, ErrorKind, Stderr};
use std::process::{Child, Command, Stdio};
use std::str;

use std::os::unix::io::{FromRawFd, RawFd};

use event::{EventFlags, RawEventQueue};
use libredox::errno::EAGAIN;

use extra::io::fail;
use libredox::call as redox;
use libredox::flag;

const _MAN_PAGE: &'static str = /* @MANSTART{getty} */
    r#"
NAME
    getty - set terminal mode

SYNOPSIS
    getty [-J | --noclear | -C | --contain ] tty
    getty [ -h | --help ]

DESCRIPTION
    The getty utility is called by init(8) to open and initialize the tty line,
    read a login name, and invoke login(1).

OPTIONS

    -h, --help
        Display this help and exit.

    -J, --noclear
        Do not clear the screen before forking login(1).

    -C, --contain
        Run contain_login instead of login

AUTHOR
    Written by Jeremy Soller.
"#; /* @MANEND */

const DEFAULT_COLS: u32 = 80;
const DEFAULT_LINES: u32 = 30;

pub fn handle(
    event_queue: &mut RawEventQueue,
    tty_fd: RawFd,
    master_fd: RawFd,
    process: &mut Child,
) {
    // tty_fd => Display
    // master_fd => PTY

    let handle_event = |event_id: usize| {
        if event_id as RawFd == tty_fd {
            let mut packet = [0; 4096];
            loop {
                let count = match redox::read(tty_fd as usize, &mut packet) {
                    Ok(0) => return,
                    Ok(count) => count,
                    Err(ref err) if err.errno() == EAGAIN => break,
                    Err(_) => panic!("getty: failed to read from TTY"),
                };
                redox::write(master_fd as usize, &packet[..count])
                    .expect("getty: failed to write master PTY");
            }
        } else if event_id as RawFd == master_fd {
            let mut packet = [0; 4096];
            loop {
                let count = match redox::read(master_fd as usize, &mut packet) {
                    Ok(0) => return,
                    Ok(count) => count,
                    Err(ref err) if err.errno() == EAGAIN => break,
                    Err(_) => panic!("getty: failed to read from master TTY"),
                };
                redox::write(tty_fd as usize, &packet[1..count])
                    .expect("getty: failed to write to TTY");
                if packet[0] & 1 == 1 {
                    let _ = redox::fsync(tty_fd as usize);
                }
            }
        }
    };

    handle_event(tty_fd as usize);
    handle_event(master_fd as usize);

    'events: loop {
        let sys_event = event_queue
            .next()
            .expect("getty: event queue stopped")
            .expect("getty: failed to read event file");
        handle_event(sys_event.fd);

        match process.try_wait() {
            Ok(status) => match status {
                Some(_code) => break 'events,
                None => (),
            },
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => panic!("getty: failed to wait on child: {:?}", err),
            },
        }
    }

    let _ = process.kill();
    process.wait().expect("getty: failed to wait on login");
}

pub fn getpty(columns: u32, lines: u32) -> (RawFd, String) {
    let master = redox::open(
        "/scheme/pty",
        flag::O_CLOEXEC | flag::O_RDWR | flag::O_CREAT | flag::O_NONBLOCK,
        0,
    )
    .expect("getty: failed to create PTY");

    if let Ok(winsize_fd) = redox::dup(master, b"winsize") {
        let _ = redox::write(
            winsize_fd,
            &redox_termios::Winsize {
                ws_row: lines as u16,
                ws_col: columns as u16,
            },
        );
        let _ = redox::close(winsize_fd);
    }

    let mut buf: [u8; 4096] = [0; 4096];
    let count = redox::fpath(master, &mut buf).unwrap();
    (master as RawFd, unsafe {
        String::from_utf8_unchecked(Vec::from(&buf[..count]))
    })
}

fn daemon(
    tty_fd: RawFd,
    clear: bool,
    contain: bool,
    stderr: &mut Stderr,
) {
    let (columns, lines) = {
        let mut path = [0; 4096];
        if let Ok(count) = redox::fpath(tty_fd as usize, &mut path) {
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

    let mut event_queue = event::RawEventQueue::new().expect("getty: failed to open event queue");

    event_queue
        .subscribe(tty_fd as usize, 0, EventFlags::READ)
        .expect("getty: failed to fevent TTY");

    event_queue
        .subscribe(master_fd as usize, 0, EventFlags::READ)
        .expect("getty: failed to fevent master PTY");

    loop {
        if clear {
            let _ = redox::write(tty_fd as usize, b"\x1Bc");
        }
        let _ = redox::fsync(tty_fd as usize);

        let slave_stdin = redox::open(&pty, flag::O_CLOEXEC | flag::O_RDONLY, 0)
            .expect("getty: failed to open slave stdin");
        let slave_stdout = redox::open(&pty, flag::O_CLOEXEC | flag::O_WRONLY, 0)
            .expect("getty: failed to open slave stdout");
        let slave_stderr = redox::open(&pty, flag::O_CLOEXEC | flag::O_WRONLY, 0)
            .expect("getty: failed to open slave stderr");

        let mut command = if contain {
            Command::new("contain_login")
        } else {
            Command::new("login")
        };
        unsafe {
            command
                .stdin(Stdio::from_raw_fd(slave_stdin as RawFd))
                .stdout(Stdio::from_raw_fd(slave_stdout as RawFd))
                .stderr(Stdio::from_raw_fd(slave_stderr as RawFd))
                .env("TERM", "xterm-256color")
                .env("TTY", &pty);
        }

        match command.spawn() {
            Ok(mut process) => {
                handle(
                    &mut event_queue,
                    tty_fd,
                    master_fd,
                    &mut process,
                );
            }
            Err(err) => fail(&format!("getty: failed to execute login: {}", err), stderr),
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
        (@arg CONTAIN: -C --("contain") "Run contain_login instead of login")
    )
    .get_matches();

    let clear = !args.is_present("NO_CLEAR");

    let contain = args.is_present("CONTAIN");

    let vt = args.value_of("TTY").unwrap();

    let buf: String;
    let vt_path = if vt.parse::<usize>().is_ok() {
        buf = format!("/scheme/fbcon/{vt}");
        &*buf
    } else {
        vt
    };

    let tty_fd = match redox::open(
        &vt_path,
        flag::O_CLOEXEC | flag::O_RDWR | flag::O_NONBLOCK,
        0,
    ) {
        Ok(fd) => fd,
        Err(err) => fail(
            &format!("getty: failed to open TTY {}: {}", vt_path, err),
            &mut stderr,
        ),
    };

    redox_daemon::Daemon::new(|d| {
        d.ready().expect("getty: failed to notify ");
        daemon(tty_fd as RawFd, clear, contain, &mut stderr);
        std::process::exit(0);
    })
    .unwrap_or_else(|err| {
        fail(
            &format!("getty: failed to fork login: {}", err),
            &mut stderr,
        )
    });
}
