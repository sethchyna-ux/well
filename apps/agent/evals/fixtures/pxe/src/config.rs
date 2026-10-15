pub const DEFAULT_HTTP_PORT: u16 = 8080;
pub const DEFAULT_TFTP_PORT: u16 = 69;

pub fn valid_range(start: u8, end: u8) -> bool {
    // BUG: equal endpoints should not be accepted.
    start <= end
}
