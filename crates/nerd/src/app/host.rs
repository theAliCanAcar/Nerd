//! A local session host. The window can close without stopping the shell.
//!
//! The same Nerd binary, started with `--host`, owns the PTYs. The window is a
//! client. Reopening Nerd attaches to the shells that are still running.
//! There is no account and no remote machine.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use nerd_terminal::{
    Cell, Color, CursorShape, Key, Modifiers, MouseButton, MouseEvent, SearchMatch, Session,
    Snapshot,
};
use serde_json::{Value, json};

pub(super) trait Shell: Send + Sync {
    fn focus(&self, focused: bool) -> Result<()>;
    fn write(&self, bytes: &[u8]) -> Result<()>;
    fn paste(&self, text: &str) -> Result<()>;
    fn snapshot(&self) -> Snapshot;
    fn resize_with_cell_size(
        &self,
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<()>;
    fn exited(&self) -> bool;
    fn scroll(&self, lines: i32);
    fn scroll_to_bottom(&self);
    fn copy(&self, start: (usize, usize), end: (usize, usize)) -> String;
    fn find(&self, query: &str) -> Vec<SearchMatch>;
    fn seek(&self, offset: usize);
    fn key(&self, key: Key, modifiers: Modifiers) -> Result<()>;
    fn mouse(
        &self,
        event: MouseEvent,
        row: usize,
        col: usize,
        modifiers: Modifiers,
    ) -> Result<bool>;
    fn end(&self);
}

impl Shell for Session {
    fn focus(&self, focused: bool) -> Result<()> {
        Session::focus(self, focused)
    }
    fn write(&self, bytes: &[u8]) -> Result<()> {
        Session::write(self, bytes)
    }
    fn paste(&self, text: &str) -> Result<()> {
        Session::paste(self, text)
    }
    fn snapshot(&self) -> Snapshot {
        Session::snapshot(self)
    }
    fn resize_with_cell_size(
        &self,
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<()> {
        Session::resize_with_cell_size(self, cols, rows, cell_width, cell_height)
    }
    fn exited(&self) -> bool {
        Session::exited(self)
    }
    fn scroll(&self, lines: i32) {
        Session::scroll(self, lines);
    }
    fn scroll_to_bottom(&self) {
        Session::scroll_to_bottom(self);
    }
    fn copy(&self, start: (usize, usize), end: (usize, usize)) -> String {
        Session::copy(self, start, end)
    }
    fn find(&self, query: &str) -> Vec<SearchMatch> {
        Session::find(self, query)
    }
    fn seek(&self, offset: usize) {
        Session::seek(self, offset);
    }
    fn key(&self, key: Key, modifiers: Modifiers) -> Result<()> {
        Session::key(self, key, modifiers)
    }
    fn mouse(
        &self,
        event: MouseEvent,
        row: usize,
        col: usize,
        modifiers: Modifiers,
    ) -> Result<bool> {
        Session::mouse(self, event, row, col, modifiers)
    }
    fn end(&self) {}
}

#[derive(Clone)]
pub(super) struct HostClient {
    inner: Arc<Client>,
}

struct Client {
    next: AtomicU64,
    writer: Mutex<Box<dyn Write + Send>>,
    pending: Mutex<HashMap<u64, Sender<Value>>>,
    snaps: Mutex<HashMap<u64, Snapshot>>,
    repaint: Box<dyn Fn() + Send + Sync>,
}

struct RemoteSession {
    sid: u64,
    client: Arc<Client>,
}

impl Drop for RemoteSession {
    fn drop(&mut self) {
        let _ = self.client.send(json!({"op": "detach", "sid": self.sid}));
    }
}

pub(super) fn serve() {
    #[cfg(unix)]
    {
        // Leave the window's session so closing Nerd does not signal this process.
        unsafe extern "C" {
            fn setsid() -> i32;
        }
        unsafe {
            setsid();
        }
        if let Err(error) = run_server() {
            eprintln!("Nerd session host stopped: {error:#}");
        }
    }
}

pub(super) fn connect(ctx: &eframe::egui::Context) -> Option<HostClient> {
    #[cfg(not(unix))]
    {
        let _ = ctx;
        return None;
    }
    #[cfg(unix)]
    {
        let repaint = {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        };
        if let Some(client) = try_connect(&repaint) {
            return Some(client);
        }
        let _ = std::fs::remove_file(socket_path());
        spawn_host();
        for _ in 0..25 {
            if let Some(client) = try_connect(&repaint) {
                return Some(client);
            }
            thread::sleep(Duration::from_millis(40));
        }
        None
    }
}

impl HostClient {
    pub(super) fn open(&self, cols: u16, rows: u16, cwd: Option<&Path>) -> Result<Arc<dyn Shell>> {
        let reply = self.inner.call(json!({
            "op": "open",
            "cols": cols,
            "rows": rows,
            "cwd": cwd.map(|path| path.to_string_lossy()),
        }))?;
        let sid = reply.get("sid").and_then(Value::as_u64).context("session id")?;
        store_snap(&self.inner, sid, &reply);
        Ok(Arc::new(RemoteSession {
            sid,
            client: self.inner.clone(),
        }))
    }

    pub(super) fn running(&self) -> Vec<Arc<dyn Shell>> {
        let Ok(reply) = self.inner.call(json!({"op": "list"})) else {
            return Vec::new();
        };
        let mut shells = Vec::new();
        for sid in reply
            .get("sessions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
        {
            if self.inner.call(json!({"op": "attach", "sid": sid})).is_err() {
                continue;
            }
            shells.push(Arc::new(RemoteSession {
                sid,
                client: self.inner.clone(),
            }) as Arc<dyn Shell>);
        }
        shells
    }
}

impl Client {
    #[cfg(unix)]
    fn start(
        stream: std::os::unix::net::UnixStream,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Result<Arc<Self>> {
        let writer = stream.try_clone().context("clone socket")?;
        let client = Arc::new(Self {
            next: AtomicU64::new(1),
            writer: Mutex::new(Box::new(writer)),
            pending: Mutex::new(HashMap::new()),
            snaps: Mutex::new(HashMap::new()),
            repaint: Box::new(repaint),
        });
        let reader = client.clone();
        thread::Builder::new()
            .name("nerd-host-read".into())
            .spawn(move || read_host(stream, reader))
            .context("start host reader")?;
        Ok(client)
    }

    fn call(&self, mut value: Value) -> Result<Value> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        value["id"] = json!(id);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().expect("pending").insert(id, tx);
        self.send(value)?;
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(value) => {
                if value.get("ok").and_then(Value::as_bool) == Some(false) {
                    bail!(
                        "{}",
                        value.get("error").and_then(Value::as_str).unwrap_or("host error")
                    );
                }
                Ok(value)
            }
            Err(_) => {
                self.pending.lock().expect("pending").remove(&id);
                bail!("the session host did not answer")
            }
        }
    }

    fn send(&self, value: Value) -> Result<()> {
        let mut writer = self.writer.lock().expect("writer");
        serde_json::to_writer(&mut *writer, &value).context("write host request")?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }
}

fn read_host(stream: impl std::io::Read + Send, client: Arc<Client>) {
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("op").and_then(Value::as_str) == Some("snap") {
            if let Some(sid) = value.get("sid").and_then(Value::as_u64) {
                store_snap(&client, sid, &value);
                (client.repaint)();
            }
            continue;
        }
        if let Some(id) = value.get("id").and_then(Value::as_u64) {
            if let Some(tx) = client.pending.lock().expect("pending").remove(&id) {
                let _ = tx.send(value);
            }
        }
    }
}

fn store_snap(client: &Client, sid: u64, value: &Value) {
    if let Some(snap) = value.get("snap").and_then(parse_snapshot) {
        client.snaps.lock().expect("snaps").insert(sid, snap);
    }
}

impl Shell for RemoteSession {
    fn focus(&self, focused: bool) -> Result<()> {
        self.client.call(json!({"op": "focus", "sid": self.sid, "focused": focused})).map(|_| ())
    }
    fn write(&self, bytes: &[u8]) -> Result<()> {
        self.client
            .call(json!({"op": "write", "sid": self.sid, "bytes": bytes}))
            .map(|_| ())
    }
    fn paste(&self, text: &str) -> Result<()> {
        self.client
            .call(json!({"op": "paste", "sid": self.sid, "text": text}))
            .map(|_| ())
    }
    fn snapshot(&self) -> Snapshot {
        self.client
            .snaps
            .lock()
            .expect("snaps")
            .get(&self.sid)
            .cloned()
            .unwrap_or_else(blank_snapshot)
    }
    fn resize_with_cell_size(
        &self,
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<()> {
        self.client
            .call(json!({
                "op": "resize",
                "sid": self.sid,
                "cols": cols,
                "rows": rows,
                "cell_width": cell_width,
                "cell_height": cell_height,
            }))
            .map(|_| ())
    }
    fn exited(&self) -> bool {
        self.snapshot().exited
    }
    fn scroll(&self, lines: i32) {
        if let Ok(reply) = self.client.call(json!({"op": "scroll", "sid": self.sid, "lines": lines})) {
            store_snap(&self.client, self.sid, &reply);
        }
    }
    fn scroll_to_bottom(&self) {
        if let Ok(reply) = self.client.call(json!({"op": "bottom", "sid": self.sid})) {
            store_snap(&self.client, self.sid, &reply);
        }
    }
    fn copy(&self, start: (usize, usize), end: (usize, usize)) -> String {
        self.client
            .call(json!({
                "op": "copy",
                "sid": self.sid,
                "start": [start.0, start.1],
                "end": [end.0, end.1],
            }))
            .ok()
            .and_then(|value| value.get("text").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_default()
    }
    fn find(&self, query: &str) -> Vec<SearchMatch> {
        self.client
            .call(json!({"op": "find", "sid": self.sid, "query": query}))
            .ok()
            .map(|value| parse_matches(&value))
            .unwrap_or_default()
    }
    fn seek(&self, offset: usize) {
        if let Ok(reply) = self.client.call(json!({"op": "seek", "sid": self.sid, "offset": offset})) {
            store_snap(&self.client, self.sid, &reply);
        }
    }
    fn key(&self, key: Key, modifiers: Modifiers) -> Result<()> {
        self.client
            .call(json!({
                "op": "key",
                "sid": self.sid,
                "key": key_value(key),
                "shift": modifiers.shift,
                "alt": modifiers.alt,
                "ctrl": modifiers.ctrl,
            }))
            .map(|_| ())
    }
    fn mouse(
        &self,
        event: MouseEvent,
        row: usize,
        col: usize,
        modifiers: Modifiers,
    ) -> Result<bool> {
        let reply = self.client.call(json!({
            "op": "mouse",
            "sid": self.sid,
            "event": mouse_value(event),
            "row": row,
            "col": col,
            "shift": modifiers.shift,
            "alt": modifiers.alt,
            "ctrl": modifiers.ctrl,
        }))?;
        Ok(reply.get("handled").and_then(Value::as_bool).unwrap_or(false))
    }
    fn end(&self) {
        let _ = self.client.call(json!({"op": "end", "sid": self.sid}));
    }
}

fn blank_snapshot() -> Snapshot {
    Snapshot {
        cols: 80,
        rows: 24,
        cells: Vec::new(),
        cursor: None,
        cursor_shape: CursorShape::Block,
        title: None,
        exited: false,
        exit_code: None,
        history_size: 0,
        scrollback_offset: 0,
    }
}

struct Slot {
    session: Session,
    dirty: Arc<AtomicBool>,
    generation: AtomicU64,
    latest: Mutex<Option<Snapshot>>,
    refresh: Mutex<()>,
}

fn sessions() -> &'static Mutex<HashMap<u64, Arc<Slot>>> {
    use std::sync::OnceLock;
    static SESSIONS: OnceLock<Mutex<HashMap<u64, Arc<Slot>>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_sid() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(unix)]
fn run_server() -> Result<()> {
    use std::os::unix::net::UnixListener;
    let path = socket_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        thread::spawn(move || handle_client(stream));
    }
    Ok(())
}

#[cfg(unix)]
fn handle_client(stream: std::os::unix::net::UnixStream) {
    let Ok(write) = stream.try_clone() else { return };
    let write = Arc::new(Mutex::new(write));
    let subscribed: Arc<Mutex<HashMap<u64, u64>>> = Arc::new(Mutex::new(HashMap::new()));
    let alive = Arc::new(AtomicBool::new(true));
    let push_write = write.clone();
    let push_ids = subscribed.clone();
    let push_alive = alive.clone();
    thread::spawn(move || push_snapshots(push_write, push_ids, push_alive));
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let reply = dispatch(&value, &subscribed);
        if let Ok(mut writer) = write.lock() {
            let _ = serde_json::to_writer(&mut *writer, &reply);
            let _ = writer.write_all(b"\n");
            let _ = writer.flush();
        }
    }
    alive.store(false, Ordering::Relaxed);
}

fn push_snapshots(
    write: Arc<Mutex<impl Write>>,
    subscribed: Arc<Mutex<HashMap<u64, u64>>>,
    alive: Arc<AtomicBool>,
) {
    while alive.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(16));
        let ids: Vec<(u64, u64)> = subscribed
            .lock()
            .expect("subscribed")
            .iter()
            .map(|(sid, seen)| (*sid, *seen))
            .collect();
        for (sid, seen) in ids {
            let Some(slot) = sessions().lock().expect("sessions").get(&sid).cloned() else {
                continue;
            };
            let generation = publish(&slot);
            if generation == seen {
                continue;
            }
            let Some(snap) = slot.latest.lock().expect("latest").clone() else {
                continue;
            };
            subscribed.lock().expect("subscribed").insert(sid, generation);
            let message = json!({"op": "snap", "sid": sid, "snap": snapshot_value(&snap)});
            let Ok(mut writer) = write.lock() else { return };
            if serde_json::to_writer(&mut *writer, &message).is_err()
                || writer.write_all(b"\n").is_err()
                || writer.flush().is_err()
            {
                return;
            }
        }
    }
}

#[cfg(unix)]
fn try_connect(repaint: &(impl Fn() + Send + Sync + Clone + 'static)) -> Option<HostClient> {
    let stream = std::os::unix::net::UnixStream::connect(socket_path()).ok()?;
    let client = Client::start(stream, repaint.clone()).ok()?;
    Some(HostClient { inner: client })
}

fn publish(slot: &Slot) -> u64 {
    let _guard = slot.refresh.lock().expect("refresh");
    if slot.dirty.swap(false, Ordering::AcqRel) {
        let mut snap = slot.session.snapshot();
        if slot.dirty.swap(false, Ordering::AcqRel) {
            snap = slot.session.snapshot();
        }
        *slot.latest.lock().expect("latest") = Some(snap);
        slot.generation.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        slot.generation.load(Ordering::Relaxed)
    }
}

fn dispatch(value: &Value, subscribed: &Mutex<HashMap<u64, u64>>) -> Value {
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    match handle(value, subscribed) {
        Ok(mut reply) => {
            reply["id"] = id;
            reply["ok"] = json!(true);
            reply
        }
        Err(error) => json!({"id": id, "ok": false, "error": format!("{error:#}")}),
    }
}

fn handle(value: &Value, subscribed: &Mutex<HashMap<u64, u64>>) -> Result<Value> {
    match value.get("op").and_then(Value::as_str).unwrap_or("") {
        "list" => {
            let mut ids: Vec<u64> = sessions().lock().expect("sessions").keys().copied().collect();
            ids.sort_unstable();
            Ok(json!({"sessions": ids}))
        }
        "open" => {
            let cols = value.get("cols").and_then(Value::as_u64).unwrap_or(80) as u16;
            let rows = value.get("rows").and_then(Value::as_u64).unwrap_or(24) as u16;
            let cwd = value
                .get("cwd")
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .filter(|path| path.is_dir());
            let dirty = Arc::new(AtomicBool::new(true));
            let flag = Arc::clone(&dirty);
            let session = Session::spawn(cols.max(2), rows.max(1), cwd.as_deref(), move || {
                flag.store(true, Ordering::Relaxed);
            })?;
            let sid = next_sid();
            let slot = Arc::new(Slot {
                session,
                dirty,
                generation: AtomicU64::new(0),
                latest: Mutex::new(None),
                refresh: Mutex::new(()),
            });
            sessions().lock().expect("sessions").insert(sid, slot.clone());
            subscribed.lock().expect("subscribed").insert(sid, 0);
            let generation = publish(&slot);
            subscribed.lock().expect("subscribed").insert(sid, generation);
            let snap = slot.latest.lock().expect("latest").clone();
            Ok(json!({"sid": sid, "snap": snap.as_ref().map(snapshot_value)}))
        }
        "attach" => {
            let sid = sid(value)?;
            if !sessions().lock().expect("sessions").contains_key(&sid) {
                bail!("that session is gone");
            }
            subscribed.lock().expect("subscribed").insert(sid, 0);
            Ok(json!({}))
        }
        "detach" => {
            if let Ok(sid) = sid(value) {
                subscribed.lock().expect("subscribed").remove(&sid);
            }
            Ok(json!({}))
        }
        "end" => {
            let sid = sid(value)?;
            sessions().lock().expect("sessions").remove(&sid);
            subscribed.lock().expect("subscribed").remove(&sid);
            Ok(json!({}))
        }
        "write" => {
            let bytes = value
                .get("bytes")
                .and_then(Value::as_array)
                .context("bytes")?
                .iter()
                .filter_map(Value::as_u64)
                .map(|byte| byte as u8)
                .collect::<Vec<_>>();
            slot(value)?.session.write(&bytes)?;
            Ok(json!({}))
        }
        "paste" => {
            slot(value)?.session.paste(value.get("text").and_then(Value::as_str).unwrap_or(""))?;
            Ok(json!({}))
        }
        "key" => {
            slot(value)?.session.key(parse_key(value.get("key"))?, modifiers(value))?;
            Ok(json!({}))
        }
        "mouse" => {
            let handled = slot(value)?.session.mouse(
                parse_mouse(value.get("event"))?,
                number(value, "row") as usize,
                number(value, "col") as usize,
                modifiers(value),
            )?;
            Ok(json!({"handled": handled}))
        }
        "resize" => {
            slot(value)?.session.resize_with_cell_size(
                number(value, "cols") as u16,
                number(value, "rows") as u16,
                number(value, "cell_width") as u16,
                number(value, "cell_height") as u16,
            )?;
            Ok(json!({}))
        }
        "scroll" => {
            let slot = slot(value)?;
            let lines = value.get("lines").and_then(Value::as_i64).unwrap_or(0) as i32;
            slot.session.scroll(lines);
            Ok(view_of(&slot))
        }
        "bottom" => {
            let slot = slot(value)?;
            slot.session.scroll_to_bottom();
            Ok(view_of(&slot))
        }
        "seek" => {
            let slot = slot(value)?;
            slot.session.seek(number(value, "offset") as usize);
            Ok(view_of(&slot))
        }
        "focus" => {
            slot(value)?.session.focus(value.get("focused").and_then(Value::as_bool).unwrap_or(false))?;
            Ok(json!({}))
        }
        "copy" => {
            let start = pair(value, "start")?;
            let end = pair(value, "end")?;
            let text = slot(value)?.session.copy(start, end);
            Ok(json!({"text": text}))
        }
        "find" => {
            let matches = slot(value)?.session.find(value.get("query").and_then(Value::as_str).unwrap_or(""));
            let matches = matches
                .into_iter()
                .map(|item| {
                    json!({
                        "offset": item.scrollback_offset,
                        "row": item.row,
                        "col": item.col,
                        "len": item.len,
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({"matches": matches}))
        }
        _ => bail!("unknown request"),
    }
}

fn view_of(slot: &Slot) -> Value {
    slot.dirty.store(true, Ordering::Relaxed);
    json!({"snap": snapshot_value(&slot.session.snapshot())})
}

fn slot(value: &Value) -> Result<Arc<Slot>> {
    let sid = sid(value)?;
    sessions()
        .lock()
        .expect("sessions")
        .get(&sid)
        .cloned()
        .context("that session is gone")
}

fn sid(value: &Value) -> Result<u64> {
    value.get("sid").and_then(Value::as_u64).context("session id")
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn pair(value: &Value, key: &str) -> Result<(usize, usize)> {
    let items = value.get(key).and_then(Value::as_array).context("range")?;
    Ok((
        items.first().and_then(Value::as_u64).unwrap_or(0) as usize,
        items.get(1).and_then(Value::as_u64).unwrap_or(0) as usize,
    ))
}

fn modifiers(value: &Value) -> Modifiers {
    Modifiers {
        shift: value.get("shift").and_then(Value::as_bool).unwrap_or(false),
        alt: value.get("alt").and_then(Value::as_bool).unwrap_or(false),
        ctrl: value.get("ctrl").and_then(Value::as_bool).unwrap_or(false),
    }
}

fn key_value(key: Key) -> Value {
    match key {
        Key::Character(c) => json!({"kind": "char", "c": c.to_string()}),
        Key::Function(n) => json!({"kind": "fn", "n": n}),
        Key::Up => json!({"kind": "up"}),
        Key::Down => json!({"kind": "down"}),
        Key::Left => json!({"kind": "left"}),
        Key::Right => json!({"kind": "right"}),
        Key::Home => json!({"kind": "home"}),
        Key::End => json!({"kind": "end"}),
        Key::PageUp => json!({"kind": "pageup"}),
        Key::PageDown => json!({"kind": "pagedown"}),
        Key::Insert => json!({"kind": "insert"}),
        Key::Delete => json!({"kind": "delete"}),
        Key::Enter => json!({"kind": "enter"}),
        Key::Tab => json!({"kind": "tab"}),
        Key::Backspace => json!({"kind": "backspace"}),
        Key::Escape => json!({"kind": "escape"}),
    }
}

fn parse_key(value: Option<&Value>) -> Result<Key> {
    let value = value.context("key")?;
    match value.get("kind").and_then(Value::as_str).unwrap_or("") {
        "char" => Ok(Key::Character(
            value
                .get("c")
                .and_then(Value::as_str)
                .and_then(|text| text.chars().next())
                .unwrap_or(' '),
        )),
        "fn" => Ok(Key::Function(value.get("n").and_then(Value::as_u64).unwrap_or(1) as u8)),
        "up" => Ok(Key::Up),
        "down" => Ok(Key::Down),
        "left" => Ok(Key::Left),
        "right" => Ok(Key::Right),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" => Ok(Key::PageUp),
        "pagedown" => Ok(Key::PageDown),
        "insert" => Ok(Key::Insert),
        "delete" => Ok(Key::Delete),
        "enter" => Ok(Key::Enter),
        "tab" => Ok(Key::Tab),
        "backspace" => Ok(Key::Backspace),
        "escape" => Ok(Key::Escape),
        _ => bail!("unknown key"),
    }
}

fn mouse_value(event: MouseEvent) -> Value {
    match event {
        MouseEvent::Press(button) => json!({"kind": "press", "button": button_name(button)}),
        MouseEvent::Release(button) => json!({"kind": "release", "button": button_name(button)}),
        MouseEvent::Move(button) => json!({"kind": "move", "button": button.map(button_name)}),
        MouseEvent::WheelUp => json!({"kind": "wheelup"}),
        MouseEvent::WheelDown => json!({"kind": "wheeldown"}),
    }
}

fn button_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Middle => "middle",
        MouseButton::Right => "right",
    }
}

fn parse_mouse(value: Option<&Value>) -> Result<MouseEvent> {
    let value = value.context("mouse")?;
    let button = value.get("button").and_then(Value::as_str).and_then(parse_button);
    match value.get("kind").and_then(Value::as_str).unwrap_or("") {
        "press" => Ok(MouseEvent::Press(button.unwrap_or(MouseButton::Left))),
        "release" => Ok(MouseEvent::Release(button.unwrap_or(MouseButton::Left))),
        "move" => Ok(MouseEvent::Move(button)),
        "wheelup" => Ok(MouseEvent::WheelUp),
        "wheeldown" => Ok(MouseEvent::WheelDown),
        _ => bail!("unknown mouse event"),
    }
}

fn parse_button(name: &str) -> Option<MouseButton> {
    match name {
        "left" => Some(MouseButton::Left),
        "middle" => Some(MouseButton::Middle),
        "right" => Some(MouseButton::Right),
        _ => None,
    }
}

fn snapshot_value(snap: &Snapshot) -> Value {
    json!({
        "cols": snap.cols,
        "rows": snap.rows,
        "cursor": snap.cursor,
        "shape": shape_name(snap.cursor_shape),
        "title": snap.title,
        "exited": snap.exited,
        "exit_code": snap.exit_code,
        "history": snap.history_size,
        "offset": snap.scrollback_offset,
        "cells": snap.cells.iter().map(cell_value).collect::<Vec<_>>(),
    })
}

fn cell_value(cell: &Cell) -> Value {
    json!([
        cell.text,
        color_value(cell.fg),
        color_value(cell.bg),
        cell.bold,
        cell.italic,
        cell.underline,
        cell.inverse,
        cell.dim,
        cell.hidden,
        cell.strikeout,
        cell.wide,
        cell.spacer,
    ])
}

fn color_value(color: Color) -> String {
    match color {
        Color::Default => "d".into(),
        Color::Indexed(index) => format!("i{index}"),
        Color::Rgb([r, g, b]) => format!("r{r},{g},{b}"),
    }
}

fn shape_name(shape: CursorShape) -> &'static str {
    match shape {
        CursorShape::Hidden => "hidden",
        CursorShape::Block => "block",
        CursorShape::Underline => "underline",
        CursorShape::Beam => "beam",
        CursorShape::HollowBlock => "hollow",
    }
}

fn parse_snapshot(value: &Value) -> Option<Snapshot> {
    let cells = value
        .get("cells")?
        .as_array()?
        .iter()
        .map(parse_cell)
        .collect::<Option<Vec<_>>>()?;
    let cursor = value.get("cursor").and_then(|cursor| {
        let pair = cursor.as_array()?;
        Some((
            pair.first()?.as_u64()? as usize,
            pair.get(1)?.as_u64()? as usize,
        ))
    });
    Some(Snapshot {
        cols: value.get("cols").and_then(Value::as_u64).unwrap_or(80) as usize,
        rows: value.get("rows").and_then(Value::as_u64).unwrap_or(24) as usize,
        cells,
        cursor,
        cursor_shape: parse_shape(value.get("shape").and_then(Value::as_str).unwrap_or("block")),
        title: value.get("title").and_then(Value::as_str).map(str::to_string),
        exited: value.get("exited").and_then(Value::as_bool).unwrap_or(false),
        exit_code: value.get("exit_code").and_then(Value::as_i64).map(|code| code as i32),
        history_size: value.get("history").and_then(Value::as_u64).unwrap_or(0) as usize,
        scrollback_offset: value.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize,
    })
}

fn parse_cell(value: &Value) -> Option<Cell> {
    let items = value.as_array()?;
    Some(Cell {
        text: items.first().and_then(Value::as_str).unwrap_or("").to_string(),
        fg: parse_color(items.get(1).and_then(Value::as_str).unwrap_or("d")),
        bg: parse_color(items.get(2).and_then(Value::as_str).unwrap_or("d")),
        bold: flag(items, 3),
        italic: flag(items, 4),
        underline: flag(items, 5),
        inverse: flag(items, 6),
        dim: flag(items, 7),
        hidden: flag(items, 8),
        strikeout: flag(items, 9),
        wide: flag(items, 10),
        spacer: flag(items, 11),
    })
}

fn flag(items: &[Value], index: usize) -> bool {
    items.get(index).and_then(Value::as_bool).unwrap_or(false)
}

fn parse_color(text: &str) -> Color {
    if let Some(index) = text.strip_prefix('i') {
        return Color::Indexed(index.parse().unwrap_or(0));
    }
    if let Some(rgb) = text.strip_prefix('r') {
        let mut parts = rgb.split(',');
        let r = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
        let g = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
        let b = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
        return Color::Rgb([r, g, b]);
    }
    Color::Default
}

fn parse_shape(name: &str) -> CursorShape {
    match name {
        "hidden" => CursorShape::Hidden,
        "underline" => CursorShape::Underline,
        "beam" => CursorShape::Beam,
        "hollow" => CursorShape::HollowBlock,
        _ => CursorShape::Block,
    }
}

fn parse_matches(value: &Value) -> Vec<SearchMatch> {
    value
        .get("matches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(SearchMatch {
                scrollback_offset: item.get("offset").and_then(Value::as_u64)? as usize,
                row: item.get("row").and_then(Value::as_u64)? as usize,
                col: item.get("col").and_then(Value::as_u64)? as usize,
                len: item.get("len").and_then(Value::as_u64)? as usize,
            })
        })
        .collect()
}

#[cfg(unix)]
fn socket_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Nerd/host.sock")
    } else {
        home.join(".local/share/nerd/host.sock")
    }
}

#[cfg(unix)]
fn spawn_host() {
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    let Ok(exe) = std::env::current_exe() else { return };
    let mut command = Command::new(exe);
    command.arg("--host").stdin(std::process::Stdio::null());
    unsafe {
        command.pre_exec(|| {
            unsafe extern "C" {
                fn setsid() -> i32;
            }
            setsid();
            Ok(())
        });
    }
    let _ = command.spawn();
}

