extern crate argon2rs;
extern crate extra;
extern crate syscall;

use std::io::{Read, Stderr, Write};
use std::fs::File;
use std::process::exit;

use argon2rs::verifier::Encoded;
use argon2rs::{Argon2, Variant};
use extra::option::OptionalExt;

const PASSWD_FILE: &'static str = "/etc/passwd";
const GROUP_FILE: &'static str = "/etc/group";

/// A struct representing a UNIX /etc/passwd file entry
pub struct Passwd<'a> {
    pub user: &'a str,
    pub hash: &'a str,
    pub uid: u32,
    pub gid: u32,
    pub name: &'a str,
    pub home: &'a str,
    pub shell: &'a str
}

impl<'a> Passwd<'a> {
    pub fn parse(line: &'a str) -> Result<Passwd<'a>, ()> {
        let mut parts = line.split(';');

        let user = parts.next().ok_or(())?;
        let hash = parts.next().ok_or(())?;
        let uid = parts.next().ok_or(())?.parse::<u32>().or(Err(()))?;
        let gid = parts.next().ok_or(())?.parse::<u32>().or(Err(()))?;
        let name = parts.next().ok_or(())?;
        let home = parts.next().ok_or(())?;
        let shell = parts.next().ok_or(())?;

        Ok(Passwd {
            user: user,
            hash: hash,
            uid: uid,
            gid: gid,
            name: name,
            home: home,
            shell: shell
        })
    }

    pub fn parse_file(file_data: &'a str) -> Result<Vec<Passwd<'a>>, ()> {
        let mut entries: Vec<Passwd<'a>> = Vec::new();

        for line in file_data.lines() {
            if let Ok(passwd) = Passwd::parse(line) {
                entries.push(passwd);
            }
        }

        Ok(entries)
    }

    pub fn encode(password: &str, salt: &str) -> String {
        let a2 = Argon2::new(10, 1, 4096, Variant::Argon2i).unwrap();
        let e = Encoded::new(a2, password.as_bytes(), salt.as_bytes(), &[], &[]);
        String::from_utf8(e.to_u8()).unwrap()
    }

    pub fn verify(&self, password: &str) -> bool {
        let e = Encoded::from_u8(self.hash.as_bytes()).unwrap();
        e.verify(password.as_bytes())
    }
}

/// A struct representing a UNIX /etc/group file entry
pub struct Group<'a> {
    pub group: &'a str,
    pub gid: u32,
    pub users: &'a str,
}

impl<'a> Group<'a> {
    pub fn parse(line: &'a str) -> Result<Group<'a>, ()> {
        let mut parts = line.split(';');

        let group = parts.next().ok_or(())?;
        let gid = parts.next().ok_or(())?.parse::<u32>().or(Err(()))?;
        let users = parts.next().ok_or(())?;

        Ok(Group {
            group: group,
            gid: gid,
            users: users
        })
    }

    pub fn parse_file(file_data: &'a str) -> Result<Vec<Group<'a>>, ()> {
        let mut entries: Vec<Group<'a>> = Vec::new();

        for line in file_data.lines() {
            if let Ok(group) = Group::parse(line) {
                entries.push(group);
            }
        }

        Ok(entries)
    }
}

/// Gets the current process effective user id aborting the caller on error.
///
/// This function issues the `geteuid` system call returning the process effective
/// user id. In case of an error it will log message to `stderr` and then abort
/// the caller process with an non-zero exit code.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let euid = get_euid(&mut stderr);
///
/// ```
pub fn get_euid(stderr: &mut Stderr) -> usize {
    match syscall::geteuid() {
        Ok(euid) => euid,
        Err(_) => {
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(b"failed to get effective UID\n");
            let _ = stderr.flush();
            exit(1)
        }
    }
}

/// Gets the current process real user id aborting the caller on error.
///
/// This function issues the `getuid` system call returning the process real
/// user id. In case of an error it will log message to `stderr` and then abort
/// the caller process with an non-zero exit code.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let uid = get_uid(&mut stderr);
///
/// ```
pub fn get_uid(stderr: &mut Stderr) -> usize {
    match syscall::getuid() {
        Ok(euid) => euid,
        Err(_) => {
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(b"failed to get real UID\n");
            let _ = stderr.flush();
            exit(1)
        }
    }
}

/// Gets the current process effective group id aborting the caller on error.
///
/// This function issues the `getegid` system call returning the process effective
/// group id. In case of an error it will log message to `stderr` and then abort
/// the caller process with an non-zero exit code.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let egid = get_egid(&mut stderr);
///
/// ```
pub fn get_egid(stderr: &mut Stderr) -> usize {
    match syscall::getegid() {
        Ok(euid) => euid,
        Err(_) => {
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(b"failed to get effective GID\n");
            let _ = stderr.flush();
            exit(1)
        }
    }
}

/// Gets the current process real group id aborting the caller on error.
///
/// This function issues the `getegid` system call returning the process real
/// group id. In case of an error it will log message to `stderr` and then abort
/// the caller process with an non-zero exit code.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let gid = get_gid(&mut stderr);
///
/// ```
pub fn get_gid(stderr: &mut Stderr) -> usize {
    match syscall::getgid() {
        Ok(euid) => euid,
        Err(_) => {
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(b"failed to get real GID\n");
            let _ = stderr.flush();
            exit(1)
        }
    }
}

/// Gets the user name for a given user id.
///
/// This function will read `/etc/passwd` looking for an entry for the provided
/// user ID, returning its UNIX username. In case of an error it will log message
/// to `stderr` and then will the caller process with an non-zero exit code.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let user = get_user(1, &mut stderr);
///
/// ```
pub fn get_user(uid: usize, stderr: &mut Stderr) -> Option<String> {
    let mut passwd_string = String::new();
    let mut file = File::open(PASSWD_FILE).try(stderr);
    file.read_to_string(&mut passwd_string).try(stderr);

    let passwd_file_entries = Passwd::parse_file(&passwd_string).unwrap();
    let passwd = passwd_file_entries.iter()
        .find(|passwd| passwd.uid as usize == uid);

    match passwd {
        Some(passwd) => Some(String::from(passwd.user)),
        None => None
    }
}

/// Gets the UNIX group name for a given group ID.
///
/// This function will read `/etc/group` file looking for an entry for the provided
/// group ID, returning its UNIX group name. In case of an error it will log message
/// to `stderr` and then will the caller process with an non-zero exit code.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let group = get_group(1, &mut stderr);
///
/// ```
pub fn get_group(gid: usize, stderr: &mut Stderr) -> Option<String> {
    let mut group_string = String::new();
    let mut file = File::open(GROUP_FILE).try(stderr);
    file.read_to_string(&mut group_string).try(stderr);

    let group_file_entries = Group::parse_file(&group_string).unwrap();
    let group = group_file_entries.iter()
        .find(|group| group.gid as usize == gid);

    match group {
        Some(group) => Some(String::from(group.group)),
        None => None
    }
}
