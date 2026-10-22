//! Owns persistent tab sessions. Background output never changes the focused session.
use crate::AppEvent;
use serde_json::{json, Value};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use well_config::Appearance;
use well_shell::pty::{PtySession, SessionEvent};
use winit::event_loop::EventLoopProxy;

pub const MAX_TABS: usize = 16;
#[derive(Clone)]
pub struct TabInfo {
    pub id: u64,
    pub exit_status: Option<u32>,
}
struct Tab {
    id: u64,
    session: PtySession,
    pending: Arc<AtomicBool>,
    exit_status: Option<u32>,
}
#[derive(Clone, Copy)]
pub enum Action {
    NewTab,
    SwitchTab(usize),
    CloseTab,
}

pub struct Workspace {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    proxy: EventLoopProxy<AppEvent>,
    socket: PathBuf,
    appearance: Appearance,
    size: (u16, u16),
    reapers: Vec<JoinHandle<()>>,
}
impl Deref for Workspace {
    type Target = PtySession;
    fn deref(&self) -> &PtySession {
        &self.tabs[self.active].session
    }
}
impl Workspace {
    pub fn new(
        proxy: EventLoopProxy<AppEvent>,
        socket: PathBuf,
        appearance: Appearance,
        rows: u16,
        cols: u16,
    ) -> Result<Self, String> {
        let mut workspace = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            proxy,
            socket,
            appearance,
            size: (rows, cols),
            reapers: Vec::new(),
        };
        workspace.new_tab()?;
        Ok(workspace)
    }
    pub fn len(&self) -> usize {
        self.tabs.len()
    }
    pub fn active_index(&self) -> usize {
        self.active
    }
    pub fn active_id(&self) -> u64 {
        self.tabs[self.active].id
    }
    pub fn exited(&self) -> bool {
        self.tabs[self.active].exit_status.is_some()
    }
    pub fn tab_info(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .map(|t| TabInfo {
                id: t.id,
                exit_status: t.exit_status,
            })
            .collect()
    }
    pub fn new_tab(&mut self) -> Result<u64, String> {
        if self.tabs.len() >= MAX_TABS {
            return Err(format!("The limit is {MAX_TABS} tabs"));
        }
        self.collect_reapers();
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or("Surface ID exhausted")?;
        let pending = Arc::new(AtomicBool::new(false));
        let worker_pending = pending.clone();
        let proxy = self.proxy.clone();
        let id_text = id.to_string();
        let session = PtySession::spawn_with_environment(
            self.size.0,
            self.size.1,
            self.appearance.foreground,
            self.appearance.background,
            &[
                ("WELL_RPC_SOCKET", self.socket.as_os_str()),
                ("WELL_SURFACE_ID", std::ffi::OsStr::new(&id_text)),
            ],
            move |event| {
                if matches!(event, SessionEvent::ScreenChanged)
                    && worker_pending.swap(true, Ordering::AcqRel)
                {
                    return;
                }
                let _ = proxy.send_event(AppEvent::Session { id, event });
            },
        )
        .map_err(|e| e.to_string())?;
        self.tabs.push(Tab {
            id,
            session,
            pending,
            exit_status: None,
        });
        self.active = self.tabs.len() - 1;
        Ok(id)
    }
    pub fn switch_tab(&mut self, index: usize) -> Result<(), String> {
        if index >= self.tabs.len() {
            return Err(format!("Tab index {index} does not exist"));
        }
        self.active = index;
        Ok(())
    }
    /// Returns false after closing the final tab; callers must then close the window.
    pub fn close_tab(&mut self) -> bool {
        let tab = self.tabs.remove(self.active);
        self.reap(tab.session);
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        !self.tabs.is_empty()
    }
    pub fn rendered(&self) {
        self.tabs[self.active]
            .pending
            .store(false, Ordering::Release);
    }
    pub fn apply_event(&mut self, id: u64, event: SessionEvent) -> bool {
        let Some(index) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        let tab = &mut self.tabs[index];
        match event {
            SessionEvent::Exited(code) => tab.exit_status = Some(code),
            SessionEvent::IoError(message) => {
                eprintln!("Well surface {id}: {message}");
                tab.session
                    .parser
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .process(format!("\r\n[Well: {}]\r\n", message.escape_default()).as_bytes());
            }
            SessionEvent::ScreenChanged => return index == self.active,
        }
        true
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<(), String> {
        self.size = (rows, cols);
        let mut errors = Vec::new();
        for tab in &self.tabs {
            if let Err(e) = tab.session.resize(rows, cols) {
                errors.push(format!("surface {}: {e}", tab.id));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
    pub fn set_appearance(&mut self, appearance: &Appearance) {
        self.appearance = appearance.clone();
        for tab in &self.tabs {
            tab.session
                .set_default_colors(appearance.foreground, appearance.background);
        }
    }
    pub fn send_to(&self, id: Option<u64>, text: &str) -> Result<Value, String> {
        let id = id.unwrap_or_else(|| self.active_id());
        let tab = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Surface {id} does not exist"))?;
        if tab.exit_status.is_some() || !tab.session.is_alive() {
            return Err(format!("Surface {id} has exited"));
        }
        tab.session
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(json!({"surface_id":id, "queued_bytes":text.len()}))
    }
    pub fn state(&self) -> Value {
        json!({"active_tab":self.active,"active_surface_id":self.active_id(),"tabs":self.tabs.iter().enumerate().map(|(index,t)|{
            let (rows,cols) = t.session.parser.lock().unwrap_or_else(|p| p.into_inner()).screen().size();
            json!({"index":index,"surface_id":t.id,"exit_status":t.exit_status,"rows":rows,"cols":cols})
        }).collect::<Vec<_>>()})
    }
    pub fn read_surface(&self, id: Option<u64>) -> Result<Value, String> {
        let id = id.unwrap_or_else(|| self.active_id());
        let tab = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Surface {id} does not exist"))?;
        let parser = tab.session.parser.lock().unwrap_or_else(|p| p.into_inner());
        let screen = parser.screen();
        let text = screen.contents();
        let mut length = text.len().min(64 * 1024);
        while !text.is_char_boundary(length) {
            length -= 1;
        }
        let (rows, cols) = screen.size();
        Ok(
            json!({"surface_id":id,"text":&text[..length],"truncated":length < text.len(),"rows":rows,"cols":cols,"cursor":screen.cursor_position(),"cursor_visible":!screen.hide_cursor() && screen.scrollback() == 0,"scrollback":screen.scrollback()}),
        )
    }
    fn collect_reapers(&mut self) {
        let mut i = 0;
        while i < self.reapers.len() {
            if self.reapers[i].is_finished() {
                let _ = self.reapers.swap_remove(i).join();
            } else {
                i += 1;
            }
        }
    }
    fn reap(&mut self, mut session: PtySession) {
        self.collect_reapers();
        // At most MAX_TABS detached cleanups can accumulate before joining the oldest.
        if self.reapers.len() >= MAX_TABS {
            let _ = self.reapers.remove(0).join();
        }
        match std::thread::Builder::new()
            .name("well-tab-close".into())
            .spawn(move || session.shutdown())
        {
            Ok(thread) => self.reapers.push(thread),
            Err(error) => eprintln!("Well: could not create shutdown worker: {error}"),
        }
    }
    pub fn shutdown(&mut self) {
        let tabs = std::mem::take(&mut self.tabs);
        for tab in tabs {
            self.reap(tab.session);
        }
        for thread in self.reapers.drain(..) {
            let _ = thread.join();
        }
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        self.shutdown();
    }
}
