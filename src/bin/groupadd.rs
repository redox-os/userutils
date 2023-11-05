#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;

use extra::option::OptionalExt;

use std::process::exit;

use redox_users::{All, AllGroups, Config, Error, GroupBuilder};

const _MAN_PAGE: &'static str = /* @MANSTART{groupadd} */ r#"
NAME
    groupadd - add a user group

SYNOPSIS
    groupadd [ -f | --force ] GROUP
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
    let args = clap_app!(groupadd =>
        (author: "Wesley Hershberger")
        (about: "Add groups based on the system's redox_users backend")
        (@arg GROUP: +required  "Add group GROUP")
        (@arg FORCE: -f --force "Force the status of the program to be 0 even if the group exists")
        (@arg GID:   -g --gid   +takes_value "Group id. Positive integer and must not be in use")
    ).get_matches();

    let mut sys_groups = AllGroups::new(Config::default().writeable(true)).unwrap_or_exit(1);

    let groupname = args.value_of("GROUP").unwrap();

    let gid = match args.value_of("GID") {
        Some(gid) => {
            let id = gid.parse::<usize>().unwrap_or_exit(1);
            if let Some(_group) = sys_groups.get_by_id(id) {
                eprintln!("groupadd: group already exists");
                exit(1);
            }
            id
        },
        None => sys_groups.get_unique_id().unwrap_or_else(|| {
                    eprintln!("groupadd: no available gid");
                    exit(1);
                })
    };

    let group = GroupBuilder::new(groupname).gid(gid);
    match sys_groups.add_group(group) {
        Ok(_) => (),
        Err(Error::GroupAlreadyExists) if args.is_present("FORCE") => {
            exit(0);
        },
        Err(err) => {
            eprintln!("groupadd: {}", err);
            exit(1);
        }
    }
    sys_groups.save().unwrap_or_exit(1);
}
