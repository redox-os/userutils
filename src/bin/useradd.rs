#![deny(warnings)]

extern crate arg_parser;
extern crate redox_users;

use std::{env, io};
use std::io::Write;
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::process::exit;

use arg_parser::ArgParser;
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

    -m, --create-home
        Creates the user's home directory if it does not already exist.
        
        This option is not enabled by default. This option must be specified
        for a home directory to be created.

    -N, --no-user-group
        Do not attempt to create the user's user group.

    -s, --shell SHELL
        The path to the user's default login shell. If left blank, the
        default shell is set as /bin/ion

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
        .add_flag(&["m", "create-home"])
        .add_flag(&["N", "no-user-group"])
        .add_opt("s", "shell");
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
    
    let uid = match get_unique_user_id() {
        Some(id) => id,
        None => {
            eprintln!("useradd: no available uid");
            exit(1);
        }
    };
    
    let gid = match get_unique_group_id() {
        Some(id) => id,
        None => {
            eprintln!("useradd: no available gid");
            exit(1);
        }
    };
    
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
                eprintln!("useradd: invalid argument: -d");
                exit(1);
            }
        }
    } else {
        format!("{}/{}", DEFAULT_HOME, login)
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
    
    if !parser.found("no-user-group") {
        match add_group(login, gid, &[login]) {
            Ok(_) => {},
            Err(err) => {
                eprintln!("useradd: error creating group {}: {}", login, err);
                exit(1);
            }
        }
    }
    
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
