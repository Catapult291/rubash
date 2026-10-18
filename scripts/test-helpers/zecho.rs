// zecho - bare-bones echo. Faithful Windows port of
// third_party/bash/support/zecho.c (FSF): arguments space-separated with a
// single trailing newline, no option or escape processing. Built by
// scripts/true-baseline.sh ensure_test_helpers into bash-tests-rw/zecho.exe.
//
// rubash#767 single-implementation contract: support/zecho.c is THE
// implementation; this port must render byte-identical output for
// identical argv (enforced by the helper-parity preflight in
// scripts/true-baseline.sh). argv byte decoding mirrors recho.rs -- see
// the wire-model comment there.
use std::ffi::OsStr;
use std::io::Write;

#[cfg(windows)]
fn arg_bytes(arg: &OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = arg.encode_wide().collect();
    let mut out = Vec::with_capacity(wide.len());
    let mut units = wide.iter().copied().peekable();
    while let Some(unit) = units.next() {
        if (0xD800..0xDC00).contains(&unit) {
            match units.peek() {
                Some(&low) if (0xDC00..0xE000).contains(&low) => {
                    units.next();
                    let c = 0x10000 + ((unit - 0xD800) as u32) * 0x400 + (low - 0xDC00) as u32;
                    match char::from_u32(c) {
                        Some(ch) => push_utf8(&mut out, ch),
                        None => push_fffd(&mut out),
                    }
                }
                _ => push_fffd(&mut out),
            }
        } else if (0xDC80..=0xDCFF).contains(&unit) {
            out.push((unit - 0xDC00) as u8);
        } else if (0xDC00..0xE000).contains(&unit) {
            push_fffd(&mut out);
        } else if unit <= 0xFF {
            out.push(unit as u8);
        } else {
            match char::from_u32(unit as u32) {
                Some(ch) => push_utf8(&mut out, ch),
                None => push_fffd(&mut out),
            }
        }
    }
    // zecho.c walks C strings: an embedded NUL ends that argument's bytes.
    out.truncate(out.iter().position(|&b| b == 0).unwrap_or(out.len()));
    out
}

#[cfg(not(windows))]
fn arg_bytes(arg: &OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = arg.as_bytes().to_vec();
    bytes.truncate(bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len()));
    bytes
}

fn push_utf8(out: &mut Vec<u8>, ch: char) {
    let mut buf = [0u8; 4];
    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
}

fn push_fffd(out: &mut Vec<u8>) {
    out.extend_from_slice("\u{FFFD}".as_bytes());
}

fn main() {
    let mut out = Vec::new();
    let mut first = true;
    for arg in std::env::args_os().skip(1) {
        if !first {
            out.push(b' ');
        }
        first = false;
        out.extend_from_slice(&arg_bytes(arg.as_os_str()));
    }
    out.push(b'\n');
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = lock.write_all(&out);
}
