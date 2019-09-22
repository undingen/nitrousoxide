use libc;
use proc_maps;
use std::process::Command;

fn get_syms_of_object(filename: &str) -> Vec<(String, u64)> {
    let output = Command::new("nm").arg(filename).output().unwrap();
    assert!(output.status.success());
    let mut syms: Vec<(String, u64)> = Vec::new();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let t: Vec<&str> = line.splitn(3, ' ').collect();
        let name = t[2].to_owned();
        let addr = u64::from_str_radix(t[0], 16);
        if addr.is_err() {
            continue;
        }

        syms.push((name, addr.unwrap()));
    }
    syms
}

pub fn get_syms() -> Vec<(String, u64)> {
    let mut syms: Vec<(String, u64)> = Vec::new();
    let mypid = unsafe { libc::getpid() };
    let r = proc_maps::get_process_maps(mypid).unwrap();
    for filename in r
        .iter()
        .filter(|&v| v.flags == "r-xp")
        .map(|v| v.filename().as_ref().unwrap())
        .filter(|&f| f.starts_with("/"))
        .filter(|&f| !f.starts_with("/lib/x86_64-linux-gnu/"))
        .filter(|&f| !f.starts_with("/usr/lib/x86_64-linux-gnu/"))
    {
        syms.extend(get_syms_of_object(filename));
    }
    syms
}
