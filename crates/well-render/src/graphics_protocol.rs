use std::collections::{HashMap, VecDeque};

#[derive(Default, Debug, Clone)]
struct ChunkAccumulator {
    payload: String,
    width: Option<u32>,
    height: Option<u32>,
    columns: Option<u32>,
    rows: Option<u32>,
    format: u32,
    action: char,
    placement_id: u32,
}

#[derive(Debug, Clone)]
pub enum GraphicEvent {
    KittyImage {
        id: u32,
        placement_id: u32,
        format: u32,
        action: char,
        payload_base64: String,
        is_complete: bool, // Support for chunked transmission (m=1)
        width: Option<u32>,
        height: Option<u32>,
        columns: Option<u32>,
        rows: Option<u32>,
    },
    WorkingDirUpdate(String),
    Hyperlink(Option<String>), // None means end of hyperlink region
}

/// A lightweight state machine interceptor that sits in front of the vt100 parser
/// to extract proprietary or advanced ANSI sequences (APC, OSC 7/8).
#[derive(Default)]
pub struct Interceptor {
    buffer: Vec<u8>,
    chunk_accumulators: HashMap<u32, ChunkAccumulator>,
    pub events: VecDeque<GraphicEvent>,
}

impl Interceptor {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            chunk_accumulators: HashMap::new(),
            events: VecDeque::new(),
        }
    }

    /// Process raw bytes. Returns the sanitized byte slice (with intercepted sequences stripped)
    /// to be passed onto the standard vt100 screen parser.
    pub fn process(&mut self, data: &[u8]) -> Vec<u8> {
        self.buffer.extend_from_slice(data);

        let mut sanitized = Vec::new();
        let mut i = 0;

        while i < self.buffer.len() {
            // Check for APC (ESC _)
            if i + 1 < self.buffer.len() && self.buffer[i] == 0x1b && self.buffer[i + 1] == b'_' {
                // Find ST (String Terminator) ESC \ or BEL (\x07)
                if let Some(end_idx) = self.find_terminator(i + 2) {
                    let payload = self.buffer[i + 2..end_idx].to_vec();
                    self.parse_apc(&payload);
                    // Skip over terminator (ESC \ is 2 bytes, BEL is 1)
                    let term_len = if self.buffer[end_idx] == 0x1b { 2 } else { 1 };
                    i = end_idx + term_len;
                    continue;
                } else {
                    // Incomplete sequence, leave in buffer
                    break;
                }
            }

            // Check for OSC (ESC ])
            if i + 1 < self.buffer.len() && self.buffer[i] == 0x1b && self.buffer[i + 1] == b']' {
                if let Some(end_idx) = self.find_terminator(i + 2) {
                    let payload = self.buffer[i + 2..end_idx].to_vec();
                    self.parse_osc(&payload);
                    let term_len = if self.buffer[end_idx] == 0x1b { 2 } else { 1 };
                    i = end_idx + term_len;
                    continue;
                } else {
                    break;
                }
            }

            sanitized.push(self.buffer[i]);
            i += 1;
        }

        // Retain unparsed remainder
        self.buffer.drain(0..i);

        sanitized
    }

    fn find_terminator(&self, start: usize) -> Option<usize> {
        let mut i = start;
        while i < self.buffer.len() {
            if self.buffer[i] == 0x07 {
                return Some(i); // BEL terminator
            }
            if i + 1 < self.buffer.len() && self.buffer[i] == 0x1b && self.buffer[i + 1] == b'\\' {
                return Some(i); // ESC \ (ST) terminator
            }
            i += 1;
        }
        None
    }

    fn parse_apc(&mut self, payload: &[u8]) {
        if payload.starts_with(b"G") {
            // Kitty Image Protocol: _G<key=value,key=value>;<base64>
            let payload_str = String::from_utf8_lossy(&payload[1..]);
            if let Some((keys, b64)) = payload_str.split_once(';') {
                let mut action = 'a';
                let mut format = 32;
                let mut is_complete = true;
                let mut id = 0;
                let mut placement_id = 0;
                let mut width = None;
                let mut height = None;
                let mut columns = None;
                let mut rows = None;

                for kv in keys.split(',') {
                    if let Some((k, v)) = kv.split_once('=') {
                        match k {
                            "a" => {
                                if let Some(c) = v.chars().next() {
                                    action = c
                                }
                            }
                            "f" => format = v.parse().unwrap_or(32),
                            "m" => is_complete = v == "0",
                            "i" => id = v.parse().unwrap_or(0),
                            "p" => placement_id = v.parse().unwrap_or(0),
                            "s" => width = v.parse().ok(),
                            "v" => height = v.parse().ok(),
                            "c" => columns = v.parse().ok(),
                            "r" => rows = v.parse().ok(),
                            _ => {}
                        }
                    }
                }

                if !is_complete {
                    let entry = self.chunk_accumulators.entry(id).or_default();
                    entry.payload.push_str(b64);
                    if width.is_some() {
                        entry.width = width;
                    }
                    if height.is_some() {
                        entry.height = height;
                    }
                    if columns.is_some() {
                        entry.columns = columns;
                    }
                    if rows.is_some() {
                        entry.rows = rows;
                    }
                    entry.format = format;
                    entry.action = action;
                    entry.placement_id = placement_id;
                } else {
                    let (full_payload, w, h, c, r, f, a, p) =
                        if let Some(mut prev) = self.chunk_accumulators.remove(&id) {
                            prev.payload.push_str(b64);
                            (
                                prev.payload,
                                width.or(prev.width),
                                height.or(prev.height),
                                columns.or(prev.columns),
                                rows.or(prev.rows),
                                format,
                                action,
                                placement_id,
                            )
                        } else {
                            (
                                b64.to_string(),
                                width,
                                height,
                                columns,
                                rows,
                                format,
                                action,
                                placement_id,
                            )
                        };

                    self.events.push_back(GraphicEvent::KittyImage {
                        id,
                        placement_id: p,
                        format: f,
                        action: a,
                        payload_base64: full_payload,
                        is_complete: true,
                        width: w,
                        height: h,
                        columns: c,
                        rows: r,
                    });
                }
            }
        }
    }

    fn parse_osc(&mut self, payload: &[u8]) {
        let payload_str = String::from_utf8_lossy(payload);
        if let Some(uri) = payload_str.strip_prefix("7;") {
            // OSC 7: Working Directory (file://hostname/path)
            self.events
                .push_back(GraphicEvent::WorkingDirUpdate(uri.to_string()));
        } else if payload_str.starts_with("8;") {
            // OSC 8: Hyperlink (8;params;URI)
            let mut parts = payload_str.splitn(3, ';');
            parts.next(); // 8
            let _params = parts.next().unwrap_or("");
            let uri = parts.next().unwrap_or("");

            if uri.is_empty() {
                self.events.push_back(GraphicEvent::Hyperlink(None));
            } else {
                self.events
                    .push_back(GraphicEvent::Hyperlink(Some(uri.to_string())));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kitty_single_chunk_parsing() {
        let mut interceptor = Interceptor::new();
        let payload = b"\x1b_Ga=T,f=100,s=320,v=240,i=42;aGVsbG8=\x1b\\";
        let sanitized = interceptor.process(payload);

        assert!(sanitized.is_empty());
        assert_eq!(interceptor.events.len(), 1);

        if let Some(GraphicEvent::KittyImage {
            id,
            format,
            action,
            payload_base64,
            is_complete,
            width,
            height,
            ..
        }) = interceptor.events.pop_front()
        {
            assert_eq!(id, 42);
            assert_eq!(format, 100);
            assert_eq!(action, 'T');
            assert_eq!(payload_base64, "aGVsbG8=");
            assert!(is_complete);
            assert_eq!(width, Some(320));
            assert_eq!(height, Some(240));
        } else {
            panic!("Expected KittyImage event");
        }
    }

    #[test]
    fn test_kitty_chunk_accumulation() {
        let mut interceptor = Interceptor::new();
        // Chunk 1: m=1 (more to follow)
        let chunk1 = b"\x1b_Ga=T,f=100,s=640,v=480,i=99,m=1;chunk_one_\x1b\\";
        let san1 = interceptor.process(chunk1);
        assert!(san1.is_empty());
        assert_eq!(interceptor.events.len(), 0);

        // Chunk 2: m=0 (last chunk)
        let chunk2 = b"\x1b_Ga=T,i=99,m=0;chunk_two\x07";
        let san2 = interceptor.process(chunk2);
        assert!(san2.is_empty());
        assert_eq!(interceptor.events.len(), 1);

        if let Some(GraphicEvent::KittyImage {
            id,
            payload_base64,
            is_complete,
            width,
            height,
            ..
        }) = interceptor.events.pop_front()
        {
            assert_eq!(id, 99);
            assert_eq!(payload_base64, "chunk_one_chunk_two");
            assert!(is_complete);
            assert_eq!(width, Some(640));
            assert_eq!(height, Some(480));
        } else {
            panic!("Expected accumulated KittyImage event");
        }
    }

    #[test]
    fn test_osc7_and_osc8_parsing() {
        let mut interceptor = Interceptor::new();
        let stream = b"\x1b]7;file://localhost/tmp\x07\x1b]8;id=1;https://example.com\x1b\\plain text\x1b]8;;\x1b\\";
        let sanitized = interceptor.process(stream);

        assert_eq!(String::from_utf8_lossy(&sanitized), "plain text");
        assert_eq!(interceptor.events.len(), 3);

        match interceptor.events.pop_front().unwrap() {
            GraphicEvent::WorkingDirUpdate(uri) => assert_eq!(uri, "file://localhost/tmp"),
            _ => panic!("Expected WorkingDirUpdate"),
        }

        match interceptor.events.pop_front().unwrap() {
            GraphicEvent::Hyperlink(Some(url)) => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Hyperlink"),
        }

        match interceptor.events.pop_front().unwrap() {
            GraphicEvent::Hyperlink(None) => (),
            _ => panic!("Expected Hyperlink close"),
        }
    }
}
