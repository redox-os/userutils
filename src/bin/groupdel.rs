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

const MAN_PAGE: &'static str =  /* @MANSTART{groupdel} */ r#"
NAME
    groupdel - modify system files to delete groups

SYNOPSYS
    groupdel [ options ] LOGIN
    groupdel [ -h | --help ]

DESCRIPTION
    groupdel removes groups from whatever backend is employed by
    the system's redox_users.

    Note that you should not remove a primary user group before
    removing the user. It is also generally wise not to remove
    groups that still own files on the system.

OPTIONS
    -h, --help
        Print this help page and exit.

AUTHORS
    Wesley Hershberger.
"#; /* @MANEND */

fn main() {
    let mut stdout = stdout();
    
    let mut parser = ArgParser::new(9)
        .add_flag(&["h", "help"]);
    parser.parse(env::args());
    
    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).unwrap();
        stdout.flush().unwrap();
        exit(0);
    }
    
    let group = if parser.args.is_empty() {
        eprintln!("groupdel: no login specified");
        exit(1);
    } else {
        &parser.args[0]
    };
    
    let mut sys_groups = AllGroups::new().unwrap_or_exit(1);
    
    sys_groups.remove_by_name(group.to_string()).unwrap_or_exit(1);
    
    sys_groups.save().unwrap_or_exit(1);
}
