pub fn architecture_name(code: u16) -> &'static str {
    match code {
        0 => "legacy-bios",
        7 | 9 => "uefi-x86_64",
        11 | 12 => "uefi-arm64",
        _ => "unknown",
    }
}
