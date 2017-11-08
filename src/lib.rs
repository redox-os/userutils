//! Redox OS user and group utilities.
//!
//! The `userutils` crate contains the utilities for dealing with users and groups in Redox OS.
//! They are heavily influenced by UNIX and are, when needed, tailored to specific Redox use cases.
//!
//! These implementations strive to be as simple as possible drawing particular
//! inspiration by BSD systems. They are indeed small, by choice.
//!
//! The included utilities are:
//!
//! - `getty`: Used by `init(8)` to open and initialize the TTY line, read a login name and invoke `login(1)`.
//! - `id`: Displays user identity.
//! - `login`: Allows users to into the system.
//! - `passwd`: Allows users to modify their passwords.
//! - `su`: Allows users to substitute identity.
//! - `sudo`: Enables users to execute a command as another user.
//! - `whoami`: Display effective user ID.

extern crate redox_users;

use std::process::Command;
use std::os::unix::process::CommandExt;

use redox_users::User;

/// Spawns a shell for the given `User`.
///
/// The new the shell process will have set the users UID and GID, its CWD will be
/// set to the users's home directory and the follwing enviroment variables will
/// be populated like so:
///
///    - `USER` set to the user's `user` field.
///    - `UID` set to the user's `uid` field.
///    - `GROUPS` set the user's `gid` field.
///    - `HOME` set to the user's `home` field.
///    - `SHELL` set to the user's `shell` field.
///
/// # Examples
///
/// ```
/// use redox_users::get_user_by_name;
///
/// let user = get_user_by_name("goyox86");
/// spawn_shell(user);
/// ```
///
/// # Panics
///
/// This function can panic under two scenarios. The first, when an error occurs while
/// spawning the new process containig the shell and the second, when after a succesful
/// spawn, an error happens while trying to wait for the newly created process.
pub fn spawn_shell(user: User) {
    let mut command = Command::new(&user.shell);

    command.uid(user.uid);
    command.gid(user.gid);

    command.current_dir(&user.home);

    command.env("USER", &user.user);
    command.env("UID", format!("{}", user.uid));
    command.env("GROUPS", format!("{}", user.gid));
    command.env("HOME", &user.home);
    command.env("SHELL", &user.shell);

    match command.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(_status) => (),
            Err(err) => panic!("userutils: failed to wait for '{}': {}", user.shell, err)
        },
        Err(err) => panic!("userutils: failed to execute '{}': {}", user.shell, err)
    }
}
