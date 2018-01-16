extern crate arg_parser;
extern crate extra;
extern crate redox_users;

use std::borrow::Borrow;
use std::hash::Hash;
use std::env;
use std::io::{self, Write, Stderr, StdoutLock};
use std::process::exit;

use extra::io::fail;
use extra::option::OptionalExt;
use arg_parser::{ArgParser, Param};
use redox_users::{get_egid, get_gid, get_euid, get_uid, get_user_by_id, get_group_by_id};

const HELP_INFO: &'static str = "Try ‘id --help’ for more information.\n";
const MAN_PAGE: &'static str = /* @MANSTART{id} */ r#"
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
    -G
        Display the different group IDs (effective and real) as white-space
        separated numbers, in no particular order.

    -g
        Display the effective group ID as a number.

    -n  Display the name of the user or group ID for the -g and -u options
        instead of the number.

    -u
        Display the effective user ID as a number.

    -a
        Ignored for compatibility with other id implementations.

    -r
        Display the real ID for the -g and -u options instead of the effective ID.

    -h
    --help
        Display this help and exit.

EXIT STATUS
     The id utility exits 0 on success, and >0 if an error occurs.

AUTHOR
    Written by Jose Narvaez.
"#; /* @MANEND */


pub fn main() {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut stderr = io::stderr();

    let mut parser = ArgParser::new(1)
        .add_flag(&["h", "help"])
        .add_flag(&["a"])
        .add_flag(&["G"])
        .add_flag(&["g"])
        .add_flag(&["u"])
        .add_flag(&["n"])
        .add_flag(&["r"]);
    parser.parse(env::args());

    // If the parser found the "help" tag...
    if parser.found("help") {
        print_msg(MAN_PAGE, &mut stdout, &mut stderr);
    // If the parser found invalid tags...
    } else if let Err(err) = parser.found_invalid() {
        stderr.write_all(err.as_bytes()).try(&mut stderr);
        print_msg(HELP_INFO, &mut stdout, &mut stderr);
        exit(1);
    // If the parser found G and...
    } else if parser.found(&'G') {
        //...found 'g' or 'u', which are mutually incompatible options...
        if any_of_found(&parser, &[&'g', &'u']) {
            let msg = "id: -G option must be used without others\n";
            print_msg(msg, &mut stdout, &mut stderr);
            print_msg(HELP_INFO, &mut stdout, &mut stderr);
            exit(1);
        }
        //...Nothing else.
        let egid = get_egid().unwrap_or_exit(1);
        let gid = get_gid().unwrap_or_exit(1);

        print_msg(&format!("{} {}\n", egid, gid), &mut stdout, &mut stderr);
    
    // If the parser found 'u' and...
    } else if parser.found(&'u'){
	//...'g', which is a mutually incompatible option, 
	if parser.found(&'g') {
	     let msg = "id: specify either -u or -g but not both\n";
	     print_msg(msg, &mut stdout, &mut stderr);
	     print_msg(HELP_INFO, &mut stdout, &mut stderr);
	     exit(1);
	} else {
	    //...'r', in which case, we show the real
	    let uid_result = if parser.found(&'r') {
		get_uid()
	    } else {
		get_euid() //Or not.
	    };
	    
	    //...'n', to display the effective/real process user ID UNIX user name
	    if parser.found(&'n') {
		 let uid = uid_result.unwrap_or_exit(1);
		 let user = get_user_by_id(uid).unwrap_or_exit(1);

		 print_msg(&format!("{}\n", user.user), &mut stdout, &mut stderr);
	     //...nothing else of importance, in which case we display effective user ID 
	     } else { 
		 let euid = get_euid().unwrap_or_exit(1);

		 print_msg(&format!("{}\n", euid), &mut stdout, &mut stderr);
	     }
	 }
    // If they found 'g' and...
    } else if parser.found(&'g') {
        //...'r', in which case we show the real group id
        let gid_result = if parser.found(&'r') {
            get_gid()
        } else {
            get_egid() //Or not.
        };
        
	//...'n', in which case we display process group ID UNIX group name
	if parser.found(&'n') {
	    let gid = gid_result.unwrap_or_exit(1);

	    let group = get_group_by_id(gid).unwrap_or_exit(1);

	    print_msg(&format!("{}\n", group.group), &mut stdout, &mut stderr);
	//...Nothing else of importance
	} else {
	    let gid_result = gid_result.unwrap_or_exit(1);
            print_msg(&format!("{}\n", gid_result), &mut stdout, &mut stderr);
	}
    // If they found -n, which does not apply if there is no -u or -g
    } else if parser.found(&'n') {
        let msg = "id: the -n option must be used with either -u or -g\n";
        fail(msg, &mut stderr);
    // If they found -r, which does not apply if there is no -u or -g
    } else if parser.found(&'r') {
        let msg = "id: the -r option must be used with either -u or -g\n";
        fail(msg, &mut stderr);
    //If they used no tags at all, we show everything.
    } else {
	let euid = get_euid().unwrap_or_exit(1);
	let egid = get_egid().unwrap_or_exit(1);

	let user = get_user_by_id(euid).unwrap_or_exit(1);
	let group = get_group_by_id(egid).unwrap_or_exit(1);

	let msg = format!("uid={}({}) gid={}({})\n", euid, user.user, egid, group.group);
	print_msg(&msg, &mut stdout, &mut stderr);
     }
     exit(0);
}

pub fn any_of_found<P: Hash + Eq + ?Sized>(parser: &ArgParser, flags: &[&P]) -> bool
    where Param: Borrow<P>
{
    for flag in flags {
        if parser.found(*flag) { return true }
    }

    false
}

fn print_msg(msg: &str, stdout: &mut StdoutLock, stderr: &mut Stderr) {
    stdout.write_all(msg.as_bytes()).try(stderr);
    stdout.flush().try(stderr);
}
