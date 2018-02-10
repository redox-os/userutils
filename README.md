# Redox OS user and group utilities.

The `userutils` crate contains the utilities for dealing with users and groups in Redox OS.
They are heavily influenced by UNIX and are, when needed, tailored to specific Redox use cases.

These implementations strive to be as simple as possible drawing particular
inspiration by BSD systems. They are indeed small, by choice.

[![Travis Build Status](https://travis-ci.org/redox-os/userutils.svg?branch=master)](https://travis-ci.org/redox-os/userutils)

**Currently included:**

- `getty`: Used by `init(8)` to open and initialize the TTY line, read a login name and invoke `login(1)`.
- `id`: Displays user identity.
- `login`: Allows users to login into the system
- `passwd`: Allows users to modify their passwords.
- `su`: Allows users to substitute identity.
- `sudo`: Enables users to execute a command as another user.
- `useradd`: Add a user
- `usermod`: Modify user information
- `userdel`: Delete a user
- `groupadd`: Add a user group
- `groupmod`: Modify group information
- `groupdel`: Remove a user group
