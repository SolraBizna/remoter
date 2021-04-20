This is a utility I wrote for myself to manage a bunch of SSHFS mounts. It pays lip service to the idea that it should work on non-UNIX, but in practice it requires a UNIX-like `mount` command and a UNIX-y idea of mounts in general, so it is probably useless on Windows.

# Requirements

- Recent Rust compiler
- SSHFS (installed as `sshfs`)
- An ANSI-compliant terminal, preferably color (these days, you can assume any terminal is ANSI-compliant)

# Building

```
cd remoter
cargo build --release
cp target/release/remoter ~/bin
```

(or, instead of `~/bin`, some other directory where you put utilities)

# Setup

Create a `remote` directory in your home folder. (This path is hardcoded, sorry.) Make a `.hosts` file inside it with your favorite editor:

```ini
# Any portion of a line after # is a comment.
# Blank (or fully commented out) lines are ignored.

# Lines take the following basic form:
dir=host:

# Simple example: I have a directory named "axl", and I want it to lead to the
# default directory on the remote host "10.0.0.2"
axl=10.0.0.2:
# I have a directory named "heracles", and I want it to lead to the default
# directory on the remote host "high-max", but I need to log in as the user
# "livingroom"
heracles=livingroom@high-max:
# I have a directory named "knockout", and I want it to lead to the ROOT
# directory on the remote host "knockout"
knockout=knockout:/
```

You also need to create the mount points yourself. (remoter doesn't do it automatically because I don't like the idea of creating directories you didn't want.) The above example `.hosts` file expects directories named `dir`, `axl`, `heracles`, and `knockout`. (Missing directories will lead that individual mount to fail.)

# Usage

Just run `remoter`. It has one line of output for each host in your `.hosts` file. It attempts each mount in parallel, and displays the running status of each mount on its particular line. Example output:

```
dir: sshfs: bad mount point `/home/sbizna/remote/dir': No such file or director
axl: ...
heracles: OK
knockout: OK
```

`dir` mount failed, `heracles` and `knockout` mounts are OK, and the attempt to mount `axl` is still pending. Those names will be red, white, and green, respectively, if your terminal supports color. If all goes well, you will see each white line gradually turn green (or red, if that host is down at the moment). `remoter` will exit when there are no pending mounts left.

Notice that "directory" is cut off in the line about `dir`; For simplicity, `remoter` assumes that your terminal is 80 columns wide, and also assumes that it can only use 79 of those columns for display. A later version may query the terminal width...

`remoter` is smart enough not to try to mount directories that are already mounted. If it sees that a given directory is mounted, but it doesn't look like it's mounted from the correct place, you might see a yellow name and output like:

```
zephyr: already mounted, but wrong source? "Desert-Wind.local:/somedir"
```

Cases like these are for you to untangle. In this example, it appears that someone manually mounted `Desert-Wind.local:/somedir` on that directory; if `Desert-Wind.local` is another name for `zephyr`, and `/somedir` is the intended location to mount, then there's no harm done. In other cases, `fusermount -u ~/remote/zephyr` will remove the existing mount, and empower `remoter` to mount the right location in its place.

# Okay, but why?

I work with a lot of computers. Some of them aren't always available. It's handy to have a one-line command I can input that boils down to "automatically sshfs-mount every computer that's currently online". Originally, I had a very dirty utility written in Lua to do this. I decided to try my hand at making a Rust replacement that was more portable, and could do the mounts in parallel (so that one machine being unavailable doesn't stop it from even *trying* to mount any others until it times out). And then I did. And then I put it on GitHub. And then it was now, and then I don't know what happened.

# License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
