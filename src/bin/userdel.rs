#[deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate redox_users;

use std::env;
use std::io::{stdout, Write};
use std::fs::remove_dir;
use std::process::exit;

use arg_parser::ArgParser;
use extra::option::OptionalExt;
use redox_users::AllUsers;

const MAN_PAGE: &'static str =  /* @MANSTART{userdel} */ r#"
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
    let mut stdout = stdout();
    
    let mut parser = ArgParser::new(9)
        .add_flag(&["h", "help"])
        .add_flag(&["r", "remove"]);
    parser.parse(env::args());
    
    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).unwrap();
        stdout.flush().unwrap();
        exit(0);
    }
    
    let login = if parser.args.is_empty() {
        eprintln!("userdel: no login specified");
        exit(1);
    } else {
        &parser.args[0]
    };
    
    let mut sys_users = AllUsers::new().unwrap_or_exit(1);
    {
        let user = sys_users.get_by_name(login).unwrap_or_else(|| {
            eprintln!("userdel: user does not exist: {}", login);
            exit(1);
        });
        
        if parser.found("remove") {
            remove_dir(&user.home).unwrap_or_exit(1);
        }
    }
    
    sys_users.remove_by_name(login.to_string()).unwrap_or_exit(1);
    
    sys_users.save().unwrap_or_exit(1);
}
