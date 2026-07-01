use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{exit, Command};

use extra::option::OptionalExt;
use libredox::flag::O_CLOEXEC;
use libredox::protocol::ProcCall;
use redox_rt::sys::proc_call;
use redox_scheme::scheme::{register_sync_scheme, SchemeState, SchemeSync};
use redox_scheme::{
    CallerCtx, OpenResult, RequestKind, Response, SendFdRequest, SignalBehavior, Socket,
};
use redox_users::{get_uid, All, AllGroups, AllUsers, Config};
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

fn main() {
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

    let file = libredox::Fd::open("/scheme/sudo", libredox::flag::O_CLOEXEC, 0).unwrap();

    let mut attempts = 0;

    loop {
        print!("[sudo] password for {}: ", user.user);
        let _ = io::stdout().flush();

        match io::stdin().read_passwd(&mut io::stdout()).unwrap() {
            Some(password) => {
                println!();

                match file.write(password.as_bytes()) {
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

    // FIXME move to libredox
    unsafe extern "C" {
        safe fn redox_cur_procfd_v0() -> usize;
    }

    // Elevate privileges of our own process with help from the sudo daemon
    file.call_wo(
        &libredox::call::dup(redox_cur_procfd_v0(), &[])
            .unwrap()
            .to_ne_bytes(),
        syscall::CallFlags::empty(),
        &[],
    )
    .unwrap();

    // FIXME perhaps keep the original namespace available in a subdirectory of the namespace we switch to?
    let ns = file.openat("ns", O_CLOEXEC, 0).unwrap();
    libredox::call::setns(ns.into_raw()).unwrap();

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
    AwaitingRootPassword,
    AwaitingContextFd,
    AwaitingNamespaceFetch { ns: libredox::Fd },

    AwaitingPasswordForPasswd { uid: u32 },
    AwaitingNewPassword { uid: u32 },

    Placeholder,

    SchemeRoot,
}

impl SchemeSync for Scheme {
    fn scheme_root(&mut self) -> Result<usize> {
        let fd = self.next_fd;
        self.next_fd = self.next_fd.checked_add(1).ok_or(Error::new(EMFILE))?;
        self.handles.insert(fd, Handle::SchemeRoot);
        Ok(fd)
    }
    fn openat(
        &mut self,
        dirfd: usize,
        path: &str,
        _flags: usize,
        _fcntl_flags: u32,
        ctx: &CallerCtx,
    ) -> Result<OpenResult> {
        let handle = match self.handles.get_mut(&dirfd).ok_or(Error::new(EBADF))? {
            Handle::SchemeRoot => match path {
                "" => Handle::AwaitingPassword { uid: ctx.uid },
                "su" => Handle::AwaitingRootPassword,
                "passwd" => Handle::AwaitingPasswordForPasswd { uid: ctx.uid },
                _ => return Err(Error::new(ENOENT)),
            },
            Handle::AwaitingNamespaceFetch { .. } => {
                if path != "ns" {
                    return Err(Error::new(ENOENT));
                }
                let ns = match self.handles.insert(dirfd, Handle::Placeholder).unwrap() {
                    Handle::AwaitingNamespaceFetch { ns } => ns,
                    _ => unreachable!(),
                };
                return Ok(OpenResult::OtherScheme { fd: ns.into_raw() });
            }
            _ => return Err(Error::new(EINVAL)),
        };

        let fd = self.next_fd;
        self.next_fd = self.next_fd.checked_add(1).ok_or(Error::new(EMFILE))?;
        self.handles.insert(fd, handle);

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
            Handle::AwaitingRootPassword => {
                let users = AllUsers::authenticator(Config::default()).unwrap_or_exit(1);
                let user = users.get_by_id(0).unwrap_or_exit(1);

                let password = validate_utf8(buf)?;
                if user.verify_passwd(&password) {
                    *handle = Handle::AwaitingContextFd
                } else {
                    *handle = Handle::AwaitingRootPassword;
                    return Err(Error::new(EPERM));
                }
            }
            Handle::AwaitingContextFd => {
                *handle = Handle::AwaitingContextFd;
                return Err(Error::new(EINVAL));
            }

            Handle::AwaitingPasswordForPasswd { uid } => {
                let users =
                    AllUsers::authenticator(Config::default()).map_err(|_| Error::new(ENOLCK))?;
                let user = users.get_by_id(uid as usize).ok_or(Error::new(EEXIST))?;

                let password = validate_utf8(buf)?;
                if user.verify_passwd(&password) {
                    *handle = Handle::AwaitingNewPassword { uid }
                } else {
                    *handle = Handle::AwaitingPasswordForPasswd { uid };
                    return Err(Error::new(EPERM));
                }
            }
            Handle::AwaitingNewPassword { uid } => {
                let mut users = AllUsers::authenticator(Config::default().writeable(true))
                    .map_err(|_| Error::new(ENOLCK))?;
                let user = users
                    .get_mut_by_id(uid as usize)
                    .ok_or(Error::new(EEXIST))?;

                let new_password = validate_utf8(buf)?;
                if user.set_passwd(&new_password).is_ok() {
                    users.save().map_err(|_| Error::new(ENOLCK))?;
                    *handle = Handle::Placeholder
                } else {
                    *handle = Handle::AwaitingNewPassword { uid };
                    return Err(Error::new(EPERM));
                }
            }

            Handle::AwaitingNamespaceFetch { .. } => {
                eprintln!("sudo: found namespace fetch handle with ID {id}");
                return Err(Error::new(EBADFD));
            }

            Handle::Placeholder => {
                eprintln!("sudo: found placeholder handle with ID {id}");
                return Err(Error::new(EBADFD));
            }

            Handle::SchemeRoot => {
                eprintln!("sudo: found Scheme root handle with ID {id}");
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
                let mut proc_fd = usize::MAX;
                req.obtain_fd(
                    socket,
                    FobtainFdFlags::empty(),
                    std::slice::from_mut(&mut proc_fd),
                )?;
                let proc_fd = unsafe { OwnedFd::from_raw_fd(proc_fd as RawFd) };

                let [ruid, euid, suid] = [0, 0, 0];
                let [rgid, egid, sgid] = [0, 0, 0];
                let mut payload = [0; size_of::<u32>() * 6];
                plain::slice_from_mut_bytes(&mut payload)
                    .unwrap()
                    .copy_from_slice(&[ruid, euid, suid, rgid, egid, sgid]);

                if let Err(err) = proc_call(
                    proc_fd.as_raw_fd() as usize,
                    &mut payload,
                    CallFlags::empty(),
                    &[ProcCall::SetResugid as u64],
                ) {
                    eprintln!("failed to setresugid: {err}");
                }

                *handle = Handle::AwaitingNamespaceFetch {
                    ns: libredox::Fd::new(
                        libredox::call::dup(libredox::call::getns().unwrap(), b"").unwrap(),
                    ),
                };
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
    // TODO: Linux kernel audit-like logging?
    let socket = Socket::create().expect("failed to open scheme socket");

    let mut state = SchemeState::new();
    let mut scheme = Scheme {
        next_fd: 1,
        handles: HashMap::new(),
    };

    register_sync_scheme(&socket, "sudo", &mut scheme)
        .expect("failed to register sudo scheme to namespace");

    loop {
        let Some(req) = socket
            .next_request(SignalBehavior::Restart)
            .expect("failed to get request")
        else {
            break;
        };

        let response = match req.kind() {
            RequestKind::Call(call) => call.handle_sync(&mut scheme, &mut state),
            RequestKind::SendFd(req) => Response::new(scheme.on_sendfd(&socket, &req), req),
            RequestKind::OnClose { id } => {
                scheme.on_close(id);
                state.on_close(id);
                continue;
            }
            _ => continue,
        };

        socket
            .write_response(response, SignalBehavior::Restart)
            .expect("sudo: scheme write failed");
    }
    std::process::exit(0)
}
