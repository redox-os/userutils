#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;
extern crate userutils;

use std::fs::{remove_dir, rename};
use std::process::exit;

use extra::option::OptionalExt;
use redox_users::{All, AllGroups, AllUsers, Config};
use userutils::{create_user_dir, AllGroupsExt};

const _MAN_PAGE: &'static str = /* @MANSTART{usermod} */ r#"
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
    let args = clap_app!(usermod =>
        (author: "Wesley Hershberger")
        (about: "Modify users according to the system's redox_users backend")
        (@arg LOGIN:
            +required
            "Apply modifications to LOGIN")
        (@arg COMMENT:
            -c --comment
            +takes_value
            "Set LOGIN's description (GECOS field)")
        (@arg HOME_DIR:
            -d --("home-dir")
            +takes_value
            "Create and set LOGIN's home directory")
        (@arg MOVE_HOME:
            -m --("move-home")
            requires[HOME_DIR]
            "Move LOGIN's old home to HOME_DIR (see --home-dir) instead of creating it. Requires -d")
        (@arg APPEND_GROUPS:
            -G --("append-groups")
            +takes_value conflicts_with[SET_GROUPS]
            "Add user to groups specified (comma separated list, see man page)")
        (@arg SET_GROUPS:
            -S --("set-groups")
            +takes_value conflicts_with[APPEND_GROUPS]
            "Set LOGIN's groups as specified (truncates existing, see man page)")
        (@arg GID:
            -g --gid
            +takes_value
            "Set LOGIN's primary group id. Group must exist")
        (@arg NEW_LOGIN:
            -l --login
            +takes_value
            "Set LOGIN's name to NEW_LOGIN")
        (@arg SHELL:
            -s --shell
            +takes_value
            "Set LOGIN's default login shell")
        (@arg UID:
            -u --uid
            +takes_value
            "Set LOGIN's user id. See man page for details")
    ).get_matches();

    let login = args.value_of("LOGIN").unwrap();

    //TODO: Does not always need shadowfile access
    let mut sys_users = AllUsers::authenticator(Config::default().writeable(true)).unwrap_or_exit(1);
    let mut sys_groups;

    if let Some(new_groups) = args.value_of("SET_GROUPS") {
        sys_groups = AllGroups::new(Config::default().writeable(true)).unwrap_or_exit(1);
        sys_groups.remove_user_from_all_groups(login);
        sys_groups.add_user_to_groups(login, new_groups.split(',').collect()).unwrap_or_exit(1);
        sys_groups.save().unwrap_or_exit(1);
    }

    if let Some(new_groups) = args.value_of("APPEND_GROUPS") {
        sys_groups = AllGroups::new(Config::default().writeable(true)).unwrap_or_exit(1);
        sys_groups.add_user_to_groups(login, new_groups.split(',').collect()).unwrap_or_exit(1);
        sys_groups.save().unwrap_or_exit(1);
    }

    let uid = args
        .value_of("UID")
        .map(|uid| {
            let uid = uid.parse::<usize>().unwrap_or_exit(1);
            if let Some(_user) = sys_users.get_by_id(uid) {
                eprintln!("usermod: userid already in use: {}", uid);
                exit(1);
            }
            uid
        });

    {
        let user = sys_users.get_mut_by_name(&login).unwrap_or_else(|| {
            eprintln!("usermod: user \"{}\" not found", login);
            exit(1);
        });

        if let Some(gecos) = args.value_of("COMMENT") {
            user.name = gecos.to_string();
        }

        if let Some(new_login) = args.value_of("NEW_LOGIN") {
            user.user = new_login.to_string();
        }

        if let Some(shell) = args.value_of("SHELL") {
            user.shell = shell.to_string();
        }

        if let Some(home) = args.value_of("HOME_DIR") {
            if args.is_present("MOVE_HOME") {
                rename(&user.home, &home).unwrap_or_exit(1);
            } else {
                create_user_dir(user, &home).unwrap_or_exit(1);
                remove_dir(&user.home).unwrap_or_exit(1);
            }
            user.home = home.to_string();
        }

        if let Some(uid) = uid {
            user.uid = uid;
        }

        if let Some(gid) = args.value_of("GID") {
            sys_groups = AllGroups::new(Config::default()).unwrap_or_exit(1);
            let gid = gid.parse::<usize>().unwrap_or_exit(1);

            if let Some(_group) = sys_groups.get_by_id(gid) {
                user.gid = gid;
            } else {
                eprintln!("usermod: no group found for id: {}", gid);
                exit(1);
            }
        }
    }

    sys_users.save().unwrap_or_exit(1);
}
