use anyhow::{anyhow, Context};
use regex::Regex;
use std::{
    cmp::Ordering,
    collections::HashMap,
    convert::TryInto,
    env::var,
    fs::File,
    io::{BufReader, BufRead, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
};
use unicode_width::UnicodeWidthStr;

const COLOR_MOUNT_PENDING: &str = "";
const COLOR_MOUNT_WARNED: &str = "\x1B[33m";
const COLOR_MOUNT_FAILED: &str = "\x1B[31m";
const COLOR_MOUNT_OKAY: &str = "\x1B[32m";
const COLOR_RESET: &str = "\x1B[0m";

#[derive(PartialEq, Debug)]
enum Status {
    Unknown,
    Pending,
    Warned(String),
    Failed(String),
    Okay,
}

struct Host {
    local: String,
    remote: String,
    y: i32,
    status: Status,
}

struct Cursor {
    cur_y: i32,
    max_y: i32,
}

impl Cursor {
    /// Make a new Cursor. The given Y coordinate is both the number of rows in
    /// the output and the current Y coordinate.
    pub fn new(max_y: i32) -> Cursor {
        Cursor { cur_y: max_y, max_y }
    }
    /// Go to a given row of the output (0-based)
    pub fn go_to(&mut self, y: i32) {
        match y.cmp(&self.cur_y) {
            Ordering::Less => {
                print!("\x1B[{}A", self.cur_y - y);
            },
            Ordering::Equal => (),
            Ordering::Greater => {
                print!("\x1B[{}B", y - self.cur_y);
            },
        }
        self.cur_y = y;
    }
    /// Go to the end of the output.
    pub fn max_out(&mut self) {
        self.go_to(self.max_y);
    }
    /// Informs this Cursor that a newline was outputted and the real cursor
    /// has fallen down one row.
    pub fn was_bumped(&mut self) {
        self.cur_y += 1;
    }
}

/// Shorten the given string to fit the given width, and also get only the
/// first line of it...
fn shorten(mut s: &str, width: usize) -> &str {
    if let Some(pos) = s.find("\n") { s = &s[..pos] }
    while UnicodeWidthStr::width(s) > width {
        let (pos, _) = s.char_indices().rev().next().unwrap();
        s = &s[..pos];
    }
    s
}

impl Host {
    pub fn check_and_spawn(&mut self,
                           remote_path: &Path,
                           mounts: &HashMap<PathBuf,String>,
                           tx: &mpsc::Sender<(i32,Status)>) {
        assert_eq!(self.status, Status::Unknown);
        let at = remote_path.join(&self.local);
        if let Some(wat) = mounts.get(&at) {
            if wat == &self.remote {
                self.status = Status::Okay;
            }
            else {
                self.status = Status::Warned(format!("already mounted, but \
                                                      wrong source? {:?}",
                                                     wat));
            }
        }
        else {
            // Not already mounted. Mount it!
            let remote = self.remote.clone();
            let tx = tx.clone();
            let y = self.y;
            std::thread::spawn(move || {
                let output = Command::new("sshfs")
                    .arg("-o").arg("ServerAliveCountMax=3")
                    .arg("-o").arg("ServeraliveInterval=10")
                    .arg(remote)
                    .arg(at)
                    .output()
                    .expect("attempting to run sshfs command");
                if output.status.success() {
                    let _ = tx.send((y, Status::Okay));
                }
                else {
                    let _ = tx.send((y, Status::Failed(String::from_utf8_lossy(&output.stderr[..]).to_owned().to_string())));
                }
            });
            self.status = Status::Pending;
        }
        assert_ne!(self.status, Status::Unknown);
    }
    pub fn print(&self) {
        match self.status {
            Status::Unknown => println!("{}: ???", self.local),
            Status::Pending => println!("{}{}{}: ...",
                                        COLOR_MOUNT_PENDING,
                                        self.local,
                                        COLOR_RESET),
            Status::Warned(ref why) => println!("{}{}{} {}",
                                                COLOR_MOUNT_WARNED,
                                                self.local,
                                                COLOR_RESET,
                                                shorten(why,
                                                        77-self.local.len())),
            Status::Failed(ref why) => println!("{}{}{}: {}",
                                                COLOR_MOUNT_FAILED,
                                                self.local,
                                                COLOR_RESET,
                                                shorten(why,
                                                        77-self.local.len())),
            Status::Okay => println!("{}{}{}: OK ",
                                     COLOR_MOUNT_OKAY,
                                     self.local,
                                     COLOR_RESET),
        }
    }
}

fn read_hosts(path: &Path) -> anyhow::Result<Vec<Host>> {
    let mut f = BufReader::new(File::open(path)?);
    let mut buf = String::new();
    let mut ret = Vec::new();
    let host_regex
        = Regex::new("^([-A-Za-z0-9_][-A-Za-z0-9_.]*)=(.*)$").unwrap();
    while f.read_line(&mut buf)? != 0 {
        let mut line = &buf[..];
        if line.ends_with("\n") {
            line = &line[..line.len()-1];
        }
        if let Some(pos) = line.find('#') {
            line = &line[..pos];
        }
        if line == "" { continue }
        if let Some(m) = host_regex.captures(line) {
            ret.push(Host {
                local: m.get(1).unwrap().as_str().to_owned(),
                remote: m.get(2).unwrap().as_str().to_owned(),
                y: ret.len().try_into().expect("that's a lotta hosts"),
                status: Status::Unknown,
            });
        }
        else {
            eprintln!("Warning: bad line in {:?}:\n{:?}", path, line);
        }
        buf.clear();
    }
    drop(f);
    Ok(ret)
}

fn read_mounts() -> anyhow::Result<HashMap<PathBuf,String>> {
    let output = Command::new("mount").output()
        .context("getting output from mount command")?;
    if !output.status.success() {
        let stderr = std::io::stderr();
        let mut stderr = stderr.lock();
        let _ = stderr.write_all(&output.stderr[..]);
        drop(stderr);
        #[cfg(target_os="unix")]
        if let Some(status) = output.status.code() {
            return Err(anyhow!("mount exited with code {}, we can't do our \
                                job.", status));
        }
        else if let Some(signal) = output.status.signal() {
            return Err(anyhow!("mount exited due to signal {}, we can't do \
                                our job.", signal));
        }
        else {
            return Err(anyhow!("mount exited with an unknown unsuccessful \
                                status, we can't do our job."));
        }
        #[cfg(not(target_os="unix"))]
        if let Some(status) = output.status.code() {
            return Err(anyhow!("mount exited with status {}, we can't do our \
                                job.", status));
        }
        else {
            return Err(anyhow!("mount exited with an unknown unsuccessful \
                                status, we can't do our job."));
        }
    }
    let stdout = String::from_utf8_lossy(&output.stdout[..]);
    let mut ret = HashMap::new();
    let mount_regex = Regex::new("^([^ ]+) on ([^ ]+) ").unwrap();
    for line in stdout.split("\n") {
        if let Some(m) = mount_regex.captures(line) {
            let on = PathBuf::from(m.get(2).unwrap().as_str());
            let what = m.get(1).unwrap().as_str().to_owned();
            // If there already was something mounted on the given "on", this
            // will replace it. This is correct behavior, since this is how
            // multiple mounts on the same mount point work; the latest mount
            // takes effect.
            ret.insert(on, what);
        }
    }
    Ok(ret)
}

fn main() {
    let home = var("HOME").expect("No HOME set");
    let remote_path = PathBuf::from(home).join("remote");
    let mut hosts = read_hosts(&remote_path.join(".hosts")).unwrap();
    let mounts = read_mounts().unwrap();
    let (res_tx, res_rx) = mpsc::channel();
    for host in hosts.iter_mut() {
        host.check_and_spawn(&remote_path, &mounts, &res_tx);
        host.print();
    }
    drop(res_tx);
    let mut cursor = Cursor::new(hosts.len().try_into()
                                 .expect("that's a lotta hosts"));
    while let Ok((y, status)) = res_rx.recv() {
        let host = y.try_into().ok().and_then(|x: usize| hosts.get_mut(x))
            .expect("somebody sent something other than an index through the \
                     channel!");
        host.status = status;
        cursor.go_to(host.y);
        host.print();
        cursor.was_bumped();
    }
    cursor.max_out();
}
