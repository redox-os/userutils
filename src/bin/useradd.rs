#![deny(warnings)]

extern crate arg_parser;
extern crate extra;
extern crate redox_users;

use std::{env, io};
use std::io::Write;
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::process::exit;

use arg_parser::ArgParser;
use extra::option::OptionalExt;
use redox_users::{add_group, add_user, get_unique_group_id, get_unique_user_id};

const MAN_PAGE: &'static str = /* @MANSTART{useradd} */ r#"
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

OPTIONS
    -h, --help
        Display this help and exit.

    -c, --comment
        Any text string, usually used as the user's full name.

    -d, --home-dir HOME_DIR
        The new user will be created with HOME_DIR as their home
        directory. The default value is LOGIN prepended with "/home".
        This flag DOES NOT create the home directory. See --create-home.

    -g, --gid GID
        The group id to use for the default login group. This value must
        not be in use and must be non-negative. The default is to pick the
        smallest available group id between values defined in redox_users.

    -m, --create-home
        Creates the user's home directory if it does not already exist.
        
        This option is not enabled by default. This option must be specified
        for a home directory to be created. If not set, the user's home dir is
        set to "/".

    -N, --no-user-group
        Do not attempt to create the user's user group. Instead, the groupid
        is set to 99 (should be the "nobody" group).

    -s, --shell SHELL
        The path to the user's default login shell. If left blank, the
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
const DEFAULT_MODE: u32 = 0o700;

fn main() {
    let mut stdout = io::stdout();
    
    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"])
        .add_opt("c", "comment")
        .add_opt("d", "home-dir")
        .add_opt("g", "gid")
        .add_flag(&["m", "create-home"])
        .add_flag(&["N", "no-user-group"])
        .add_opt("s", "shell")
        .add_opt("u", "uid");
    parser.parse(env::args());
    
    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes()).unwrap();
        stdout.flush().unwrap();
        exit(0);
    }
    
    let login = if parser.args.is_empty() {
        eprintln!("useradd: no login specified");
        exit(1);
    } else {
        &parser.args[0]
    };
    
    let uid = if parser.found("uid") {
        match parser.get_opt("uid") {
            Some(uid) => uid.parse::<u32>().unwrap_or_exit(1),
            None => {
                eprintln!("useradd: missing uid value");
                exit(1);
            }
        }
    } else {
        match get_unique_user_id() {
            Some(id) => id,
            None => {
                eprintln!("useradd: no available uid");
                exit(1);
            }
        }
    };
    
    //This is a ridiculous mess and could use reworking
    let gid: u32;
    if parser.found("no-user-group") {
        gid = 99;
        //TODO: Add this user to the "nobody" group
    } else {
        if parser.found("gid") {
            gid = match parser.get_opt("gid") {
                Some(gid) => gid.parse::<u32>().unwrap_or_exit(1),
                None => {
                    eprintln!("useradd: missing gid argument");
                    exit(1);
                }
            };
        } else {
            gid = match get_unique_group_id() {
                Some(id) => id,
                None => {
                    eprintln!("useradd: no available gid");
                    exit(1);
                }
            };
        }
        match add_group(login, gid, &[login]) {
            Ok(_) => {},
            Err(err) => {
                eprintln!("useradd: error creating group {}: {}", login, err);
                exit(1);
            }
        }
    }
    
    let username = if parser.found("comment") {
        match parser.get_opt("comment") {
            Some(user) => user,
            None => {
                eprintln!("useradd: invalid argument: -c");
                exit(1);
            }
        }
    } else {
        login.to_owned()
    };
    
    let userhome = if parser.found("home-dir") {
        match parser.get_opt("home-dir") {
            Some(dir) => dir,
            None => {
                eprintln!("useradd: missing directory argument");
                exit(1);
            }
        }
    } else if parser.found("create-home") {
        format!("{}/{}", DEFAULT_HOME, login)
    } else {
        "/".to_string()
    };
    
    let shell = if parser.found("shell") {
        match parser.get_opt("shell") {
            Some(sh) => sh,
            None => {
                eprintln!("useradd: invalid argument: -s");
                exit(1);
            }
        }
    } else {
        DEFAULT_SHELL.to_string()
    };
    
    match add_user(login, uid, gid, username.as_str(), userhome.as_str(), shell.as_str()) {
        Ok(_) => {},
        Err(err) => {
            eprintln!("useradd: {}: {}", err, login);
            exit(1);
        }
    }
    
    if parser.found("create-home") {
        let mut builder = DirBuilder::new();
        builder.mode(DEFAULT_MODE);
        
        match builder.create(&userhome) {
            Ok(_) => {},
            Err(ref err) if err.kind() == io::ErrorKind::AlreadyExists => {},
            Err(err) => {
                eprintln!("useradd: failed to create home dir: {}", err);
                exit(1);
            }
        };
    }
}
