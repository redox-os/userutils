#[macro_use]
extern crate clap;
extern crate extra;
extern crate libredox;
extern crate orbclient;
extern crate redox_termios;
extern crate event;

use std::io::{self, ErrorKind, Stderr};
use std::process::{Child, Command, Stdio};
use std::str;

use std::os::unix::io::{FromRawFd, RawFd};

use event::{EventFlags, RawEventQueue};
use libredox::errno::EAGAIN;
use orbclient::{Event, EventOption};

use extra::io::fail;
use libredox::flag::{O_RDONLY, self};
use libredox::call as redox;

const _MAN_PAGE: &'static str = /* @MANSTART{getty} */
    r#"
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

fn process_events(ctrl: &mut bool, events: &[Event]) -> Vec<u8> {
    let mut buf = vec![];

    for event in events.iter() {
        if let EventOption::Key(key_event) = event.to_option() {
            if key_event.scancode == 0x1D {
                *ctrl = key_event.pressed;
            } else if key_event.pressed {
                match key_event.scancode {
                    0x0E => {
                        // Backspace
                        buf.extend_from_slice(b"\x7F");
                    }
                    0x47 => {
                        // Home
                        buf.extend_from_slice(b"\x1B[H");
                    }
                    0x48 => {
                        // Up
                        buf.extend_from_slice(b"\x1B[A");
                    }
                    0x49 => {
                        // Page up
                        buf.extend_from_slice(b"\x1B[5~");
                    }
                    0x4B => {
                        // Left
                        buf.extend_from_slice(b"\x1B[D");
                    }
                    0x4D => {
                        // Right
                        buf.extend_from_slice(b"\x1B[C");
                    }
                    0x4F => {
                        // End
                        buf.extend_from_slice(b"\x1B[F");
                    }
                    0x50 => {
                        // Down
                        buf.extend_from_slice(b"\x1B[B");
                    }
                    0x51 => {
                        // Page down
                        buf.extend_from_slice(b"\x1B[6~");
                    }
                    0x52 => {
                        // Insert
                        buf.extend_from_slice(b"\x1B[2~");
                    }
                    0x53 => {
                        // Delete
                        buf.extend_from_slice(b"\x1B[3~");
                    }
                    _ => {
                        let c = match key_event.character {
                            c @ 'A'..='Z' if *ctrl => ((c as u8 - b'A') + b'\x01') as char,
                            c @ 'a'..='z' if *ctrl => ((c as u8 - b'a') + b'\x01') as char,
                            c => c,
                        };

                        if c != '\0' {
                            let mut b = [0; 4];
                            buf.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                        }
                    }
                }
            }
        }
    }

    buf
}

pub fn handle(
    event_queue: &mut RawEventQueue,
    tty_fd: RawFd,
    consumer_fd: Option<RawFd>,
    master_fd: RawFd,
    process: &mut Child,
) {
    // tty_fd => Display
    // master_fd => PTY
    // consumer_fd => Either(`input:consumer/{#VT}`, $DEVICE)

    let mut ctrl = false;
    let mut handle_event = |event_id: usize| {
        if event_id as RawFd == tty_fd {
            let mut packet = [0; 4096];
            loop {
                let count = match redox::read(tty_fd as usize, &mut packet) {
                    Ok(0) => return,
                    Ok(count) => count,
                    Err(ref err) if err.errno == EAGAIN => break,
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
                    Err(ref err) if err.errno == EAGAIN => break,
                    Err(_) => panic!("getty: failed to read from master TTY"),
                };
                redox::write(tty_fd as usize, &packet[1..count])
                    .expect("getty: failed to write to TTY");
                if packet[0] & 1 == 1 {
                    let _ = redox::fsync(tty_fd as usize);
                }
            }
        } else {
            if let Some(consumer_fd) = consumer_fd {
                if event_id as RawFd != consumer_fd {
                    println!("getty: unknown event {}", event_id);
                }

                let mut packet = [0; 4096];
                loop {
                    let count = match redox::read(consumer_fd as usize, &mut packet) {
                        Ok(0) => return,
                        Ok(count) => count,
                        Err(ref err) if err.errno == EAGAIN => break,
                        Err(_) => panic!("getty: failed to read from master TTY"),
                    };

                    let events = unsafe {
                        core::slice::from_raw_parts(
                            packet.as_ptr() as *const Event,
                            count / core::mem::size_of::<Event>(),
                        )
                    };

                    let buf = process_events(&mut ctrl, events);
                    redox::write(master_fd as usize, buf.as_slice())
                        .expect("getty: failed to write to TTY");

                    if packet[0] & 1 == 1 {
                        let _ = redox::fsync(tty_fd as usize);
                    }
                }
            } else {
                println!("getty: unknown event {}", event_id);
            }
        }
    };

    handle_event(tty_fd as usize);
    handle_event(master_fd as usize);

    if let Some(consumer_fd) = consumer_fd {
        handle_event(consumer_fd as usize);
    }

    'events: loop {
        let sys_event = event_queue
            .next().expect("getty: event queue stopped")
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
        "pty:",
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

fn daemon(tty_fd: RawFd, consumer_fd: Option<RawFd>, clear: bool, stderr: &mut Stderr) {
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

    if let Some(consumer_fd) = consumer_fd {
        event_queue.subscribe(consumer_fd as usize, 0, EventFlags::READ).expect("getty: failed to fevent TTY");
    }

    event_queue.subscribe(tty_fd as usize, 0, EventFlags::READ).expect("getty: failed to fevent TTY");

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
                handle(
                    &mut event_queue,
                    tty_fd,
                    consumer_fd,
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
    )
    .get_matches();

    let clear = !args.is_present("NO_CLEAR");

    let vt = args.value_of("TTY").unwrap();

    let mut buf = [0; 1024];
    let (vt_path, consumer) = if vt.parse::<usize>().is_ok() {
        let consumer = redox::open(format!("input:consumer/{vt}"), O_RDONLY, 0)
            .expect("getty: failed to open consumer");

        let written = redox::fpath(consumer, &mut buf).expect("getty: failed to get the display");
        assert!(written <= buf.len());

        (
            core::str::from_utf8(&buf[..written])
                .expect("getty: UTF-8 validation failed for the display path"),
            Some(consumer as RawFd),
        )
    } else {
        (vt, None)
    };

    let tty_fd = match redox::open(
        vt_path,
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
        daemon(tty_fd as RawFd, consumer, clear, &mut stderr);
        std::process::exit(0);
    })
    .unwrap_or_else(|err| {
        fail(
            &format!("getty: failed to fork login: {}", err),
            &mut stderr,
        )
    });
}
