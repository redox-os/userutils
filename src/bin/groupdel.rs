#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;

use extra::option::OptionalExt;
use redox_users::{All, AllGroups, Config};

const _MAN_PAGE: &'static str =  /* @MANSTART{groupdel} */ r#"
NAME
    groupdel - modify system files to delete groups

SYNOPSYS
    groupdel [ options ] GROUP
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
    let matches = clap_app!(groupdel =>
        (author: "Wesley Hershberger")
        (about: "Removes a group from the system using redox_users")
        (@arg GROUP: +required "Removes group GROUP")
    ).get_matches();

    let group = matches.value_of("GROUP").unwrap();

    let mut sys_groups = AllGroups::new(Config::default().writeable(true)).unwrap_or_exit(1);

    sys_groups.remove_by_name(group.to_string());

    sys_groups.save().unwrap_or_exit(1);
}
