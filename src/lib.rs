extern crate argon2rs;

use argon2rs::verifier::Encoded;
use argon2rs::{Argon2, Variant};

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

    pub fn encode(password: &str, salt: &str) -> String {
        let a2 = Argon2::new(10, 1, 4096, Variant::Argon2i).unwrap();
        let e = Encoded::new(a2, password.as_bytes(), salt.as_bytes(), &[], &[]);
        String::from_utf8(e.to_u8()).unwrap()
    }

    pub fn verify(&self, password: &str) -> bool {
        let e = Encoded::from_u8(self.hash.as_bytes()).unwrap();
        e.verify(password.as_bytes())
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
