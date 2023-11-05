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
extern crate libredox;

use std::io::Result as IoResult;

use redox_users::{auth, All, AllGroups, Result, User, Error};
use libredox::call::{open, fchown};
use libredox::error::Result as SysResult;
use libredox::flag::{O_CREAT, O_DIRECTORY, O_CLOEXEC};

const DEFAULT_MODE: u16 = 0o700;

// Not the prettiest thing in the world, but some functionality here makes
// some of the utils much less gross
pub trait AllGroupsExt {
    fn add_user_to_groups(&mut self, login: &str, groups: Vec<&str>) -> Result<()>;
    fn remove_user_from_all_groups(&mut self, login: &str);
}

impl AllGroupsExt for AllGroups {
    // new_groups is a comma separated list of groupnames
    fn add_user_to_groups(&mut self, login: &str, new_groups: Vec<&str>) -> Result<()> {
        for groupname in new_groups {
            let group = match self.get_mut_by_name(groupname) {
                Some(group) => group,
                None => return Err(Error::UserNotFound)
            };
            group.users.push(login.to_string());
        }
        Ok(())
    }

    /// Remove a user from all groups of which they are a member
    fn remove_user_from_all_groups(&mut self, login: &str) {
        for group in self.iter_mut() {
            let op_pos = group.users.iter()
                .position(|username| username == login );
            if let Some(indx) = op_pos {
                group.users.remove(indx);
            }
        }
    }
}

/// Spawns a shell for the given `User`.
///
/// This function wraps the shell_cmd function of the User struct
/// from redox_users and manages the child process. It is a blocking
/// operation.
///
/// # Examples
///
/// ```
/// use redox_users::AllUsers;
///
/// let sys_users = AllUsers::new().unwrap();
/// let user = sys_users.get_by_name("goyox86");
/// spawn_shell(user).unwrap();
/// ```
pub fn spawn_shell(user: &User<auth::Full>) -> IoResult<i32> {
    let mut command = user.shell_cmd();

    let mut child = command.spawn()?;
    match child.wait()?.code() {
        Some(code) => Ok(code),
        None => Ok(1)
    }
}

/// Creates a directory with 700 user:user permissions
pub fn create_user_dir<T>(user: &User<auth::Full>, dir: T) -> SysResult<()>
    where T: AsRef<str> + std::convert::AsRef<[u8]>
{
    let fd = open(dir, O_CREAT | O_DIRECTORY | O_CLOEXEC, DEFAULT_MODE)?;
    fchown(fd, user.uid as u32, user.gid as u32)?;
    Ok(())
}
