use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum GraphicEvent {
    KittyImage {
        id: u32,
        placement_id: u32,
        format: u32,
        action: char,
        payload_base64: String,
        is_complete: bool, // Support for chunked transmission (m=1)
    },
    WorkingDirUpdate(String),
    Hyperlink(Option<String>), // None means end of hyperlink region
}

/// A lightweight state machine interceptor that sits in front of the vt100 parser
/// to extract proprietary or advanced ANSI sequences (APC, OSC 7/8).
pub struct Interceptor {
    buffer: Vec<u8>,
    pub events: VecDeque<GraphicEvent>,
}

impl Interceptor {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            events: VecDeque::new(),
        }
    }

    /// Process raw bytes. Returns the sanitized byte slice (with intercepted sequences stripped)
    /// to be passed onto the standard vt100 screen parser.
    pub fn process<'a>(&mut self, data: &'a [u8]) -> Vec<u8> {
        self.buffer.extend_from_slice(data);
        
        let mut sanitized = Vec::new();
        let mut i = 0;
        
        while i < self.buffer.len() {
            // Check for APC (ESC _)
            if i + 1 < self.buffer.len() && self.buffer[i] == 0x1b && self.buffer[i+1] == b'_' {
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
            if i + 1 < self.buffer.len() && self.buffer[i] == 0x1b && self.buffer[i+1] == b']' {
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
            if i + 1 < self.buffer.len() && self.buffer[i] == 0x1b && self.buffer[i+1] == b'\\' {
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
                
                for kv in keys.split(',') {
                    if let Some((k, v)) = kv.split_once('=') {
                        match k {
                            "a" => if let Some(c) = v.chars().next() { action = c },
                            "f" => format = v.parse().unwrap_or(32),
                            "m" => is_complete = v == "0",
                            "i" => id = v.parse().unwrap_or(0),
                            "p" => placement_id = v.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
                
                self.events.push_back(GraphicEvent::KittyImage {
                    id,
                    placement_id,
                    format,
                    action,
                    payload_base64: b64.to_string(),
                    is_complete,
                });
            }
        }
    }
    
    fn parse_osc(&mut self, payload: &[u8]) {
        let payload_str = String::from_utf8_lossy(payload);
        if payload_str.starts_with("7;") {
            // OSC 7: Working Directory (file://hostname/path)
            let uri = &payload_str[2..];
            self.events.push_back(GraphicEvent::WorkingDirUpdate(uri.to_string()));
        } else if payload_str.starts_with("8;") {
            // OSC 8: Hyperlink (8;params;URI)
            let mut parts = payload_str.splitn(3, ';');
            parts.next(); // 8
            let _params = parts.next().unwrap_or("");
            let uri = parts.next().unwrap_or("");
            
            if uri.is_empty() {
                self.events.push_back(GraphicEvent::Hyperlink(None));
            } else {
                self.events.push_back(GraphicEvent::Hyperlink(Some(uri.to_string())));
            }
        }
    }
}
