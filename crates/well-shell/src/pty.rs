//! Persistent PTY session, terminal decoding, and owned process lifecycle.
use crate::graphics::{GraphicEvent, Interceptor};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
pub use vt100::{MouseProtocolEncoding, MouseProtocolMode};

fn pack_colors(fg: [u8; 3], bg: [u8; 3]) -> u64 {
    u64::from_le_bytes([fg[0], fg[1], fg[2], bg[0], bg[1], bg[2], 0, 0])
}
fn color_reply(code: u8, [r, g, b]: [u8; 3]) -> Vec<u8> {
    format!("\x1b]{code};rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}\x1b\\").into_bytes()
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    ScreenChanged,
    Exited(u32),
    IoError(String),
}

/// A detached copy of visible cells; no parser lock is held during rendering.
#[derive(Clone, Debug)]
pub struct ScreenSnapshot {
    pub rows: u16,
    pub cols: u16,
    pub cells: Vec<vt100::Cell>,
    pub cursor: Option<(u16, u16)>,
    pub scrollback: usize,
    pub wrapped: Vec<bool>,
}

pub struct PtySession {
    pub parser: Arc<Mutex<vt100::Parser>>,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Option<Box<dyn MasterPty + Send>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    pub graphic_events_rx: Mutex<Receiver<GraphicEvent>>,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    input: Sender<Vec<u8>>,
    queued_bytes: Arc<AtomicUsize>,
    input_worker: Option<JoinHandle<()>>,
    default_colors: Arc<AtomicU64>,
}

/// Retains incomplete escape sequences between reads. Queries are answered only
/// after preceding bytes have updated the screen (notably cursor-position reports).
#[derive(Default)]
pub struct TerminalDecoder {
    pending: Vec<u8>,
    discarded: bool,
    previous_escape: bool,
    graphics: Interceptor,
    default_colors: Option<([u8; 3], [u8; 3])>,
}

impl TerminalDecoder {
    pub fn process<W: Write + ?Sized>(
        &mut self,
        data: &[u8],
        parser: &mut vt100::Parser,
        writer: &mut W,
    ) -> io::Result<Vec<u8>> {
        let mut output = Vec::new();
        for &byte in data {
            if self.discarded {
                if byte == 7 || (self.previous_escape && byte == b'\\') {
                    self.discarded = false;
                }
                self.previous_escape = byte == 27;
                continue;
            }
            if self.pending.is_empty() {
                if byte == 27 {
                    self.pending.push(byte);
                } else {
                    parser.process(&[byte]);
                    output.push(byte);
                }
                continue;
            }
            self.pending.push(byte);
            let complete = match self.pending.get(1) {
                Some(b'[') => self.pending.len() > 2 && (0x40..=0x7e).contains(&byte),
                Some(b']' | b'P' | b'_' | b'^' | b'X') => {
                    byte == 7 || self.pending.ends_with(b"\x1b\\")
                }
                Some(0x20..=0x2f) => self.pending.len() > 2 && (0x30..=0x7e).contains(&byte),
                _ => true,
            };
            if complete {
                let seq = std::mem::take(&mut self.pending);
                let reply: Option<Vec<u8>> = match seq.as_slice() {
                    b"\x1b[c" | b"\x1b[0c" => Some(b"\x1b[?1;2c".to_vec()),
                    b"\x1b[>c" | b"\x1b[>0c" => Some(b"\x1b[>0;1;0c".to_vec()),
                    b"\x1b[5n" => Some(b"\x1b[0n".to_vec()),
                    b"\x1b[6n" => {
                        let (r, c) = parser.screen().cursor_position();
                        Some(format!("\x1b[{};{}R", r + 1, c + 1).into_bytes())
                    }
                    b"\x1b[?u" => Some(b"\x1b[?0u".to_vec()),
                    _ if seq.starts_with(b"\x1bP+q") => {
                        let mut reply = b"\x1bP0+r".to_vec();
                        let end = seq.len() - if seq.ends_with(b"\x1b\\") { 2 } else { 1 };
                        reply.extend_from_slice(&seq[4..end]);
                        reply.extend_from_slice(b"\x1b\\");
                        Some(reply)
                    }
                    _ if seq.starts_with(b"\x1b]10;?") => Some(color_reply(
                        10,
                        self.default_colors.unwrap_or(([208; 3], [16, 20, 26])).0,
                    )),
                    _ if seq.starts_with(b"\x1b]11;?") => Some(color_reply(
                        11,
                        self.default_colors.unwrap_or(([208; 3], [16, 20, 26])).1,
                    )),
                    _ => None,
                };
                if let Some(reply) = reply {
                    writer.write_all(&reply)?;
                    writer.flush()?;
                } else if seq.starts_with(b"\x1b_G")
                    || seq.starts_with(b"\x1b]7;")
                    || seq.starts_with(b"\x1b]8;")
                {
                    self.graphics.process(&seq);
                } else if !matches!(seq.get(1), Some(b'_' | b'P' | b'^' | b'X')) {
                    parser.process(&seq);
                    output.extend_from_slice(&seq);
                }
            } else if self.pending.len() >= 65536 {
                self.pending.clear();
                self.discarded = true;
                self.previous_escape = byte == 27;
            }
        }
        Ok(output)
    }
}

/// One-shot compatibility helper. Streaming callers should retain a TerminalDecoder.
pub fn filter_and_respond_queries<W: Write + ?Sized>(
    data: &[u8],
    parser: &Arc<Mutex<vt100::Parser>>,
    writer: &mut W,
) -> Vec<u8> {
    let mut parser = parser.lock().unwrap_or_else(|p| p.into_inner());
    TerminalDecoder::default()
        .process(data, &mut parser, writer)
        .unwrap_or_default()
}
pub fn handle_terminal_queries<W: Write + ?Sized>(
    data: &[u8],
    parser: &Arc<Mutex<vt100::Parser>>,
    writer: &mut W,
) {
    let _ = filter_and_respond_queries(data, parser, writer);
}

impl PtySession {
    pub fn spawn<F: Fn() + Send + 'static>(
        rows: u16,
        cols: u16,
        on_output: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::spawn_with_shell(rows, cols, None, on_output)
    }
    pub fn spawn_with_shell<F: FnMut() + Send + 'static>(
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        mut output: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::spawn_with_stream(rows, cols, shell, move |_| output())
    }
    pub fn spawn_with_stream<F: FnMut(&[u8]) + Send + 'static>(
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        stream: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start(rows, cols, shell, true, stream, |_| {})
    }
    pub fn spawn_with_events<F: FnMut(SessionEvent) + Send + 'static>(
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        events: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start(rows, cols, shell, true, |_| {}, events)
    }
    pub fn spawn_with_colors<F: FnMut(SessionEvent) + Send + 'static>(
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        foreground: [u8; 3],
        background: [u8; 3],
        events: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_configured(
            rows,
            cols,
            shell,
            true,
            |_| {},
            events,
            (foreground, background),
        )
    }
    pub(crate) fn start<
        S: FnMut(&[u8]) + Send + 'static,
        F: FnMut(SessionEvent) + Send + 'static,
    >(
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        login: bool,
        stream: S,
        events: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_configured(
            rows,
            cols,
            shell,
            login,
            stream,
            events,
            ([208; 3], [16, 20, 26]),
        )
    }
    fn start_configured<
        S: FnMut(&[u8]) + Send + 'static,
        F: FnMut(SessionEvent) + Send + 'static,
    >(
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        login: bool,
        stream: S,
        events: F,
        colors: ([u8; 3], [u8; 3]),
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_with_env(rows, cols, shell, login, stream, events, colors, &[])
    }
    pub fn spawn_with_environment<F: FnMut(SessionEvent) + Send + 'static>(
        rows: u16,
        cols: u16,
        foreground: [u8; 3],
        background: [u8; 3],
        environment: &[(&str, &std::ffi::OsStr)],
        events: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_with_env(
            rows,
            cols,
            None,
            true,
            |_| {},
            events,
            (foreground, background),
            environment,
        )
    }
    fn start_with_env<S: FnMut(&[u8]) + Send + 'static, F: FnMut(SessionEvent) + Send + 'static>(
        rows: u16,
        cols: u16,
        shell: Option<&str>,
        login: bool,
        mut stream: S,
        mut events: F,
        colors: ([u8; 3], [u8; 3]),
        environment: &[(&str, &std::ffi::OsStr)],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let pair = native_pty_system().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let shell = shell
            .map(str::to_owned)
            .or_else(|| std::env::var("SHELL").ok().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    "powershell.exe".into()
                } else {
                    "/bin/sh".into()
                }
            });
        let mut cmd = CommandBuilder::new(shell);
        if cfg!(unix) {
            if login {
                cmd.arg("-l");
            }
            cmd.arg("-i");
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Well-Shell");
        cmd.env("WELL_SHELL", "1");
        for (key, value) in environment {
            cmd.env(key, value);
        }
        if !login {
            cmd.env("HISTFILE", "/dev/null");
            cmd.env("ENV", "/dev/null");
        }
        cmd.cwd(std::env::current_dir()?);
        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let mut reader = pair.master.try_clone_reader()?;
        #[cfg(unix)]
        let fd = pair
            .master
            .as_raw_fd()
            .ok_or_else(|| io::Error::other("PTY has no file descriptor"))?;
        #[cfg(unix)]
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags == -1 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
                return Err(io::Error::last_os_error().into());
            }
        }
        let child = Arc::new(Mutex::new(pair.slave.spawn_command(cmd)?));

        drop(pair.slave);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 10_000)));
        let default_colors = Arc::new(AtomicU64::new(pack_colors(colors.0, colors.1)));
        let worker_colors = default_colors.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let parser_thread = Arc::clone(&parser);
        let (input, input_rx) = channel::<Vec<u8>>();
        let (input_error, input_errors) = channel::<String>();
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let queued_worker = Arc::clone(&queued_bytes);
        let input_stop = Arc::clone(&stop);
        let input_writer = Arc::clone(&writer);
        let input_worker = thread::Builder::new()
            .name("well-pty-writer".into())
            .spawn(move || {
                while !input_stop.load(Ordering::Acquire) {
                    let bytes = match input_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                        Ok(bytes) => bytes,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    let mut offset = 0;
                    while offset < bytes.len() && !input_stop.load(Ordering::Acquire) {
                        let result = input_writer
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .write(&bytes[offset..]);
                        match result {
                            Ok(0) => {
                                let _ = input_error.send("PTY input closed".into());
                                break;
                            }
                            Ok(n) => offset += n,
                            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(5))
                            }
                            Err(e) => {
                                let _ = input_error.send(e.to_string());
                                break;
                            }
                        }
                    }
                    queued_worker.fetch_sub(bytes.len(), Ordering::AcqRel);
                }
            });
        let input_worker = match input_worker {
            Ok(worker) => worker,
            Err(e) => {
                let mut child = child.lock().unwrap_or_else(|p| p.into_inner());
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        };
        let reply_input = input.clone();
        let reply_bytes = Arc::clone(&queued_bytes);

        let child_thread = Arc::clone(&child);
        let stop_thread = Arc::clone(&stop);
        let (gfx_tx, gfx_rx) = sync_channel(256);
        let worker = thread::Builder::new()
            .name("well-pty-reader".into())
            .spawn(move || {
                let mut decoder = TerminalDecoder::default();
                let mut buf = [0; 16384];
                while !stop_thread.load(Ordering::Acquire) {
                    for error in input_errors.try_iter() {
                        events(SessionEvent::IoError(error));
                    }

                    #[cfg(unix)]
                    {
                        let mut poll = libc::pollfd {
                            fd,
                            events: libc::POLLIN,
                            revents: 0,
                        };
                        // fd remains owned by the session until this thread has joined.
                        let ready = unsafe { libc::poll(&mut poll, 1, 100) };
                        if ready == 0 {
                            continue;
                        }
                        if ready < 0 {
                            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                                continue;
                            }
                            events(SessionEvent::IoError(
                                io::Error::last_os_error().to_string(),
                            ));
                            break;
                        }
                    }
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let result = {
                                let mut replies = Vec::new();
                                let mut parser =
                                    parser_thread.lock().unwrap_or_else(|p| p.into_inner());
                                let colors = worker_colors.load(Ordering::Acquire).to_le_bytes();
                                decoder.default_colors = Some((
                                    [colors[0], colors[1], colors[2]],
                                    [colors[3], colors[4], colors[5]],
                                ));
                                let result = decoder.process(&buf[..n], &mut parser, &mut replies);
                                if !replies.is_empty() {
                                    if let Err(e) = enqueue(&reply_input, &reply_bytes, &replies) {
                                        events(SessionEvent::IoError(e.to_string()));
                                    }
                                }
                                result
                            };
                            while let Some(event) = decoder.graphics.events.pop_front() {
                                let _ = gfx_tx.try_send(event);
                            }
                            match result {
                                Ok(bytes) => {
                                    stream(&bytes);
                                    events(SessionEvent::ScreenChanged);
                                }
                                Err(e) => {
                                    events(SessionEvent::IoError(e.to_string()));
                                    break;
                                }
                            }
                        }
                        Err(e)
                            if matches!(
                                e.kind(),
                                io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                            ) =>
                        {
                            continue
                        }
                        #[cfg(unix)]
                        Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                        Err(e) => {
                            events(SessionEvent::IoError(e.to_string()));
                            break;
                        }
                    }
                }
                if !stop_thread.load(Ordering::Acquire) {
                    // EOF follows all output; poll without holding the child lock while waiting.
                    loop {
                        if stop_thread.load(Ordering::Acquire) {
                            break;
                        }
                        let status = child_thread
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .try_wait();
                        match status {
                            Ok(Some(status)) => {
                                let code = status.exit_code();
                                let message = format!("\r\n[Shell exited with status {code}]\r\n");
                                parser_thread
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner())
                                    .process(message.as_bytes());
                                events(SessionEvent::Exited(code));
                                break;
                            }
                            Ok(None) => thread::sleep(std::time::Duration::from_millis(20)),
                            Err(e) => {
                                events(SessionEvent::IoError(e.to_string()));
                                break;
                            }
                        }
                    }
                }
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(e) => {
                let mut child = child.lock().unwrap_or_else(|p| p.into_inner());
                let _ = child.kill();
                let _ = child.wait();
                stop.store(true, Ordering::Release);
                let _ = input_worker.join();
                return Err(e.into());
            }
        };
        Ok(Self {
            parser,
            writer,
            master: Some(pair.master),
            child,
            graphic_events_rx: Mutex::new(gfx_rx),
            stop,
            reader: Some(worker),
            input,
            queued_bytes,
            input_worker: Some(input_worker),
            default_colors,
        })
    }
    pub fn set_default_colors(&self, foreground: [u8; 3], background: [u8; 3]) {
        self.default_colors
            .store(pack_colors(foreground, background), Ordering::Release);
    }
    pub fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        if self.stop.load(Ordering::Acquire) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Session closed"));
        }
        enqueue(&self.input, &self.queued_bytes, bytes)
    }
    pub fn resize(
        &self,
        rows: u16,
        cols: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let mut parser = self
            .parser
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        if let Some(master) = &self.master {
            master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        parser.screen_mut().set_size(rows, cols);
        Ok(())
    }
    pub fn snapshot(&self) -> ScreenSnapshot {
        let parser = self.parser.lock().unwrap_or_else(|p| p.into_inner());
        let screen = parser.screen();
        let (rows, cols) = screen.size();
        let mut cells = Vec::with_capacity(rows as usize * cols as usize);
        for r in 0..rows {
            for c in 0..cols {
                cells.push(screen.cell(r, c).unwrap().clone());
            }
        }
        ScreenSnapshot {
            rows,
            cols,
            cells,
            cursor: if screen.hide_cursor() || screen.scrollback() > 0 {
                None
            } else {
                Some(screen.cursor_position())
            },
            scrollback: screen.scrollback(),
            wrapped: (0..rows).map(|r| screen.row_wrapped(r)).collect(),
        }
    }
    pub fn scroll(&self, lines: i32) {
        let mut parser = self.parser.lock().unwrap_or_else(|p| p.into_inner());
        let screen = parser.screen_mut();
        if !screen.alternate_screen() {
            let offset = screen.scrollback().saturating_add_signed(lines as isize);
            screen.set_scrollback(offset);
        }
    }
    pub fn scroll_to_bottom(&self) {
        self.parser
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .screen_mut()
            .set_scrollback(0);
    }
    pub fn input_modes(&self) -> (bool, bool) {
        let parser = self.parser.lock().unwrap_or_else(|p| p.into_inner());
        (
            parser.screen().application_cursor(),
            parser.screen().bracketed_paste(),
        )
    }
    pub fn mouse_modes(&self) -> (MouseProtocolMode, MouseProtocolEncoding) {
        let parser = self.parser.lock().unwrap_or_else(|p| p.into_inner());
        (
            parser.screen().mouse_protocol_mode(),
            parser.screen().mouse_protocol_encoding(),
        )
    }
    pub fn is_alive(&self) -> bool {
        self.child
            .lock()
            .map(|mut c| matches!(c.try_wait(), Ok(None)))
            .unwrap_or(false)
    }
    pub fn shutdown(&mut self) {
        if self.stop.swap(true, Ordering::AcqRel) {
            return;
        }
        let mut child = self.child.lock().unwrap_or_else(|p| p.into_inner());
        if matches!(child.try_wait(), Ok(None)) {
            #[cfg(unix)]
            let foreground = self
                .master
                .as_ref()
                .and_then(|m| m.process_group_leader())
                .filter(|&group| group > 1 && group != unsafe { libc::getpgrp() });
            #[cfg(unix)]
            if let Some(group) = foreground {
                // Only signal the foreground group attached to our owned PTY.
                unsafe {
                    libc::kill(-group, libc::SIGHUP);
                }
            }
            let _ = child.kill();
            #[cfg(unix)]
            if let Some(group) = foreground {
                // Keep the group captured before shell exit: hangup can make
                // tcgetpgrp fail even while a program ignoring SIGHUP survives.
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
            }
        }
        drop(child);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(writer) = self.input_worker.take() {
            let _ = writer.join();
        }
        *self.writer.lock().unwrap_or_else(|p| p.into_inner()) = Box::new(io::sink());
        self.master.take();
        // Close every master descriptor before waiting. On macOS an exiting
        // child can wait for a full terminal output buffer to drain; waiting
        // while keeping the unread master open deadlocks under output floods.
        let _ = self.child.lock().unwrap_or_else(|p| p.into_inner()).wait();
    }
}
impl Drop for PtySession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Bound pending input to 1 MiB without blocking the UI or output reader.
fn enqueue(sender: &Sender<Vec<u8>>, queued: &AtomicUsize, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    let mut pending = queued.load(Ordering::Acquire);
    loop {
        let next = pending
            .checked_add(bytes.len())
            .filter(|&n| n <= 1024 * 1024)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Terminal input queue is full (1 MiB limit)",
                )
            })?;
        match queued.compare_exchange_weak(pending, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(current) => pending = current,
        }
    }
    if sender.send(bytes.to_vec()).is_err() {
        queued.fetch_sub(bytes.len(), Ordering::AcqRel);
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "PTY input worker stopped",
        ));
    }
    Ok(())
}
pub use vt100::Color;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn decode_chunks(data: &[u8], width: usize) -> (vt100::Parser, Vec<u8>) {
        let mut parser = vt100::Parser::new(24, 80, 100);
        let mut decoder = TerminalDecoder::default();
        let mut replies = Vec::new();
        for chunk in data.chunks(width) {
            decoder.process(chunk, &mut parser, &mut replies).unwrap();
        }
        (parser, replies)
    }
    #[test]
    fn fragmented_queries_follow_screen_updates() {
        for width in 1..20 {
            let (parser, reply) = decode_chunks(
                b"hello\x1b[6n\x1b[c\x1b[>c\x1b[?u\x1b]11;?\x1b\\world",
                width,
            );
            assert_eq!(parser.screen().contents(), "helloworld");
            assert_eq!(
                reply,
                b"\x1b[1;6R\x1b[?1;2c\x1b[>0;1;0c\x1b[?0u\x1b]11;rgb:1010/1414/1a1a\x1b\\"
            );
        }
    }
    #[test]
    fn alternate_screen_restores_main_screen() {
        let mut parser = vt100::Parser::new(24, 80, 100);
        let mut decoder = TerminalDecoder::default();
        for b in b"main\x1b[?1049h\x1b[H\x1b[2Jalternate" {
            decoder
                .process(&[*b], &mut parser, &mut Vec::new())
                .unwrap();
        }
        assert!(parser.screen().alternate_screen());
        assert_eq!(parser.screen().contents(), "alternate");
        decoder
            .process(b"\x1b[?1049l", &mut parser, &mut Vec::new())
            .unwrap();
        assert!(!parser.screen().alternate_screen());
        assert_eq!(parser.screen().contents(), "main");
    }
    #[test]
    fn oversized_strings_are_bounded_without_leaking_payload() {
        let mut parser = vt100::Parser::new(24, 80, 100);
        let mut decoder = TerminalDecoder::default();
        decoder
            .process(b"\x1b_G", &mut parser, &mut Vec::new())
            .unwrap();
        decoder
            .process(&vec![b'x'; 100_000], &mut parser, &mut Vec::new())
            .unwrap();
        assert!(decoder.pending.len() < 65536);
        decoder
            .process(b"\x1b\\after", &mut parser, &mut Vec::new())
            .unwrap();
        assert_eq!(parser.screen().contents(), "after");
    }
    #[test]
    fn queued_input_is_bounded() {
        let (tx, rx) = channel();
        let queued = AtomicUsize::new(0);
        enqueue(&tx, &queued, &vec![0; 1024 * 1024]).unwrap();
        assert_eq!(
            enqueue(&tx, &queued, b"x").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(rx.recv().unwrap().len(), 1024 * 1024);
    }
    fn wait_for(session: &PtySession, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let contents = session.parser.lock().unwrap().screen().contents();
            if contents.contains(needle) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Missing {needle:?}; screen: {contents:?}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[cfg(unix)]
    fn shell() -> PtySession {
        let session = PtySession::start(24, 80, Some("/bin/sh"), false, |_| {}, |_| {}).unwrap();
        session
            .write_all(b"stty -echo; PS1=''; printf '\\137READY\\137\\n'\r")
            .unwrap();
        wait_for(&session, "_READY_");
        session
    }
    #[cfg(unix)]
    #[test]
    fn persistent_shell_streams_quotes_pipelines_cwd_resize_interrupt_and_exit() {
        let (tx, rx) = channel();
        let mut session = PtySession::start(
            24,
            80,
            Some("/bin/sh"),
            false,
            |_| {},
            move |e| {
                let _ = tx.send(e);
            },
        )
        .unwrap();
        session
            .write_all(b"stty -echo; PS1=''; printf '\\137READY\\137\\n'\r")
            .unwrap();
        wait_for(&session, "_READY_");
        session.write_all(b"printf 'DIR:%s\\n' \"$PWD\"\r").unwrap();
        wait_for(
            &session,
            &format!("DIR:{}", std::env::current_dir().unwrap().display()),
        );
        session
            .write_all(b"cd /tmp\rprintf 'DIR:%s\\n' \"$PWD\"; printf '%s\\n' 'a b' | tr ' ' '_'\r")
            .unwrap();
        wait_for(&session, "DIR:/tmp");
        wait_for(&session, "a_b");
        session.resize(33, 91).unwrap();
        session.write_all(b"stty size\r").unwrap();
        wait_for(&session, "33 91");
        session
            .write_all(b"printf '\\137RUN\\137\\n'; sleep 30\r")
            .unwrap();
        wait_for(&session, "_RUN_");
        session.write_all(&[3]).unwrap();
        session
            .write_all(b"printf '\\137INTERRUPTED\\137\\n'\r")
            .unwrap();
        wait_for(&session, "_INTERRUPTED_");
        session
            .write_all(b"printf '\\137FINAL\\137\\n'; exit 7\r")
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let event = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("exit notification");
            if let SessionEvent::Exited(code) = event {
                assert_eq!(code, 7);
                break;
            }
            if let SessionEvent::IoError(e) = event {
                panic!("PTY error: {e}");
            }
        }
        wait_for(&session, "_FINAL_");
        wait_for(&session, "Shell exited with status 7");
        assert!(!session.is_alive());
        session.shutdown();
    }
    #[cfg(unix)]
    #[test]
    fn scrollback_clamps_and_returns_to_bottom() {
        let mut session = shell();
        session.write_all(b"i=0; while [ $i -lt 150 ]; do echo line-$i; i=$((i+1)); done; printf '\\137DONE\\137\\n'\r").unwrap();
        wait_for(&session, "_DONE_");
        session.scroll(100_000);
        let snapshot = session.snapshot();
        assert!(snapshot.scrollback > 0 && snapshot.scrollback <= 10_000);
        assert_eq!(snapshot.cursor, None);
        session.scroll_to_bottom();
        assert_eq!(session.snapshot().scrollback, 0);
        session.shutdown();
    }
    #[cfg(unix)]
    #[test]
    fn blocked_large_input_does_not_block_shutdown() {
        let mut session = shell();
        session
            .write_all(b"printf '\\137SLEEPING\\137\\n'; sleep 30\r")
            .unwrap();
        wait_for(&session, "_SLEEPING_");
        let before = Instant::now();
        session.write_all(&vec![b'x'; 512 * 1024]).unwrap();
        assert!(before.elapsed() < Duration::from_millis(100));
        session.shutdown();
        assert!(before.elapsed() < Duration::from_secs(2));
    }
    #[cfg(unix)]
    #[test]
    fn vim_restores_screen_and_cursor_modes() {
        let mut session = shell();
        session
            .write_all(b"vim -Nu NONE -n -i NONE; printf '\\137VIM_DONE\\137\\n'\r")
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !session.parser.lock().unwrap().screen().alternate_screen() {
            assert!(
                Instant::now() < deadline,
                "Vim never entered the alternate screen"
            );
            thread::sleep(Duration::from_millis(10));
        }
        session.write_all(b"\x1b:q!\r").unwrap();
        wait_for(&session, "_VIM_DONE_");
        assert!(!session.parser.lock().unwrap().screen().alternate_screen());
        session.shutdown();
    }
    #[cfg(unix)]
    #[test]
    fn shutdown_reaps_shell_and_foreground_job() {
        let mut session = shell();
        session
            .write_all(b"/bin/sh -c 'trap \"\" HUP; printf \"JOB:%s\\n\" \"$$\"; exec sleep 30'\r")
            .unwrap();
        wait_for(&session, "JOB:");
        let contents = session.parser.lock().unwrap().screen().contents();
        let pid: i32 = contents
            .split("JOB:")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let before = Instant::now();
        session.shutdown();
        assert!(before.elapsed() < Duration::from_secs(2));
        assert!(!session.is_alive());
        // The foreground job belongs to the PTY session we just closed.
        let deadline = Instant::now() + Duration::from_secs(2);
        while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "job survived session shutdown"
        );
    }
}
