use cir::lirc;
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use rlimit::Resource;
use serde::{Deserialize, Serialize};
use std::{
    fs::{read_dir, File},
    io,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    thread, time,
};

/*
 * This program must be run as root, otherwise lircd refuses to run. The module rc-loopback
 * must be loaded, and it's lirc chardev must be /dev/lirc0.
 *
 * For each lircd.conf file, this program start lircd, asks lircd for the list of remotes
 * and codes, and then asks lircd to transmit. The IR is captured over rc-loopback. All this
 * data is saved to a json file.
 *
 * The purpose of this is to ensure that the IR encoder of cir matches what lircd
 * would send.
 *
 * There are a few bugs in lircd which make this very awkward
 * - lircd hangs randomly when sent a transmit command, before sending a response. If this happens
 *   we kill lircd with SIGKILL and retry.
 * - lircd may send garbage before or after the response. This test tries to deal with this
 *   as best it can by trimming garbage.
 * - lircd sometimes leaks file descriptors, and it may say "too many files open". We increase the
 *   file limit because of this.
 *
 * This is not particularly elegant code it is run-once.
 */
const LIRCD_SOCKET: &str = "/var/run/lirc/lircd";

#[derive(Debug, Serialize, Deserialize)]
pub struct Remote {
    name: String,
    codes: Vec<Code>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Code {
    name: String,
    code: String,
    rawir: Vec<u32>,
}

const DEFAULT_NOFILE_LIMIT: u64 = 4096;

fn main() {
    Resource::NOFILE
        .set(DEFAULT_NOFILE_LIMIT, DEFAULT_NOFILE_LIMIT)
        .unwrap();

    recurse(&PathBuf::from("../testdata/lircd_conf"));
}

fn recurse(path: &Path) {
    for entry in read_dir(path).unwrap() {
        let e = entry.unwrap();
        let path = e.path();
        if e.metadata().unwrap().file_type().is_dir() {
            recurse(&path);
        } else if !path.ends_with(".lircmd.conf") && !path.ends_with(".testdata") {
            let mut testdata = path.clone();

            let mut filename = testdata.file_name().unwrap().to_os_string();

            filename.push(".testdata");

            testdata.set_file_name(filename);

            if testdata.exists() {
                continue;
            }

            let conf = path.to_str().unwrap();

            loop {
                println!("starting lircd {conf}");
                let mut child = Command::new("lircd")
                    .args(["-n", "-H", "default", conf])
                    .spawn()
                    .unwrap();

                thread::sleep(time::Duration::from_millis(500));

                let remotes = if let Ok(remotes) = list_remotes() {
                    remotes
                } else {
                    // maybe lircd is hanging, kill and try again
                    println!("KILL lircd and retry");
                    signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGKILL).unwrap();
                    continue;
                };

                if !remotes.is_empty() {
                    let test_data = serde_json::to_string(&remotes).unwrap();

                    println!("writing {}", testdata.display());

                    let mut file = File::create(&testdata).unwrap();

                    file.write_all(test_data.as_bytes()).unwrap();
                }

                child.kill().unwrap();
                child.wait_with_output().unwrap();
                break;
            }
        }
    }
}

fn list_remotes() -> io::Result<Vec<Remote>> {
    let (success, remote_names) = send_lirc_command("list\n")?;
    assert!(success);
    let mut remotes = Vec::new();

    for name in remote_names {
        let mut codes = Vec::new();

        println!("Remote: {name}");

        let (success, res) = send_lirc_command(&format!("list {name}\n"))?;
        assert!(success);

        for code in res {
            let list = code.split_whitespace().collect::<Vec<&str>>();

            assert_eq!(list.len(), 2);

            let file = lirc::open("/dev/lirc0").unwrap();

            let (success, _) = send_lirc_command(&format!("send_once {} {}\n", name, list[1]))?;

            println!("Code sent: {} success:{}", list[1], success);

            if success {
                let rawir = read_rc_loopback(file).unwrap();

                println!("read raw ir, {} lengths", rawir.len());

                codes.push(Code {
                    code: list[0].to_string(),
                    name: list[1].to_string(),
                    rawir,
                });
            }
        }

        if !codes.is_empty() {
            remotes.push(Remote { name, codes });
        }
    }

    Ok(remotes)
}

fn send_lirc_command(cmd: &str) -> io::Result<(bool, Vec<String>)> {
    println!("sending lirc command: {cmd}");

    let mut stream = UnixStream::connect(LIRCD_SOCKET)?;

    stream.write_all(cmd.as_bytes())?;
    stream.set_nonblocking(true)?;

    let mut result = Vec::new();

    let start = time::SystemTime::now();

    loop {
        let mut buf = [0u8; 32];
        if let Ok(size) = stream.read(&mut buf) {
            result.extend_from_slice(&buf[..size]);
        }
        // if result.ends_with(b"\nEND\n") {
        //     break;
        // }

        // lircd sometimes sends garbage after END
        if String::from_utf8_lossy(&result).contains("\nEND\n") {
            break;
        }

        if start.elapsed().unwrap().as_secs() > 600 {
            println!("read so far:{}:", String::from_utf8_lossy(&result),);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "timeout reading response on lirc domain socket",
            ));
        }

        thread::sleep(time::Duration::from_millis(100));
    }

    let s = String::from_utf8_lossy(&result);
    // lircd sometimes sends garbage before BEGIN
    let start = s.find("BEGIN\n").unwrap();

    let mut lines = Vec::new();

    let mut iter = s[start..].lines();

    assert_eq!(iter.next(), Some("BEGIN"));
    assert_eq!(iter.next(), Some(cmd.trim_end()));
    let success = match iter.next() {
        Some("SUCCESS") => true,
        Some("ERROR") => false,
        _ => unreachable!(),
    };

    match iter.next() {
        Some("DATA") => {
            let mut count = u32::from_str(iter.next().unwrap()).unwrap();

            while count > 0 {
                lines.push(iter.next().unwrap().to_string());
                count -= 1;
            }

            assert_eq!(iter.next(), Some("END"));
            // do not read anything from iter beyond this, there might be garbage
        }
        Some("END") => (),
        _ => unreachable!(),
    }

    println!("read lirc command reponse");

    Ok((success, lines))
}

fn read_rc_loopback(mut file: lirc::Lirc) -> io::Result<Vec<u32>> {
    let mut buf = Vec::with_capacity(1024);

    file.receive_raw(&mut buf)?;

    let mut rawir = Vec::with_capacity(1024);
    let mut leading_space = true;

    for e in buf {
        if e.is_timeout() {
            return Ok(rawir);
        }
        if leading_space {
            if e.is_pulse() {
                leading_space = false;
            } else if e.is_space() {
                continue;
            }
        }
        if e.is_pulse() || e.is_space() {
            rawir.push(e.value());
        }
    }

    unreachable!();
}
