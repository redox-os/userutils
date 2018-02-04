#[deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate redox_users;

use std::env;
use std::io::{stdout, Write};
use std::process::exit;

use arg_parser::ArgParser;
use extra::option::OptionalExt;
use redox_users::AllGroups;

const MAN_PAGE: &'static str =  /* @MANSTART{groupmod} */ r#"
NAME
    groupmod - modify group information

SYNOPSYS
    groupmod [ options ] GROUP
    groupmod [ -h | --help ]

DESCRIPTION
    groupmod modifies a user group GROUP in the system's
    redox_users backend.

OPTIONS
    -h, --help
        Print this help page and exit.

    -g, --gid GID
        Change GROUP's group id. GID must be a non-negative
        decimal integer.
        
        Files with GROUP's old gid will not be updated.
        
        User's who use the old gid as their primary gid will
        also not be updated. This is a TODO and will change.
    
    -n, --name NAME
        The name of the group will be set to NAME

AUTHORS
    Wesley Hershberger.
"#; /* @MANEND */

fn main() {
    let mut stdout = stdout();
    
    let mut parser = ArgParser::new(9)
        .add_flag(&["h", "help"])
        .add_opt("g", "gid")
        .add_opt("n", "name");
    parser.parse(env::args());
    
    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).unwrap();
        stdout.flush().unwrap();
        exit(0);
    }
    
    let groupname = if parser.args.is_empty() {
        eprintln!("groupmod: no login specified");
        exit(1);
    } else {
        &parser.args[0]
    };
    
    let mut sys_groups = AllGroups::new().unwrap_or_exit(1);
    {
        let group = sys_groups.get_mut_by_name(groupname).unwrap_or_else(|| {
            eprintln!("groupmod: group does not exist: {}", groupname);
            exit(1);
        });
        
        //TODO: Update user's primary GID, if gid is used as such
        if let Some(gid) = parser.get_opt("gid") {
            let gid = gid.parse::<usize>().unwrap_or_exit(1);
            group.gid = gid;
        } else if parser.found("gid") {
            eprintln!("groupmod: no gid found");
            exit(1);
        }
        
        if let Some(name) = parser.get_opt("name") {
            group.group = name;
        } else if parser.found("name") {
            eprintln!("groupmod: no name found");
            exit(1);
        }
    }
    
    sys_groups.save().unwrap_or_exit(1);
}
