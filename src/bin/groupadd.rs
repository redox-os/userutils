#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate redox_users;

use extra::option::OptionalExt;

use std::{io, env};
use std::io::Write;
use std::process::exit;

use arg_parser::ArgParser;
use redox_users::{add_group, get_group_by_id, get_unique_group_id, UsersError};

const MAN_PAGE: &'static str = /* @MANSTART{groupadd} */ r#"
NAME
    groupadd - add a user group

SYNOPSIS
    groupadd [ -f | --force ] group
    groupadd [ -h | --help ]

DESCRIPTION
    The groupadd utility adds a new user group using values
    passed on the command line and system defaults.

OPTIONS
    -f, --force
        Simply forces the exit status of the program to 0
        even if the group already exists. A message is still
        printed to stdout.

    -g, --gid GID
        The group id to use. This value must not be used and must
        be non-negative. The default is to pick the smallest available
        group id (between values defined in redox_users).

    -h, --help
        Display this help and exit.

AUTHOR
    Written by Wesley Hershberger.
"#; /* @MANEND */

fn main() {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut stderr = io::stderr();

    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"])
        .add_flag(&["f", "force"])
        .add_opt("g", "gid");
    parser.parse(env::args());

    // Shows the help
    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).try(&mut stderr);
        stdout.flush().try(&mut stderr);
        exit(0);
    }

    let groupname = if parser.args.is_empty() {
        eprintln!("groupadd: no group name specified");
        exit(1);
    } else {
        &parser.args[0]
    };

    let gid = if let Some(gid) = parser.get_opt("gid") {
        let gid = gid.parse::<u32>().unwrap_or_exit(1);

        match get_group_by_id(gid as usize) {
            Ok(_) => {
                eprintln!("groupadd: group already exists");
                exit(1);
            },
            Err(ref err) if err.downcast_ref::<UsersError>() == Some(&UsersError::AlreadyExists) => {
                gid
            },
            Err(err) => {
                eprintln!("groupadd: {}", err);
                exit(1);
            }
        }
    } else {
        match get_unique_group_id() {
            Some(gid) => gid,
            None => {
                eprintln!("groupadd: no available gid");
                exit(1);
            }
        }
    };

    match add_group(groupname, gid, &[""]) {
        Ok(_) => { },
        Err(ref err) if err.downcast_ref::<UsersError>() == Some(&UsersError::AlreadyExists) && parser.found("force") => {
            exit(0);
        },
        Err(err) => {
            eprintln!("groupadd: {}", err);
            exit(1);
        }
    }
}
