#![feature(question_mark)]

extern crate octavo;

use octavo::octavo_digest::Digest;
use octavo::octavo_digest::sha3::Sha512;

pub struct Passwd<'a> {
    pub user: &'a str,
    pub hash: &'a str,
    pub uid: u32,
    pub gid: u32,
    pub name: &'a str,
    pub home: &'a str,
    pub shell: &'a str
}

impl<'a> Passwd<'a> {
    pub fn encode(password: &str) -> String {
        let mut output = vec![0; Sha512::output_bytes()];
        let mut hash = Sha512::default();
        hash.update(&password.as_bytes());
        hash.result(&mut output);
        let mut encoded = String::new();
        for b in output.iter() {
            encoded.push_str(&format!("{:X}", b));
        }
        encoded
    }

    pub fn parse(line: &'a str) -> Result<Passwd<'a>, ()> {
        let mut parts = line.split(';');

        let user = parts.next().ok_or(())?;
        let hash = parts.next().ok_or(())?;
        let uid = parts.next().ok_or(())?.parse::<u32>().or(Err(()))?;
        let gid = parts.next().ok_or(())?.parse::<u32>().or(Err(()))?;
        let name = parts.next().ok_or(())?;
        let home = parts.next().ok_or(())?;
        let shell = parts.next().ok_or(())?;

        Ok(Passwd {
            user: user,
            hash: hash,
            uid: uid,
            gid: gid,
            name: name,
            home: home,
            shell: shell
        })
    }
}

pub struct Group<'a> {
    pub group: &'a str,
    pub gid: u32,
    pub users: &'a str,
}

impl<'a> Group<'a> {
    pub fn parse(line: &'a str) -> Result<Group<'a>, ()> {
        let mut parts = line.split(';');

        let group = parts.next().ok_or(())?;
        let gid = parts.next().ok_or(())?.parse::<u32>().or(Err(()))?;
        let users = parts.next().ok_or(())?;

        Ok(Group {
            group: group,
            gid: gid,
            users: users
        })
    }
}
