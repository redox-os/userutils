#[macro_use]
extern crate clap;

use libredox::error::Result;
use std::fs::File;
use std::io::{self, Write};
use std::str;

use extra::option::OptionalExt;
use redox_users::{All, AllUsers, Config, User};
use termion::input::TermRead;
use userutils::spawn_shell;

const _MAN_PAGE: &'static str = /* @MANSTART{login} */
    r#"
NAME
    login - log into the computer

SYNOPSIS
    login

DESCRIPTION
    The login utility logs users (and pseudo-users) into the computer system.

OPTIONS

    -h --help
        Display help info and exit.

AUTHOR
    Written by Jeremy Soller, Jose Narvaez.
"#; /* @MANEND */

const ISSUE_FILE: &'static str = "/etc/issue";
const MOTD_FILE: &'static str = "/etc/motd";

// TODO: Move to redox_users once the definition solidifies.
const DEFAULT_SCHEMES: [&'static str; 26] = [
    // Kernel schemes
    "debug",
    "event",
    "memory",
    "pipe",
    "serio",
    "irq",
    "time",
    "sys",
    // Base schemes
    "rand",
    "null",
    "zero",
    "log",
    // Network schemes
    "ip",
    "icmp",
    "tcp",
    "udp",
    // IPC schemes
    "shm",
    "chan",
    "uds_stream",
    "uds_dgram",
    // File schemes
    "file",
    // Display schemes
    "display.vesa",
    "display*",
    // Other schemes
    "pty",
    "sudo",
    "audio",
];
pub fn apply_login_schemes(
    user: &User<redox_users::auth::Full>,
    default_schemes: &[&str],
) -> Result<libredox::Fd> {
    let schemes = match load_config_schemes(user) {
        Some(s) => s,
        _ => default_schemes.iter().map(|s| s.to_string()).collect(),
    };

    let mut names: Vec<ioslice::IoSlice> = Vec::with_capacity(schemes.len());
    for scheme in schemes.iter() {
        names.push(ioslice::IoSlice::new(scheme.as_bytes()));
    }

    let ns_fd = libredox::call::mkns(&names)?;
    let before_ns_fd = libredox::Fd::new(libredox::call::setns(ns_fd)?);

    Ok(before_ns_fd)
}

fn load_config_schemes(user: &User<redox_users::auth::Full>) -> Option<Vec<String>> {
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use std::fs;

    const LOGIN_SCHEMES_FILE: &'static str = "/etc/login_schemes.toml";

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct UserSchemeConfig {
        pub schemes: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct LoginConfig {
        #[serde(rename = "user_schemes")]
        pub user_schemes: BTreeMap<String, UserSchemeConfig>,
    }

    let config_str = fs::read_to_string(LOGIN_SCHEMES_FILE).ok()?;
    let config: LoginConfig = toml::from_str(&config_str).ok()?;

    config
        .user_schemes
        .get(&user.user)
        .map(|cfg| cfg.schemes.clone())
}

pub fn main() {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();

    let _args = clap_app!(login =>
        (author: "Jeremy Soller, Jose Narvaez")
        (about: "Login as a user")
    )
    .get_matches();

    if let Ok(mut issue) = File::open(ISSUE_FILE) {
        io::copy(&mut issue, &mut stdout).r#try(&mut stderr);
        stdout.flush().r#try(&mut stderr);
    }

    loop {
        let user = liner::Context::new()
            .read_line(
                liner::Prompt::from("\x1B[1mredox login:\x1B[0m "),
                None,
                &mut liner::BasicCompleter::new(Vec::<String>::new()),
            )
            .r#try(&mut stderr);

        if !user.is_empty() {
            let stdin = io::stdin();
            let mut stdin = stdin.lock();
            let sys_users = AllUsers::authenticator(Config::default()).unwrap_or_exit(1);

            match sys_users.get_by_name(user) {
                None => {
                    stdout.write(b"\nLogin incorrect\n").r#try(&mut stderr);
                    stdout.write(b"\n").r#try(&mut stderr);
                    stdout.flush().r#try(&mut stderr);
                    continue;
                }
                Some(user) => {
                    if user.is_passwd_blank() {
                        if let Ok(mut motd) = File::open(MOTD_FILE) {
                            io::copy(&mut motd, &mut stdout).r#try(&mut stderr);
                            stdout.flush().r#try(&mut stderr);
                        }

                        let before_ns_fd =
                            apply_login_schemes(user, &DEFAULT_SCHEMES).unwrap_or_exit(1);

                        let _ = syscall::fcntl(
                            before_ns_fd.raw(),
                            syscall::F_SETFD,
                            syscall::O_CLOEXEC,
                        );
                        spawn_shell(user).unwrap_or_exit(1);
                        let _ = syscall::fcntl(before_ns_fd.raw(), syscall::F_SETFD, 0);
                        let _ = libredox::call::close(
                            libredox::call::setns(before_ns_fd.into_raw()).unwrap_or_exit(1),
                        );
                        break;
                    }

                    stdout
                        .write_all(b"\x1B[1mpassword:\x1B[0m ")
                        .r#try(&mut stderr);
                    stdout.flush().r#try(&mut stderr);
                    if let Some(password) = stdin.read_passwd(&mut stdout).r#try(&mut stderr) {
                        stdout.write(b"\n").r#try(&mut stderr);
                        stdout.flush().r#try(&mut stderr);

                        if user.verify_passwd(&password) {
                            if let Ok(mut motd) = File::open(MOTD_FILE) {
                                io::copy(&mut motd, &mut stdout).r#try(&mut stderr);
                                stdout.flush().r#try(&mut stderr);
                            }

                            spawn_shell(user).unwrap_or_exit(1);
                            break;
                        }
                    }
                }
            }
        } else {
            stdout.write(b"\n").r#try(&mut stderr);
            stdout.flush().r#try(&mut stderr);
        }
    }
}
