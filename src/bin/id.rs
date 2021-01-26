#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;

use std::env::args;
use std::process::exit;

use extra::option::OptionalExt;
use redox_users::{get_egid, get_gid, get_euid, get_uid, All, AllUsers, AllGroups, Config};

const _MAN_PAGE: &'static str = /* @MANSTART{id} */ r#"
NAME
    id - display user identity

SYNOPSIS
    id
    id -g [-nr]
    id -u [-nr]
    id [ -h | --help ]

DESCRIPTION
    The id utility displays the user and group names and numeric IDs, of
    the calling process, to the standard output.

OPTIONS
    -G, --groups
        Display the different group IDs (effective and real) as white-space
        separated numbers, in no particular order.

    -g, --group
        Display the effective group ID as a number.

    -n, --name
        Display the name of the user or group ID for the -g and -u options
        instead of the number.

    -u, --user
        Display the effective user ID as a number.

    -a
        Ignored for compatibility with other id implementations.

    -r, --real
        Display the real ID for the -g and -u options instead of the effective ID.

    -h, --help
        Display help and exit.

AUTHOR
    Written by Jose Narvaez.
"#; /* @MANEND */

pub fn main() {
    let app = clap_app!(id =>
        (author: "Jose Narvaez")
        (about: "Get user and group information about the current user")
        (@arg IGNORE: -a "Ignored for compatibility with other impls of id")
        (@arg GROUPS: -G --groups conflicts_with[selector modifier] "Display current user's real and effective group id's")
        (@group selector =>
            (@arg GROUP: -g --group "Display current user's effective group id")
            (@arg USER:  -u --user  "Display the effective userid")
        )
        (@group modifier =>
            (@arg NAME: -n --name requires[selector] "Display names of groups/users instead of ids (use with -g or -u)")
            (@arg REAL: -r --real requires[selector] "Display real id's instead of effective ids (use with -g and -u)")
        )
    );

    let args = match &*args().nth(0).unwrap_or(String::new()) {
        "whoami" => app.get_matches_from(["id", "-un"].iter()),
        _ => app.get_matches()
    };

    // Display the different group IDs (effective and real)
    // as white-space separated numbers, in no particular order.
    if args.is_present("GROUPS") {
        let egid = get_egid().unwrap_or_exit(1);

        let gid = get_gid().unwrap_or_exit(1);

        println!("{} {}", egid, gid);
        exit(0);
   }

   // Display effective/real process user ID UNIX user name
   if args.is_present("USER") && args.is_present("NAME") {
        // Did they pass -r? If so, we show the real
        let uid = if args.is_present("REAL") {
            get_uid()
        } else {
            get_euid()
        }.unwrap_or_exit(1);

        let users = AllUsers::basic(Config::default()).unwrap_or_exit(1);
        let user = users.get_by_id(uid).unwrap_or_exit(1);

        println!("{}", user.user);
        exit(0);
    }

    // Display real user ID
    if args.is_present("USER") && args.is_present("REAL") {
        let uid = get_uid().unwrap_or_exit(1);

        println!("{}", uid);
        exit(0);
    }

    // Display effective user ID
    if args.is_present("USER") {
        let euid = get_euid().unwrap_or_exit(1);

        println!("{}", euid);
        exit(0);
    }

   // Display effective/real process group ID UNIX group name
   if args.is_present("GROUP") && args.is_present("NAME") {
        // Did they pass -r? If so we show the real one
        let gid = if args.is_present("REAL") {
            get_gid()
        } else {
            get_egid()
        }.unwrap_or_exit(1);

        let groups = AllGroups::new(Config::default()).unwrap_or_exit(1);
        let group = groups.get_by_id(gid).unwrap_or_exit(1);

        println!("{}", group.group);
        exit(0);
    }

    // Display the real group ID
    if args.is_present("GROUP") && args.is_present("REAL") {
        let gid = get_gid().unwrap_or_exit(1);

        println!("{}", gid);
        exit(0);
    }

    // Display effective group ID
    if args.is_present("GROUP") {
        let egid = get_egid().unwrap_or_exit(1);

        println!("{}", egid);
        exit(0);
    }

    // We get everything we can and show
    let euid = get_euid().unwrap_or_exit(1);
    let egid = get_egid().unwrap_or_exit(1);

    let users = AllUsers::basic(Config::default()).unwrap_or_exit(1);
    let groups = AllGroups::new(Config::default()).unwrap_or_exit(1);

    let user = users.get_by_id(euid).unwrap_or_exit(1);
    let group = groups.get_by_id(egid).unwrap_or_exit(1);

    println!("uid={}({}) gid={}({})", euid, user.user, egid, group.group);
    exit(0);
}
