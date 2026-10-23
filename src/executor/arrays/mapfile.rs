use crate::lexer::ansi::is_assignment_carrier_byte;

pub(in crate::executor) fn split_mapfile_input(
    input: &str,
    delimiter: Option<u8>,
    trim_delimiter: bool,
) -> Vec<String> {
    use crate::executor::substitution_metadata::{bytes_to_shell_text, shell_text_to_raw_bytes};

    // GNU builtins/mapfile.def:177-209: the zgetline loop splits raw input
    // bytes on the delimiter and bind_array_element stores each line, with
    // no per-line re-encode. Decode once, split the raw bytes as ranges
    // (no per-byte push buffer), strip `-t` in the byte domain, and encode
    // each segment exactly once (subst.c raw-byte marker contract keeps
    // `-d $'\xff'` byte-exact).
    let delim = delimiter.unwrap_or(b'\n');
    // Fast path (rubash#680 slice 2, rescue/stash-perf-680 merge): when the
    // delimiter is a plain non-NUL ASCII byte and the input carries no
    // text-layer carrier byte and no 0xEE marker-zone lead, the decode is
    // the identity (nothing to unescape) and the per-segment re-encode is
    // the identity too (`bytes_to_shell_text` only rewrites carrier bytes
    // and raw-byte markers — a Rust String input is always valid UTF-8, an
    // ASCII delimiter can never cut a multi-byte sequence, and a valid
    // UTF-8 segment re-encodes to itself because only carrier bytes make
    // `push_escaped_text_with_carriers` differ from its input). Splitting
    // the text directly then yields byte-identical values with one alloc
    // per segment instead of a whole-input decode plus a per-segment
    // encode scan — UTF-8 (CJK, accented) payloads ride the fast path,
    // not just ASCII. The gate is the AUTHORITATIVE carrier set
    // (`is_assignment_carrier_byte` — {0x0c, CTLESC 0x11, 0x13, 0x14..=0x1f},
    // lexer/ansi.rs) plus the 0xEE lead of the U+E000..U+EFFF registry
    // marker zone, so a future carrier registration automatically
    // tightens the gate; NUL stays on the decode path by contract
    // (delim != 0, rubash#117) and non-carrier C0 bytes (tab/LF/CR/VT…)
    // encode to themselves.
    if delim != 0
        && delim.is_ascii()
        && !is_assignment_carrier_byte(u32::from(delim))
        && input
            .bytes()
            .all(|b| !is_assignment_carrier_byte(u32::from(b)) && b != 0xEE)
    {
        let strip = trim_delimiter;
        let mut values = Vec::new();
        values.reserve(std::cmp::min(input.len() / 8, 65_536) + 1);
        let mut start = 0;
        for (index, &byte) in input.as_bytes().iter().enumerate() {
            if byte != delim {
                continue;
            }
            let end = if strip { index } else { index + 1 };
            // GNU mapfile.def: input is C-string, truncate at first NUL
            // (rubash#1420).
            let segment = &input[start..end];
            let value = match segment.find('\0') {
                Some(pos) => segment[..pos].to_string(),
                None => segment.to_string(),
            };
            values.push(value);
            start = index + 1;
        }
        if start < input.len() {
            let segment = &input[start..];
            let value = match segment.find('\0') {
                Some(pos) => segment[..pos].to_string(),
                None => segment.to_string(),
            };
            values.push(value);
        }
        return values;
    }
    let bytes = shell_text_to_raw_bytes(input);
    if bytes.is_empty() {
        return Vec::new();
    }
    let strip = trim_delimiter || delim == 0;
    let mut values = Vec::new();
    values.reserve(std::cmp::min(bytes.len() / 8, 65_536) + 1);
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        if byte != delim {
            continue;
        }
        let end = if strip { index } else { index + 1 };
        // GNU mapfile.def: truncate at first NUL (rubash#1420), except when
        // NUL is the delimiter (delim==0, already handled by `strip`).
        let segment = &bytes[start..end];
        let filtered: Vec<u8> = if delim == 0 {
            segment.to_vec()
        } else if let Some(pos) = segment.iter().position(|&b| b == 0) {
            segment[..pos].to_vec()
        } else {
            segment.to_vec()
        };
        values.push(bytes_to_shell_text(&filtered));
        start = index + 1;
    }
    if start < bytes.len() {
        let segment = &bytes[start..];
        let filtered: Vec<u8> = if delim == 0 {
            segment.to_vec()
        } else if let Some(pos) = segment.iter().position(|&b| b == 0) {
            segment[..pos].to_vec()
        } else {
            segment.to_vec()
        };
        values.push(bytes_to_shell_text(&filtered));
    }
    values
}
