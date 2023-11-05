#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;
extern crate userutils;

use std::fs::remove_dir;
use std::process::exit;

use extra::option::OptionalExt;
use redox_users::{All, AllGroups, AllUsers, Config};
use userutils::AllGroupsExt;

const _MAN_PAGE: &'static str =  /* @MANSTART{userdel} */ r#"
NAME
    userdel - modify system files to delete users

SYNOPSYS
    userdel [ options ] LOGIN
    userdel [ -h | --help ]

DESCRIPTION
    userdel removes users from whatever backend is employed by
    the system's redox_users. The utility removes the user from
    all groups of which they are a member.

    It can also be used to manage removal of home directories.

OPTIONS
    -h, --help
        Print this help page and exit.

    -r, --remove
        The user's home directory and all files inside will be
        removed.

AUTHORS
    Wesley Hershberger.
"#; /* @MANEND */

fn main() {
    let args = clap_app!(userdel =>
        (author: "Wesley Hershberger")
        (about: "Removes system users using redox_users")
        (@arg LOGIN: +required "Remove user LOGIN")
        (@arg REMOVE: -r --remove "Remove the user's home and all files and directories inside")
    ).get_matches();

    let login = args.value_of("LOGIN").unwrap();

    let mut sys_users = AllUsers::authenticator(Config::default().writeable(true)).unwrap_or_exit(1);
    let mut sys_groups = AllGroups::new(Config::default().writeable(true)).unwrap_or_exit(1);
    {
        sys_groups.remove_user_from_all_groups(login);

        if args.is_present("REMOVE") {
            let user = sys_users.get_by_name(login).unwrap_or_else(|| {
                eprintln!("userdel: user does not exist: {}", login);
                exit(1);
            });
            remove_dir(&user.home).unwrap_or_exit(1);
        }
    }

    sys_users.remove_by_name(login.to_string());

    sys_groups.save().unwrap_or_exit(1);
    sys_users.save().unwrap_or_exit(1);
}
