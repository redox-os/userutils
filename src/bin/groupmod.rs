#[deny(warnings)]

#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;

use std::process::exit;

use extra::option::OptionalExt;
use redox_users::AllGroups;

const _MAN_PAGE: &'static str =  /* @MANSTART{groupmod} */ r#"
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
    let args = clap_app!(groupmod =>
        (author: "Wesley Hershberger")
        (about: "Modify users according to the system's redox_users backend")
        (@arg GROUP:          +required    "Modify GROUP")
        (@arg GID:  -g --gid  +takes_value "Change GROUP's group id. See man page for details")
        (@arg NAME: -n --name +takes_value "Change GROUP's name")
    ).get_matches();
    
    let groupname = args.value_of("GROUP").unwrap();
    
    let mut sys_groups = AllGroups::new().unwrap_or_exit(1);
    {
        let group = sys_groups
            .get_mut_by_name(groupname)
            .unwrap_or_else(|| {
                eprintln!("groupmod: group not found: {}", groupname);
                exit(1);
            });
        
        //TODO: Update user's primary GID, if gid is used as such
        if let Some(gid) = args.value_of("GID") {
            let gid = gid.parse::<usize>().unwrap_or_exit(1);
            group.gid = gid;
        }
        
        if let Some(name) = args.value_of("NAME") {
            group.group = name.to_string();
        }
    }
    
    sys_groups.save().unwrap_or_exit(1);
}
