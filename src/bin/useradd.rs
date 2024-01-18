#[macro_use]
extern crate clap;
extern crate extra;
extern crate redox_users;
extern crate userutils;

use std::process::exit;

use extra::option::OptionalExt;
use redox_users::{All, AllGroups, AllUsers, Config, GroupBuilder, UserBuilder};
use userutils::create_user_dir;

const _MAN_PAGE: &'static str = /* @MANSTART{useradd} */ r#"
NAME
    useradd - add a new user

SYNOPSYS
    useradd [ options ] LOGIN
    useradd [ -h | --help ]

DESCRIPTION
    The useradd utility creates a new user based on
    system defaults and values passed on the command line.

    Useradd creates a new group for the user by default and
    can also be instructed to create the user's home directory.

    Note that useradd creates a new user with the password
    unset (no login). This is better documented with the
    redox_users crate.

OPTIONS
    -h, --help
        Display this help and exit.

    -c, --comment
        Any text string, usually used as the user's full name.
        Historically known as the GECOS field

    -d, --home-dir HOME_DIR
        The new user will be created with HOME_DIR as their home
        directory. The default value is LOGIN prepended with "/home/".
        This flag DOES NOT create the home directory. See --create-home

    -g, --gid GID
        The group id to use when creating the default login group. This value
        must not be in use and must be non-negative. The default is to pick the
        smallest available group id between values defined in redox_users.

    -m, --create-home
        Creates the user's home directory if it does not already exist.

        This option is not enabled by default. This option must be specified
        for a home directory to be created. If not set, the user's home dir is
        set to "/"

    -N, --no-user-group
        Do not attempt to create the user's user group. Instead, the groupid
        is set to 99 ("nobody"). -N and -g are mutually exclusive.

    -s, --shell SHELL
        The path to the user's default login shell. If not specified, the
        default shell is set as "/bin/ion"

    -u, --uid UID
        The user id to use. This value must not be in use and must be
        non-negative. The default is to pick the smallest available
        user id between the defaults defined in redox_users

AUTHORS
    Written by Wesley Hershberger.
"#; /* @MANEND */
const DEFAULT_SHELL: &'static str = "/bin/ion";
const DEFAULT_HOME: &'static str = "/home";
const DEFAULT_NO_GROUP: &'static str = "nobody";

fn main() {
    let args = clap_app!(useradd =>
        (author: "Wesley Hershberger")
        (about: "Add users based on the system's redox_users backend")
        (@arg LOGIN:
            +required
            "Add user LOGIN")
        (@arg COMMENT:
            -c --comment
            +takes_value
            "Set user description (GECOS field)")
        (@arg HOME_DIR:
            -d --("home-dir")
            +takes_value
            "Set LOGIN's home dir to HOME_DIR (does not create directory)")
        (@arg CREATE_HOME:
            -m --("create-home")
            "Create the user's home directory")
        (@arg SHELL:
            -s --shell
            +takes_value
            "Set user's default login shell")
        (@arg GID:
            -g --gid
            +takes_value
            "Set LOGIN's primary group id. Positive integer and must not be in use.")
        (@arg NO_USER_GROUP:
            -N --("no-user-group")
            conflicts_with[GID]
            "Do not create primary user group (set gid to 99, \"nobody\")")
        (@arg UID:
            -u --uid
            +takes_value
            "Set LOGIN's user id. Positive ineger and must not be in use.")
    ).get_matches();

    // unwrap is safe because of "+required". clap-rs is cool...
    let login = args.value_of("LOGIN").unwrap();

    let mut sys_users = AllUsers::authenticator(Config::default().writeable(true)).unwrap_or_exit(1);
    let mut sys_groups = AllGroups::new(Config::default().writeable(true)).unwrap_or_exit(1);

    let uid = match args.value_of("UID") {
        Some(uid) => {
            let id = uid.parse::<usize>().unwrap_or_exit(1);
            if let Some(_user) = sys_users.get_by_id(id) {
                eprintln!("useradd: userid already in use: {}", id);
                exit(1);
            }
            id
        },
        None => sys_users
                    .get_unique_id()
                    .unwrap_or_else(|| {
                        eprintln!("useradd: no available uid");
                        exit(1);
                    })
    };

    let gid = if args.is_present("NO_USER_GROUP") {
        let nobody = sys_groups
            .get_mut_by_name(DEFAULT_NO_GROUP)
            .unwrap_or_else(|| {
                eprintln!("useradd: group not found: {}", DEFAULT_NO_GROUP);
                exit(1)
            });
        nobody.users.push(login.to_string());
        99
    } else {
        let id = match args.value_of("GID") {
            Some(id) => {
                let id = id.parse::<usize>().unwrap_or_exit(1);
                if let Some(_group) = sys_groups.get_by_id(id) {
                    eprintln!("useradd: group already exists with gid: {}", id);
                    exit(1);
                }
                id
            },
            None => sys_groups
                        .get_unique_id()
                        .unwrap_or_else(|| {
                            eprintln!("useradd: no available gid");
                            exit(1);
                        })
        };
        sys_groups
            .add_group(GroupBuilder::new(login).gid(id).user(login))
            .unwrap_or_else(|err| {
                eprintln!("useradd: {}: {}", err, login);
                exit(1);
            });
        id
    };

    let gecos = args
        .value_of("COMMENT")
        .unwrap_or(login);

    //Ugly way to satisfy the borrow checker...
    let mut sys_homes = String::from(DEFAULT_HOME);
    let userhome = args
        .value_of("HOME_DIR")
        .unwrap_or_else(|| {
            if args.is_present("CREATE_HOME") {
                sys_homes.push_str("/");
                sys_homes.push_str(&login);
                sys_homes.as_str()
            } else {
                "/"
            }
        });

    let shell = args
        .value_of("SHELL")
        .unwrap_or(DEFAULT_SHELL);

    let user = UserBuilder::new(login).uid(uid).gid(gid).name(gecos).home(userhome).shell(shell);
    sys_users
        .add_user(user)
        .unwrap_or_else(|err| {
            eprintln!("useradd: {}: {}", err, login);
            exit(1);
        });

    // Make sure to try and create the user/groups before we create
    // their home, that way we get a permissions error that makes
    // more sense
    sys_groups.save().unwrap_or_exit(1);
    sys_users.save().unwrap_or_exit(1);

    if args.is_present("CREATE_HOME") {
        //Shouldn't ever error...
        let user = sys_users.get_by_id(uid).unwrap_or_exit(1);
        create_user_dir(user, userhome).unwrap_or_exit(1);
    }
}
