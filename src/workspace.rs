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
#[derive(Clone, Debug)]
pub struct TabInfo {
    pub id: u64,
    pub exit_status: Option<u32>,
    pub cwd: Option<PathBuf>,
    pub title: Option<String>,
    pub astraea: well_prompt::AstraeaSnapshot,
}
struct Tab {
    id: u64,
    session: PtySession,
    pending: Arc<AtomicBool>,
    exit_status: Option<u32>,
    cwd: Option<PathBuf>,
    title: Option<String>,
    astraea: well_prompt::AstraeaSnapshot,
}
#[derive(Clone, Debug)]
pub enum Action {
    NewTab,
    SwitchTab(usize),
    CloseTab,
    SendInput(String),
    ExecuteMetisInProcess(String),
    MinimizeWindow,
    MaximizeWindow,
    CloseWindow,
    DragWindow,
    ConnectSsh(well_shell::SshTarget),
    ResizeWindowGrid(u16, u16),
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
    pub history: Arc<std::sync::Mutex<well_shell::MetisHistory>>,
    pub astraea: Arc<well_prompt::AstraeaStateVector>,
    pub astraea_config: well_prompt::AstraeaConfig,
    pub viewport_state: Arc<well_ipc::HermesSeqlock<well_ipc::HermesViewportState>>,
    pub ssh_engine: well_shell::SshEngine,
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
        let mut initial_history = well_shell::MetisHistory::new();
        initial_history.load_all_shell_histories();
        let history = Arc::new(std::sync::Mutex::new(initial_history));
        let astraea = Arc::new(well_prompt::AstraeaStateVector::in_memory());
        let viewport_state = Arc::new(well_ipc::HermesSeqlock::new(
            well_ipc::HermesViewportState::default(),
        ));
        let ssh_engine = well_shell::SshEngine::new();

        let mut workspace = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            proxy,
            socket,
            appearance,
            size: (rows, cols),
            reapers: Vec::new(),
            history,
            astraea,
            astraea_config: well_prompt::AstraeaConfig::default(),
            viewport_state,
            ssh_engine,
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
                cwd: t.cwd.clone(),
                title: t.title.clone(),
                astraea: t.astraea.clone(),
            })
            .collect()
    }
    pub fn update_cwd(&mut self, id: u64, cwd: PathBuf) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            let title = cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            tab.title = if title.is_empty() { None } else { Some(title) };
            tab.cwd = Some(cwd.clone());
            tab.astraea = well_prompt::AstraeaWatcher::scan_path(
                &cwd,
                tab.exit_status.unwrap_or(0) as i32,
                0,
            );
            self.astraea.write_snapshot(&tab.astraea);
        }
    }
    pub fn active_cwd(&self) -> Option<&PathBuf> {
        self.tabs.get(self.active).and_then(|t| t.cwd.as_ref())
    }
    pub fn active_title(&self) -> Option<&str> {
        self.tabs.get(self.active).and_then(|t| t.title.as_deref())
    }
    #[allow(dead_code)]
    pub fn active_astraea(&self) -> Option<&well_prompt::AstraeaSnapshot> {
        self.tabs.get(self.active).map(|t| &t.astraea)
    }
    #[allow(dead_code)]
    pub fn astraea_prompt(&self) -> String {
        let snap = self.astraea.read_snapshot();
        well_prompt::AstraeaCompiler::with_config_and_snapshot(self.astraea_config.clone(), snap)
            .compile_prompt()
    }
    #[allow(dead_code)]
    pub fn astraea_transient_prompt(&self) -> String {
        let snap = self.astraea.read_snapshot();
        well_prompt::AstraeaCompiler::with_config_and_snapshot(self.astraea_config.clone(), snap)
            .compile_transient_prompt()
    }
    #[allow(dead_code)]
    pub fn set_astraea_config(&mut self, config: well_prompt::AstraeaConfig) {
        self.astraea_config = config;
    }
    pub fn sync_viewport_state(&self) {
        if let Some(tab) = self.tabs.get(self.active) {
            let parser = tab.session.parser.lock().unwrap_or_else(|p| p.into_inner());
            let screen = parser.screen();
            let (r, c) = screen.cursor_position();
            let state = well_ipc::HermesViewportState {
                rows: self.size.0,
                cols: self.size.1,
                cursor_row: r,
                cursor_col: c,
                cursor_visible: !screen.hide_cursor(),
                mode_flags: 0,
                scrollback: screen.scrollback() as u32,
                theme_id: 1,
                fg_packed: (self.appearance.foreground[0] as u32) << 16
                    | (self.appearance.foreground[1] as u32) << 8
                    | (self.appearance.foreground[2] as u32),
                bg_packed: (self.appearance.background[0] as u32) << 16
                    | (self.appearance.background[1] as u32) << 8
                    | (self.appearance.background[2] as u32),
                generation: self.viewport_state.sequence(),
                _reserved: [0; 24],
            };
            self.viewport_state.write(state);
        }
    }
    #[allow(dead_code)]
    pub fn history(&self) -> Arc<std::sync::Mutex<well_shell::MetisHistory>> {
        self.history.clone()
    }
    pub fn execute_metis(&mut self, command: &str) -> well_shell::ShellOutput {
        let cwd = self
            .active_cwd()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
        let hist = self
            .history
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let mut executor = well_shell::MetisExecutor::with_history(cwd, hist);
        let output = executor.execute(command);
        let updated_cwd = executor.current_dir;
        let id = self.active_id();
        self.update_cwd(id, updated_cwd);
        *self.history.lock().unwrap_or_else(|p| p.into_inner()) = executor.history;
        output
    }
    pub fn poll_graphic_events(&mut self) -> bool {
        let new_cwd = {
            let Ok(events) = self.tabs[self.active].session.graphic_events_rx.lock() else {
                return false;
            };
            let mut found = None;
            while let Ok(event) = events.try_recv() {
                match event {
                    well_shell::graphics::GraphicEvent::WorkingDirUpdate(uri) => {
                        let path_str = if let Some(stripped) = uri.strip_prefix("file://") {
                            if let Some(slash_idx) = stripped.find('/') {
                                &stripped[slash_idx..]
                            } else {
                                stripped
                            }
                        } else {
                            &uri
                        };
                        let path = PathBuf::from(path_str);
                        if path.exists() {
                            found = Some(path);
                        }
                    }
                    well_shell::graphics::GraphicEvent::Hyperlink(_) => {}
                    well_shell::graphics::GraphicEvent::KittyImage { .. } => {}
                }
            }
            found
        };
        if let Some(path) = new_cwd {
            let id = self.active_id();
            self.update_cwd(id, path);
            true
        } else {
            if let Some(proc_cwd) = self.tabs[self.active].session.get_process_cwd() {
                if self.tabs[self.active].cwd.as_ref() != Some(&proc_cwd) {
                    let id = self.active_id();
                    self.update_cwd(id, proc_cwd);
                    return true;
                }
            }
            false
        }
    }
    pub fn split_pane(&mut self, _horizontal: bool) -> Result<u64, String> {
        self.new_tab()
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

        let initial_cwd = session
            .get_process_cwd()
            .or_else(|| std::env::current_dir().ok());
        let initial_title = initial_cwd
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let astraea_snap = initial_cwd
            .as_ref()
            .map(|p| well_prompt::AstraeaWatcher::scan_path(p, 0, 0))
            .unwrap_or_default();
        self.astraea.write_snapshot(&astraea_snap);

        self.tabs.push(Tab {
            id,
            session,
            pending,
            exit_status: None,
            cwd: initial_cwd,
            title: initial_title,
            astraea: astraea_snap,
        });
        self.active = self.tabs.len() - 1;
        self.sync_viewport_state();
        Ok(id)
    }
    pub fn new_ssh_tab(&mut self, target: well_shell::SshTarget) -> Result<u64, String> {
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

        let ssh_args = target.to_command_args();
        let host_name = target.destination.clone();

        let mut env_vars: Vec<(&str, &std::ffi::OsStr)> = vec![
            ("WELL_RPC_SOCKET", self.socket.as_os_str()),
            ("WELL_SURFACE_ID", std::ffi::OsStr::new(&id_text)),
            ("SSH_CONNECTION", std::ffi::OsStr::new("1")),
            ("SSH_CLIENT", std::ffi::OsStr::new("1")),
        ];

        let askpass_path = "/tmp/well_askpass.sh";
        let askpass_os = std::ffi::OsStr::new(askpass_path);
        let force_os = std::ffi::OsStr::new("force");
        let display_os = std::ffi::OsStr::new(":0");
        let pass_os;
        if let Some(pass) = &target.password {
            if !std::path::Path::new(askpass_path).exists() {
                let _ = std::fs::write(
                    askpass_path,
                    "#!/bin/sh\nprintf '%s\\n' \"$WELL_SSH_PASSWORD\"\n",
                );
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        askpass_path,
                        std::fs::Permissions::from_mode(0o755),
                    );
                }
            }
            pass_os = std::ffi::OsString::from(pass);
            env_vars.push(("SSH_ASKPASS", askpass_os));
            env_vars.push(("SSH_ASKPASS_REQUIRE", force_os));
            env_vars.push(("DISPLAY", display_os));
            env_vars.push(("WELL_SSH_PASSWORD", &pass_os));
        }

        let session = PtySession::spawn_command_with_environment(
            self.size.0,
            self.size.1,
            self.appearance.foreground,
            self.appearance.background,
            "ssh",
            &ssh_args,
            &env_vars,
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

        let title = format!("🌐 {host_name}");
        let astraea = well_prompt::AstraeaSnapshot {
            is_ssh: true,
            git_branch: host_name.clone(),
            ..Default::default()
        };

        self.ssh_engine.register_session(id, target);

        self.tabs.push(Tab {
            id,
            session,
            pending,
            exit_status: None,
            cwd: None,
            title: Some(title),
            astraea,
        });
        self.active = self.tabs.len() - 1;
        self.sync_viewport_state();
        Ok(id)
    }
    pub fn switch_tab(&mut self, index: usize) -> Result<(), String> {
        if index >= self.tabs.len() {
            return Err(format!("Tab index {index} does not exist"));
        }
        self.active = index;
        if let Some(proc_cwd) = self.tabs[self.active].session.get_process_cwd() {
            if self.tabs[self.active].cwd.as_ref() != Some(&proc_cwd) {
                let id = self.active_id();
                self.update_cwd(id, proc_cwd);
            }
        }
        self.astraea.write_snapshot(&self.tabs[self.active].astraea);
        self.sync_viewport_state();
        Ok(())
    }
    /// Returns false after closing the final tab; callers must then close the window.
    pub fn close_tab(&mut self) -> bool {
        let tab = self.tabs.remove(self.active);
        self.ssh_engine.remove_session(tab.id);
        self.reap(tab.session);
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        self.sync_viewport_state();
        !self.tabs.is_empty()
    }
    pub fn rendered(&self) {
        self.tabs[self.active]
            .pending
            .store(false, Ordering::Release);
        self.sync_viewport_state();
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
        self.sync_viewport_state();
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
        self.sync_viewport_state();
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
            json!({
                "index": index,
                "surface_id": t.id,
                "exit_status": t.exit_status,
                "rows": rows,
                "cols": cols,
                "cwd": t.cwd.as_ref().map(|p| p.display().to_string()),
                "title": t.title.as_deref().unwrap_or(""),
            })
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
