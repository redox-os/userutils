use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, exit};

use extra::option::OptionalExt;
use libredox::flag::O_CLOEXEC;
use redox_scheme::scheme::SchemeSync;
use redox_scheme::{
    CallerCtx, OpenResult, RequestKind, Response, SendFdRequest, SignalBehavior, Socket,
};
use redox_users::{All, AllGroups, AllUsers, Config, get_uid};
use syscall::error::*;
use syscall::flag::*;
use syscall::schemev2::NewFdFlags;
use termion::input::TermRead;

const MAX_ATTEMPTS: u16 = 3;
const _MAN_PAGE: &'static str = /* @MANSTART{sudo} */
    r#"
NAME
    sudo - execute a command as another user

SYNOPSIS
    sudo command
    sudo [ -h | --help ]

DESCRIPTION
    The sudo utility allows a permitted user to execute a command as the
    superuser or another user, as specified by the security policy.

EXIT STATUS
    Upon successful execution of a command, the exit status from sudo will
    be the exit status of the program that was executed. In case of error
    the exit status will be >0.

AUTHOR
    Written by Jeremy Soller, Jose Narvaez, bjorn3.
"#; /* @MANEND */

pub fn main() {
    if env::args().nth(1).as_deref() == Some("--daemon") {
        daemon_main();
    }

    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| {
        eprintln!("sudo: no command provided");
        exit(1);
    });

    let users = AllUsers::basic(Config::default()).unwrap_or_exit(1);
    let uid = get_uid().unwrap_or_exit(1);
    let user = users.get_by_id(uid).unwrap_or_exit(1);

    if uid == 0 {
        // We are root already. No need to elevate privileges
        run_command_as_root(&cmd, &args.collect());
    }

    let file = libredox::call::open("/scheme/sudo", O_CLOEXEC, 0).unwrap();

    let mut attempts = 0;

    loop {
        print!("[sudo] password for {}: ", user.user);
        let _ = io::stdout().flush();

        match io::stdin().read_passwd(&mut io::stdout()).unwrap() {
            Some(password) => {
                println!();

                match libredox::call::write(file, password.as_bytes()) {
                    Ok(_) => break,
                    Err(err) if err.errno() == EPERM => {
                        attempts += 1;
                        eprintln!(
                            "sudo: incorrect password or not in sudo group ({}/{})",
                            attempts, MAX_ATTEMPTS,
                        );
                        if attempts >= MAX_ATTEMPTS {
                            exit(1);
                        }
                    }
                    Err(err) => panic!("{err}"),
                }
            }
            None => {
                println!();
                exit(1);
            }
        }
    }

    // Elevate privileges of our own process with help from the sudo daemon
    let self_proc = syscall::open("/scheme/thisproc/current/open_via_dup", 0).unwrap();
    syscall::sendfd(file, self_proc, 0, 0).unwrap();

    run_command_as_root(&cmd, &args.collect());
}

enum Policy {
    Deny,
    Authenticate,
}

fn policy_for_user(uid: u32) -> Policy {
    let users = AllUsers::authenticator(Config::default()).unwrap_or_exit(1);
    let groups = AllGroups::new(Config::default()).unwrap_or_exit(1);

    let user = users.get_by_id(uid as usize).unwrap_or_exit(1);

    let sudo_group = groups.get_by_name("sudo").unwrap_or_exit(1);
    if !sudo_group.users.iter().any(|name| name == &user.user) {
        return Policy::Deny;
    }

    Policy::Authenticate
}

fn run_command_as_root(cmd: &str, args: &Vec<String>) -> ! {
    let mut command = Command::new(&cmd);
    for arg in args {
        command.arg(&arg);
    }

    command.uid(0);
    command.gid(0);
    command.env("USER", "root");
    command.env("UID", "0");
    command.env("GROUPS", "0");

    let err = command.exec();

    eprintln!("sudo: failed to execute {}: {}", cmd, err);
    exit(1);
}

struct Scheme {
    next_fd: usize,
    handles: HashMap<usize, Handle>,
}
enum Handle {
    AwaitingPassword { uid: u32 },
    AwaitingContextFd,
    Placeholder,
}

impl SchemeSync for Scheme {
    fn open(&mut self, path: &str, _flags: usize, ctx: &CallerCtx) -> Result<OpenResult> {
        // Path must be empty
        if path.trim_start_matches('/') != "" {
            return Err(Error::new(ENOENT));
        }
        let fd = self.next_fd;
        self.next_fd = self.next_fd.checked_add(1).ok_or(Error::new(EMFILE))?;
        self.handles
            .insert(fd, Handle::AwaitingPassword { uid: ctx.uid });

        Ok(OpenResult::ThisScheme {
            number: fd,
            flags: NewFdFlags::empty(),
        })
    }

    fn write(
        &mut self,
        id: usize,
        buf: &[u8],
        _off: u64,
        _flags: u32,
        _ctx: &CallerCtx,
    ) -> Result<usize> {
        let handle = self.handles.get_mut(&id).ok_or(Error::new(EBADF))?;

        let validate_utf8 = |buf| std::str::from_utf8(buf).map_err(|_| Error::new(EINVAL));

        match std::mem::replace(handle, Handle::Placeholder) {
            Handle::AwaitingPassword { uid } => {
                let users = AllUsers::authenticator(Config::default()).unwrap_or_exit(1);
                let user = users.get_by_id(uid as usize).unwrap_or_exit(1);

                match policy_for_user(uid) {
                    Policy::Deny => {
                        *handle = Handle::AwaitingPassword { uid };
                        return Err(Error::new(EPERM));
                    }
                    Policy::Authenticate => {
                        let password = validate_utf8(buf)?;
                        if user.verify_passwd(&password) {
                            *handle = Handle::AwaitingContextFd
                        } else {
                            *handle = Handle::AwaitingPassword { uid };
                            return Err(Error::new(EPERM));
                        }
                    }
                }
            }
            Handle::AwaitingContextFd => {
                *handle = Handle::AwaitingContextFd;
                return Err(Error::new(EINVAL));
            }

            Handle::Placeholder => {
                eprintln!("sudo: found placeholder handle with ID {id}");
                return Err(Error::new(EBADFD));
            }
        }
        Ok(buf.len())
    }
}
impl Scheme {
    fn on_close(&mut self, id: usize) {
        self.handles.remove(&id);
    }

    fn on_sendfd(&mut self, socket: &Socket, req: &SendFdRequest) -> Result<usize> {
        let handle = self.handles.get_mut(&req.id()).ok_or(Error::new(EBADF))?;
        match std::mem::replace(handle, Handle::Placeholder) {
            Handle::AwaitingContextFd => {
                let mut context_file = usize::MAX;
                req.obtain_fd(socket, FobtainFdFlags::empty(), Err(&mut context_file))?;
                let context_file = unsafe { OwnedFd::from_raw_fd(context_file as RawFd) };

                let context_uid_fd = unsafe {
                    OwnedFd::from_raw_fd(
                        syscall::dup(context_file.as_raw_fd() as usize, b"uid")? as RawFd
                    )
                };
                let context_gid_fd = unsafe {
                    OwnedFd::from_raw_fd(
                        syscall::dup(context_file.as_raw_fd() as usize, b"gid")? as RawFd
                    )
                };

                // The caller was previously authenticated as member of the sudo group. Allow it to
                // elevate privileges of a single process (likely itself).
                syscall::write(context_uid_fd.as_raw_fd() as usize, b"0")
                    .map_err(|_| Error::new(EIO))?;

                syscall::write(context_gid_fd.as_raw_fd() as usize, b"0")
                    .map_err(|_| Error::new(EIO))?;
            }
            old => {
                *handle = old;
                return Err(Error::new(EBADF));
            }
        }
        Ok(0)
    }
}

fn daemon_main() -> ! {
    redox_daemon::Daemon::new(move |daemon| {
        // TODO: Linux kernel audit-like logging?
        let socket = Socket::create("sudo").expect("failed to open scheme socket");

        let mut scheme = Scheme {
            next_fd: 1,
            handles: HashMap::new(),
        };

        daemon
            .ready()
            .expect("failed to signal sudo scheme readiness");

        loop {
            let Some(req) = socket
                .next_request(SignalBehavior::Restart)
                .expect("failed to get request")
            else {
                break;
            };

            let response = match req.kind() {
                RequestKind::Call(call) => call.handle_sync(&mut scheme),
                RequestKind::SendFd(req) => Response::new(scheme.on_sendfd(&socket, &req), req),
                RequestKind::OnClose { id } => {
                    scheme.on_close(id);
                    continue;
                }
                _ => continue,
            };

            socket
                .write_response(response, SignalBehavior::Restart)
                .expect("sudo: scheme write failed");
        }
        std::process::exit(0)
    })
    .expect("failed to start sudo daemon");
}
