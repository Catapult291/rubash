//! niubash#451 / #379 regression gate: a console-less `rubash -c` run must
//! never show a console window — not for the shell and not for any external
//! child it spawns. The bug class: a console-subsystem child created by a
//! parent with no console gets a *visible* console auto-allocated by
//! Windows. Children of a detached shell take the executor's
//! CREATE_NO_WINDOW path — a *hidden* console is fine, a visible window
//! fails this gate.

#![cfg(windows)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;

type Hwnd = *mut c_void;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(cb: extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
    fn GetClassNameW(hwnd: Hwnd, buf: *mut u16, len: i32) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Process32FirstW(snap: *mut c_void, pe: *mut ProcessEntry) -> i32;
    fn Process32NextW(snap: *mut c_void, pe: *mut ProcessEntry) -> i32;
    fn CloseHandle(h: *mut c_void) -> i32;
}

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    default_heap: usize,
    module_id: u32,
    threads: u32,
    ppid: u32,
    pri_class_base: i32,
    flags: u32,
    exe_file: [u16; 260],
}

fn visible_console_window_owners() -> Vec<u32> {
    use std::sync::Mutex;
    static OWNERS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    extern "system" fn collect(hwnd: Hwnd, _lparam: isize) -> i32 {
        let mut cls = [0u16; 64];
        unsafe {
            if GetClassNameW(hwnd, cls.as_mut_ptr(), cls.len() as i32) == 0 {
                return 1;
            }
            let name =
                String::from_utf16_lossy(&cls[..cls.iter().position(|&c| c == 0).unwrap_or(64)]);
            if name == "ConsoleWindowClass" && IsWindowVisible(hwnd) != 0 {
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, &mut pid);
                OWNERS.lock().unwrap().push(pid);
            }
        }
        1
    }
    OWNERS.lock().unwrap().clear();
    unsafe {
        EnumWindows(collect, 0);
    }
    std::mem::take(&mut *OWNERS.lock().unwrap())
}

fn process_tree() -> HashMap<u32, u32> {
    let mut map = HashMap::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() || snap == -1isize as *mut c_void {
            return map;
        }
        let mut pe = ProcessEntry {
            size: std::mem::size_of::<ProcessEntry>() as u32,
            usage: 0,
            pid: 0,
            default_heap: 0,
            module_id: 0,
            threads: 0,
            ppid: 0,
            pri_class_base: 0,
            flags: 0,
            exe_file: [0; 260],
        };
        if Process32FirstW(snap, &mut pe) != 0 {
            loop {
                map.insert(pe.pid, pe.ppid);
                if Process32NextW(snap, &mut pe) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    map
}

fn in_tree(pid: u32, root: u32, tree: &HashMap<u32, u32>) -> bool {
    let mut cur = pid;
    for _ in 0..64 {
        if cur == root {
            return true;
        }
        match tree.get(&cur) {
            Some(&next) if next != cur && next != 0 => cur = next,
            _ => return false,
        }
    }
    false
}

fn collect_offenders(child_pid: u32, mut alive: impl FnMut() -> bool) -> Vec<u32> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut offenders = Vec::new();
    while alive() && Instant::now() < deadline {
        let owners = visible_console_window_owners();
        if !owners.is_empty() {
            let tree = process_tree();
            for owner in owners {
                if in_tree(owner, child_pid, &tree) && !offenders.contains(&owner) {
                    offenders.push(owner);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    offenders
}

fn spawn_detached(exe: &str, args: &[&str]) -> std::process::Child {
    let mut command = Command::new(exe);
    command
        .args(args)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.spawn().expect("spawn child")
}

#[test]
fn detached_rubash_c_spawns_no_visible_console_window() {
    let rubash = env!("CARGO_BIN_EXE_rubash");
    // ~2s of external-command work — long enough for a regression to flash.
    let mut child = spawn_detached(
        rubash,
        &["-c", "C:/Windows/System32/ping.exe -n 3 127.0.0.1 >NUL"],
    );
    let pid = child.id();
    let offenders = collect_offenders(pid, || child.try_wait().ok().flatten().is_none());
    let status = child.wait().expect("wait rubash");
    assert!(status.success(), "rubash -c ping exited {status}");
    assert!(
        offenders.is_empty(),
        "visible console windows appeared in the rubash tree (owner pids {offenders:?})"
    );
}

#[test]
fn shim_forwarding_spawns_no_visible_console_window() {
    // The bash.exe forwarder needs a host shell; skip when no niu.exe is
    // configured for this run (NIU_SHELL is the shim's own resolver key).
    let Some(niu) = std::env::var_os("NIU_SHELL") else {
        eprintln!("skip: NIU_SHELL unset — no niu.exe for the shim to forward to");
        return;
    };
    let bash = env!("CARGO_BIN_EXE_bash");
    let mut command = Command::new(bash);
    command
        .args(["-c", "C:/Windows/System32/ping.exe -n 3 127.0.0.1 >NUL"])
        .env("NIU_SHELL", niu)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn shim");
    let pid = child.id();
    let offenders = collect_offenders(pid, || child.try_wait().ok().flatten().is_none());
    let status = child.wait().expect("wait shim");
    assert!(status.success(), "bash -c ping exited {status}");
    assert!(
        offenders.is_empty(),
        "visible console windows appeared in the shim tree (owner pids {offenders:?})"
    );
}
