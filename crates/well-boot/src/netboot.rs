//! RFC 1350 / 2348 / 2349 / 7440 TFTP option negotiation and RFC 9110 HTTP byte-range resolution
//! for zero-copy iPXE / UEFI netboot streaming of AgentOS kernels and ESP images.

/// TFTP Read Request (RRQ) opcode (RFC 1350).
pub const TFTP_OPCODE_RRQ: u16 = 1;
/// TFTP Data (DATA) opcode (RFC 1350).
pub const TFTP_OPCODE_DATA: u16 = 3;
/// TFTP Acknowledgment (ACK) opcode (RFC 1350).
pub const TFTP_OPCODE_ACK: u16 = 4;
/// TFTP Error (ERROR) opcode (RFC 1350).
pub const TFTP_OPCODE_ERROR: u16 = 5;
/// TFTP Option Acknowledgment (OACK) opcode (RFC 2347).
pub const TFTP_OPCODE_OACK: u16 = 6;

/// Default RFC 1350 TFTP data payload size in bytes.
pub const TFTP_DEFAULT_BLKSIZE: usize = 512;
/// Minimum negotiated TFTP block size permitted by AgentOS PXE policy.
pub const TFTP_MIN_BLKSIZE: usize = 512;
/// Maximum negotiated TFTP block size (fits standard 1500-byte Ethernet MTU without IP fragmentation).
pub const TFTP_MAX_BLKSIZE: usize = 1468;
/// Minimum RFC 7440 window size.
pub const TFTP_MIN_WINDOWSIZE: u16 = 1;
/// Maximum RFC 7440 window size permitted by AgentOS PXE policy.
pub const TFTP_MAX_WINDOWSIZE: u16 = 16;

/// Negotiated parameters for an active TFTP RRQ transfer session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TftpTransferSession {
    pub filename: String,
    pub blksize: usize,
    pub windowsize: u16,
    pub total_size: usize,
}

/// Parses consecutive NUL-terminated fields from a TFTP packet body.
fn split_nul_fields(mut bytes: &[u8]) -> Result<Vec<&str>, &'static str> {
    let mut fields = Vec::new();
    while !bytes.is_empty() {
        let Some(nul_pos) = bytes.iter().position(|&b| b == 0) else {
            return Err("unterminated NUL field in TFTP packet");
        };
        let field = core::str::from_utf8(&bytes[..nul_pos])
            .map_err(|_| "non-UTF8 field in TFTP packet")?;
        fields.push(field);
        bytes = &bytes[nul_pos + 1..];
    }
    Ok(fields)
}

/// Validates and negotiates an incoming TFTP RRQ packet against an artifact of `artifact_size` bytes.
///
/// Supports RFC 2348 (`blksize`, clamped to `512..=1468`), RFC 2349 (`tsize`), and
/// RFC 7440 (`windowsize`, clamped to `1..=16`). Returns the initialized transfer session
/// along with an encoded `OACK` packet when one or more supported options were acknowledged,
/// or `None` when falling back to classic RFC 1350 `DATA #1` delivery.
pub fn negotiate_tftp_rrq(
    packet: &[u8],
    artifact_size: usize,
) -> Result<(TftpTransferSession, Option<Vec<u8>>), &'static str> {
    if packet.len() < 4 {
        return Err("TFTP RRQ packet too short");
    }
    let opcode = u16::from_be_bytes([packet[0], packet[1]]);
    if opcode != TFTP_OPCODE_RRQ {
        return Err("expected TFTP RRQ opcode");
    }

    let fields = split_nul_fields(&packet[2..])?;
    if fields.len() < 2 {
        return Err("missing filename or mode in TFTP RRQ");
    }

    let filename = fields[0];
    if filename.is_empty() || filename.contains("..") {
        return Err("invalid or unsafe TFTP filename");
    }
    let mode = fields[1];
    if !mode.eq_ignore_ascii_case("octet") && !mode.eq_ignore_ascii_case("netascii") {
        return Err("unsupported TFTP transfer mode");
    }

    let opt_tokens = &fields[2..];
    if opt_tokens.len() % 2 != 0 {
        return Err("odd number of TFTP option tokens");
    }

    let mut blksize = TFTP_DEFAULT_BLKSIZE;
    let mut windowsize: u16 = TFTP_MIN_WINDOWSIZE;
    let mut oack_pairs: Vec<(&'static str, String)> = Vec::new();

    for pair in opt_tokens.chunks_exact(2) {
        let opt_name = pair[0];
        let opt_val = pair[1];

        if opt_name.eq_ignore_ascii_case("blksize") {
            let parsed = opt_val
                .parse::<usize>()
                .map_err(|_| "invalid TFTP blksize value")?;
            if parsed < 8 {
                return Err("TFTP blksize below RFC 2348 minimum");
            }
            blksize = parsed.clamp(TFTP_MIN_BLKSIZE, TFTP_MAX_BLKSIZE);
            oack_pairs.push(("blksize", blksize.to_string()));
        } else if opt_name.eq_ignore_ascii_case("windowsize") {
            let parsed = opt_val
                .parse::<u16>()
                .map_err(|_| "invalid TFTP windowsize value")?;
            if parsed == 0 {
                return Err("TFTP windowsize cannot be zero");
            }
            windowsize = parsed.clamp(TFTP_MIN_WINDOWSIZE, TFTP_MAX_WINDOWSIZE);
            oack_pairs.push(("windowsize", windowsize.to_string()));
        } else if opt_name.eq_ignore_ascii_case("tsize") {
            let _ = opt_val
                .parse::<usize>()
                .map_err(|_| "invalid TFTP tsize value")?;
            oack_pairs.push(("tsize", artifact_size.to_string()));
        }
    }

    let oack_packet = if oack_pairs.is_empty() {
        None
    } else {
        let mut pkt = Vec::with_capacity(64);
        pkt.extend_from_slice(&TFTP_OPCODE_OACK.to_be_bytes());
        for (name, val) in &oack_pairs {
            pkt.extend_from_slice(name.as_bytes());
            pkt.push(0);
            pkt.extend_from_slice(val.as_bytes());
            pkt.push(0);
        }
        Some(pkt)
    };

    let session = TftpTransferSession {
        filename: filename.to_string(),
        blksize,
        windowsize,
        total_size: artifact_size,
    };
    Ok((session, oack_packet))
}

/// Encodes RFC 1350 TFTP `DATA` packet `block_number` (1-based) using `blksize`.
///
/// Properly handles exact-multiple artifacts by returning a 4-byte (zero-payload)
/// terminal `DATA` packet when `start == artifact.len()` and `artifact.len() % blksize == 0`,
/// and returns `None` once transfer termination has already been reached.
pub fn build_tftp_data_block(
    artifact: &[u8],
    blksize: usize,
    block_number: u16,
) -> Option<Vec<u8>> {
    if blksize == 0 || block_number == 0 {
        return None;
    }
    let start = usize::from(block_number - 1).checked_mul(blksize)?;
    if start > artifact.len() {
        return None;
    }
    if start == artifact.len() && !artifact.is_empty() && artifact.len() % blksize != 0 {
        return None;
    }

    let end = start.saturating_add(blksize).min(artifact.len());
    let slice = &artifact[start..end];
    let mut pkt = Vec::with_capacity(4 + slice.len());
    pkt.extend_from_slice(&TFTP_OPCODE_DATA.to_be_bytes());
    pkt.extend_from_slice(&block_number.to_be_bytes());
    pkt.extend_from_slice(slice);
    Some(pkt)
}

/// Resolves an HTTP `Range` request header for an artifact of `total_len` bytes.
///
/// Returns `(http_status, start_offset, end_inclusive)`:
/// - `200` with `(0, total_len.saturating_sub(1))` when `range_header` is `None`.
/// - `206` with clamped `(start, end_inclusive)` for valid `bytes=start-end`, `bytes=start-`,
///   or suffix `bytes=-N` ranges (RFC 9110 §14.1.2).
/// - `Err(...)` for malformed syntax, multipart ranges, or 416 Range Not Satisfiable bounds.
pub fn resolve_http_byte_range(
    range_header: Option<&str>,
    total_len: usize,
) -> Result<(u16, usize, usize), &'static str> {
    let Some(raw_header) = range_header else {
        return Ok((200, 0, total_len.saturating_sub(1)));
    };
    if total_len == 0 {
        return Err("range not satisfiable for empty resource");
    }

    let trimmed = raw_header.trim();
    if trimmed.len() < 6 || !trimmed[..6].eq_ignore_ascii_case("bytes=") {
        return Err("unsupported HTTP Range unit");
    }
    let spec = trimmed[6..].trim();
    if spec.is_empty() || spec.contains(',') {
        return Err("unsupported or multipart HTTP byte range");
    }

    let Some((start_str, end_str)) = spec.split_once('-') else {
        return Err("malformed HTTP byte range missing '-'");
    };
    let start_str = start_str.trim();
    let end_str = end_str.trim();

    if start_str.is_empty() {
        let suffix_len = end_str
            .parse::<usize>()
            .map_err(|_| "invalid HTTP suffix range length")?;
        if suffix_len == 0 {
            return Err("zero-length HTTP suffix range not satisfiable");
        }
        let start = total_len.saturating_sub(suffix_len);
        return Ok((206, start, total_len - 1));
    }

    let start = start_str
        .parse::<usize>()
        .map_err(|_| "invalid HTTP range start offset")?;
    if start >= total_len {
        return Err("HTTP range start beyond artifact length");
    }

    if end_str.is_empty() {
        return Ok((206, start, total_len - 1));
    }

    let raw_end = end_str
        .parse::<usize>()
        .map_err(|_| "invalid HTTP range end offset")?;
    if start > raw_end {
        return Err("HTTP range start exceeds end");
    }

    let end_inclusive = raw_end.min(total_len - 1);
    Ok((206, start, end_inclusive))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_rrq_packet(filename: &str, mode: &str, opts: &[(&str, &str)]) -> Vec<u8> {
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&TFTP_OPCODE_RRQ.to_be_bytes());
        pkt.extend_from_slice(filename.as_bytes());
        pkt.push(0);
        pkt.extend_from_slice(mode.as_bytes());
        pkt.push(0);
        for (k, v) in opts {
            pkt.extend_from_slice(k.as_bytes());
            pkt.push(0);
            pkt.extend_from_slice(v.as_bytes());
            pkt.push(0);
        }
        pkt
    }

    #[test]
    fn test_tftp_rrq_oack_negotiation_and_clamping() {
        // Request oversized blksize (8192 -> clamped 1468), windowsize (64 -> clamped 16), and tsize.
        let rrq = build_rrq_packet(
            "BOOTX64.EFI",
            "octet",
            &[("blksize", "8192"), ("tsize", "0"), ("windowsize", "64")],
        );
        let (session, oack) = negotiate_tftp_rrq(&rrq, 65536).expect("valid RRQ");
        assert_eq!(session.filename, "BOOTX64.EFI");
        assert_eq!(session.blksize, 1468);
        assert_eq!(session.windowsize, 16);
        assert_eq!(session.total_size, 65536);

        let oack_bytes = oack.expect("OACK must be emitted");
        assert_eq!(u16::from_be_bytes([oack_bytes[0], oack_bytes[1]]), TFTP_OPCODE_OACK);
        let fields = split_nul_fields(&oack_bytes[2..]).expect("valid OACK NUL fields");
        assert_eq!(
            fields,
            vec!["blksize", "1468", "tsize", "65536", "windowsize", "16"]
        );

        // Classic RFC 1350 RRQ with no options yields None OACK.
        let classic = build_rrq_packet("ipxe.efi", "octet", &[]);
        let (classic_session, classic_oack) = negotiate_tftp_rrq(&classic, 1024).expect("classic RRQ");
        assert_eq!(classic_session.blksize, 512);
        assert_eq!(classic_session.windowsize, 1);
        assert!(classic_oack.is_none());

        // Path traversal must be rejected.
        let traversal = build_rrq_packet("../etc/passwd", "octet", &[]);
        assert!(negotiate_tftp_rrq(&traversal, 1024).is_err());
    }

    #[test]
    fn test_tftp_data_blocks_exact_multiple_termination() {
        let artifact = vec![0xABu8; 1024];
        let b1 = build_tftp_data_block(&artifact, 512, 1).expect("block 1");
        assert_eq!(b1.len(), 4 + 512);
        assert_eq!(u16::from_be_bytes([b1[0], b1[1]]), TFTP_OPCODE_DATA);
        assert_eq!(u16::from_be_bytes([b1[2], b1[3]]), 1);

        let b2 = build_tftp_data_block(&artifact, 512, 2).expect("block 2");
        assert_eq!(b2.len(), 4 + 512);

        // Exact-multiple transfer requires a 0-payload termination block #3.
        let b3 = build_tftp_data_block(&artifact, 512, 3).expect("termination block 3");
        assert_eq!(b3, vec![0, 3, 0, 3]);

        // Past termination returns None.
        assert!(build_tftp_data_block(&artifact, 512, 4).is_none());
    }

    #[test]
    fn test_http_byte_range_resolution() {
        assert_eq!(resolve_http_byte_range(None, 4096), Ok((200, 0, 4095)));
        assert_eq!(
            resolve_http_byte_range(Some("bytes=0-511"), 4096),
            Ok((206, 0, 511))
        );
        assert_eq!(
            resolve_http_byte_range(Some("bytes=3500-9999"), 4096),
            Ok((206, 3500, 4095))
        );
        assert_eq!(
            resolve_http_byte_range(Some("bytes=2048-"), 4096),
            Ok((206, 2048, 4095))
        );
        assert_eq!(
            resolve_http_byte_range(Some("bytes=-256"), 4096),
            Ok((206, 3840, 4095))
        );
        assert!(resolve_http_byte_range(Some("bytes=4096-4099"), 4096).is_err());
        assert!(resolve_http_byte_range(Some("bytes=100-50"), 4096).is_err());
    }
}
