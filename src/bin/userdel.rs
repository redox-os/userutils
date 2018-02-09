#[deny(warnings)]

#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;

use std::fs::remove_dir;
use std::process::exit;

use extra::option::OptionalExt;
use redox_users::AllUsers;

const _MAN_PAGE: &'static str =  /* @MANSTART{userdel} */ r#"
NAME
    userdel - modify system files to delete users

SYNOPSYS
    userdel [ options ] LOGIN
    userdel [ -h | --help ]

DESCRIPTION
    userdel removes users from whatever backend is employed by
    the system's redox_users.
    
    It can also be used to manage removal of home directories.
    
    This utility does not remove the user from any groups! This is
    a planned feature and will be implemented at some point.

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
    
    let mut sys_users = AllUsers::new().unwrap_or_exit(1);
    {
        let user = sys_users.get_by_name(login).unwrap_or_else(|| {
            eprintln!("userdel: user does not exist: {}", login);
            exit(1);
        });
        
        if args.is_present("REMOVE") {
            remove_dir(&user.home).unwrap_or_exit(1);
        }
    }
    
    sys_users.remove_by_name(login.to_string()).unwrap_or_exit(1);
    
    sys_users.save().unwrap_or_exit(1);
}
