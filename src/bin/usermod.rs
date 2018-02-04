#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate redox_users;
extern crate userutils;

use std::env;
use std::io::{stdout, Write};
use std::fs::{remove_dir, rename};
use std::process::exit;

use arg_parser::ArgParser;
use extra::option::OptionalExt;
use redox_users::{AllGroups, AllUsers};
use userutils::create_user_dir;

const MAN_PAGE: &'static str = /* @MANSTART{usermod} */ r#"
NAME
    usermod - modify user information

SYNOPSYS
    usermod [ options ] LOGIN
    usermod [ -h | --help ]

DESCRIPTION
    The usermod utility can be used to modify user information.

    This utility uses the redox_users API, so the backend is whatever
    backend in use on the system for that API at the time.

    See passwd for setting user passwords.

OPTIONS
    -h, --help
        Display this help and exit.

    -c, --comment COMMENT
        The comment field (or GECOS, historically) for the user. This
        is typically the full name of the user, although sometimes it
        includes an e-mail.

    -d, --home-dir HOME_DIR
        Sets the home directory to HOME_DIR and creates the directory.
        See -m for move

    -m, --move-home
        Moves the the user's old home directory into the home directory
        specified by --home-dir. Has no effect if passed without --home-dir

    -G, --append-groups GROUP[,GROUP, ...]
        Add this user to GROUP groups. This does not remove the user from
        any group of which they are already a member.

    -S, --set-groups GROUP[,GROUP, ...]
        Remove the user from all groups of which they are a part and add
        them to GROUP groups.

    -g, --gid GID
        Set the user's primary group id. If the group does not exist,
        a warning is issued and no changes are applied.

    -l, --login NEW_LOGIN
        Set the new login name for the user. Must not be in use.

    -s, --shell SHELL
        Set the user's login shell as SHELL. This must be a full path.

    -u, --uid UID
        Set the user's user id. If another user's userid is the same as
        UID, a warning is issued and no changes are applied. Note that
        changing the value of the user's userid may have unexpected consequences.

AUTHORS
    Written by Wesley Hershberger.
"#; /* @MANEND */

fn main() {
    let mut stdout = stdout();
    
    let mut parser = ArgParser::new(9)
        .add_flag(&["h", "help"])
        .add_flag(&["m", "move-home"])
        .add_opt("c", "comment")
        .add_opt("d", "home-dir")
        .add_opt("G", "groups")
        .add_opt("g", "gid")
        .add_opt("l", "login")
        .add_opt("s", "shell")
        .add_opt("u", "uid");
    parser.parse(env::args());
    
    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).unwrap();
        stdout.flush().unwrap();
        exit(0);
    }
    
    let login = if parser.args.is_empty() {
        eprintln!("usermod: no login specified");
        exit(1);
    } else {
        &parser.args[0]
    };
    
    let mut sys_users = AllUsers::new().unwrap_or_exit(1);
    let mut sys_groups;
    
    if parser.found("groups") {
        sys_groups = AllGroups::new().unwrap_or_exit(1);
        
        let new_groups = parser.get_opt("groups").unwrap_or_else(|| {
            eprintln!("usermod: no groups found");
            exit(1);
        });
        let new_groups = new_groups.split(',');
        
        for groupname in new_groups {
            let group = sys_groups.get_mut_by_name(groupname).unwrap_or_else(|| {
                eprintln!("usermod: no group found: {}", groupname);
                exit(1);
            });
            group.users.push(String::from(login.as_str()));
        }
        
        sys_groups.save().unwrap_or_exit(1);
    }
    
    // Nasty to satisfy borrow checker. See line ~174 too
    let uid = if let Some(uid) = parser.get_opt("uid") {
        let uid = uid.parse::<usize>().unwrap_or_exit(1);
        if let Some(_user) = sys_users.get_by_id(uid) {
            eprintln!("usermod: userid already in use: {}", uid);
            exit(1);
        } else {
            Some(uid)
        }
    } else if parser.found("uid") {
        eprintln!("usermod: no uid found");
        exit(1);
    } else {
        None
    };
    
    {
        let user = sys_users.get_mut_by_name(&login).unwrap_or_else(|| {
            eprintln!("usermod: user \"{}\" not found", login);
            exit(1);
        });
        
        if let Some(gecos) = parser.get_opt("comment") {
            user.name = gecos;
        // If we found it but ^that^ was None, problem
        } else if parser.found("comment") {
            eprintln!("usermod: no comment found");
            exit(1);
        }
        
        if let Some(new_login) = parser.get_opt("login") {
            user.user = new_login;
        } else if parser.found("login") {
            eprintln!("usermod: no login found");
            exit(1);
        }
        
        if let Some(shell) = parser.get_opt("shell") {
            user.shell = shell;
        } else if parser.found("shell") {
            eprintln!("usermod: no shell found");
            exit(1);
        }
        
        if let Some(home) = parser.get_opt("home-dir") {
            if parser.found("move-home") {
                rename(&user.home, &home).unwrap_or_exit(1);
            } else {
                create_user_dir(user, &home).unwrap_or_exit(1);
                remove_dir(&user.home).unwrap_or_exit(1);
            }
            user.home = home;
        } else if parser.found("home-dir") {
            eprintln!("usermod: no home dir found");
            exit(1);
        }
        
        if let Some(uid) = uid {
            user.uid = uid;
        }
        
        if let Some(gid) = parser.get_opt("gid") {
            sys_groups = AllGroups::new().unwrap_or_exit(1);
            let gid = gid.parse::<usize>().unwrap_or_exit(1);
            
            if let Some(_group) = sys_groups.get_by_id(gid) {
                user.gid = gid;
            } else {
                eprintln!("usermod: no group found for id: {}", gid);
            }
        } else if parser.found("gid") {
            eprintln!("usermod: no gid found");
            exit(1);
        }
    }
    
    sys_users.save().unwrap_or_exit(1);
}
