// recho -- really echo args, bracketed with <> and with invisible chars
// made visible. Faithful Windows port of third_party/bash/support/recho.c
// (Chet Ramey, FSF): `argv[i] = <str>` per argument, control chars as ^X,
// DEL as ^?, every other byte raw. Built by scripts/true-baseline.sh
// ensure_test_helpers into bash-tests-rw/recho.exe so the rubash side
// spawns a REAL helper process instead of carrying a test-only emulation
// in the executor.
//
// rubash#767 single-implementation contract: support/recho.c is THE
// implementation; this port must render byte-identical output for
// identical argv, and scripts/true-baseline.sh enforces that with a
// helper-parity preflight (ELF vs PE od-dump diff) before any ledger run.
// Byte-exactness rides on how rubash writes argv onto the Windows command
// line (probed with an encode_wide dumper, 2026-10): every byte below
// 0x100 travels as wide char U+00nn and valid multi-byte sequences travel
// as their code points, so decoding a wire char <= 0xFF back to that one
// byte and anything above as UTF-8 reproduces the producer's byte stream.
// Lone low surrogates DC80-DCFF (the other common byte-carrier spelling)
// also decode to their byte. Rendering then follows recho.c exactly:
// bytes < 0x20 as ^X, 0x7F as ^?, 0x80-0xFF raw, stop at the first NUL.
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
            // High surrogate: only a following low surrogate gives it a
            // reading (one astral char, UTF-8 encoded). Rubash emits
            // proper pairs for astral chars, so the pair wins even when
            // the low half lands in the DC80-DCFF byte-carrier range.
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
            // Unpaired byte-carrier surrogate (WSL-interop spelling).
            out.push((unit - 0xDC00) as u8);
        } else if (0xDC00..0xE000).contains(&unit) {
            // Unpaired low surrogate outside the byte-carrier range.
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
    out
}

#[cfg(not(windows))]
fn arg_bytes(arg: &OsStr) -> Vec<u8> {
    // Unix needs no decoding: execve argv is already raw bytes.
    use std::os::unix::ffi::OsStrExt;
    arg.as_bytes().to_vec()
}

fn push_utf8(out: &mut Vec<u8>, ch: char) {
    let mut buf = [0u8; 4];
    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
}

fn push_fffd(out: &mut Vec<u8>) {
    out.extend_from_slice("\u{FFFD}".as_bytes());
}

fn strprint(out: &mut Vec<u8>, s: &[u8]) {
    // recho.c strprint walks a C string: the first NUL byte ends the
    // argument's rendering.
    for &b in s {
        if b == 0 {
            return;
        }
        if b < b' ' {
            out.push(b'^');
            out.push(b + 64);
        } else if b == 127 {
            out.push(b'^');
            out.push(b'?');
        } else {
            out.push(b);
        }
    }
}

fn main() {
    let mut out = Vec::new();
    for (index, arg) in std::env::args_os().skip(1).enumerate() {
        let bytes = arg_bytes(arg.as_os_str());
        out.extend_from_slice(format!("argv[{}] = <", index + 1).as_bytes());
        strprint(&mut out, &bytes);
        out.extend_from_slice(b">\n");
    }
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = lock.write_all(&out);
}
