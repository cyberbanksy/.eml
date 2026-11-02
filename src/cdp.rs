//! Minimal Chrome DevTools Protocol client over a local WebSocket.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

use crate::error::{Error, Result};

/// Event handler for one session. Returns notifications to send back (method, params).
pub type Handler = Box<dyn FnMut(&Value) -> Vec<(String, Value)>>;

pub struct Browser {
    child: Child,
    log: File,
    profile: PathBuf,
    ws: Option<WebSocket<TcpStream>>,
    next_id: u64,
    handlers: HashMap<String, Handler>,
    timeout: Duration,
    closed: bool,
}

fn cdp_err(msg: impl Into<String>) -> Error {
    Error::Cdp(msg.into())
}

/// Wait for Chrome to publish its DevTools port. Retries on any read error: Windows
/// locks `DevToolsActivePort` while Chrome is still writing it.
pub fn wait_for_port(
    mut read: impl FnMut() -> io::Result<String>,
    mut alive: impl FnMut() -> bool,
    log_tail: impl FnOnce() -> String,
    timeout: Duration,
) -> Result<(u16, String)> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !alive() {
            return Err(cdp_err(format!(
                "Browser exited on startup:\n{}",
                log_tail()
            )));
        }
        if let Ok(text) = read() {
            let mut lines = text.split_whitespace();
            if let (Some(port), Some(path)) = (lines.next(), lines.next())
                && let Ok(port) = port.parse()
            {
                return Ok((port, path.to_string()));
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(cdp_err("Timed out waiting for the browser to start"))
}

fn is_timeout(err: &tungstenite::Error) -> bool {
    matches!(err, tungstenite::Error::Io(e)
        if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut))
}

impl Browser {
    pub fn launch(executable: &Path, timeout: Duration) -> Result<Browser> {
        let profile = tempfile::Builder::new().prefix("doteml-").tempdir()?.keep();
        // A file, not a pipe: Chrome logs a lot and would block once a pipe buffer fills.
        let log = tempfile::tempfile()?;
        let mut cmd = Command::new(executable);
        cmd.args([
            "--headless=new",
            "--remote-debugging-port=0",
            &format!("--user-data-dir={}", profile.display()),
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-sync",
            "--disable-default-apps",
            "--mute-audio",
            "--hide-scrollbars",
            // Background targets otherwise stop producing frames under load (screenshots fail).
            "--disable-renderer-backgrounding",
            "--disable-background-timer-throttling",
            "--disable-backgrounding-occluded-windows",
        ]);
        #[cfg(unix)]
        if unsafe { libc::geteuid() } == 0 {
            cmd.arg("--no-sandbox"); // Chrome refuses to run as root otherwise
        }
        cmd.arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log.try_clone()?));
        let child = cmd.spawn()?;

        let mut browser = Browser {
            child,
            log,
            profile,
            ws: None,
            next_id: 0,
            handlers: HashMap::new(),
            timeout,
            closed: false,
        };
        match browser.connect() {
            Ok(()) => Ok(browser),
            Err(e) => {
                browser.shutdown();
                Err(e)
            }
        }
    }

    fn connect(&mut self) -> Result<()> {
        let marker = self.profile.join("DevToolsActivePort");
        let child = &mut self.child;
        let log = &mut self.log;
        let (port, path) = wait_for_port(
            || std::fs::read_to_string(&marker),
            || matches!(child.try_wait(), Ok(None)),
            || {
                let mut text = String::new();
                let _ = log.seek(SeekFrom::Start(0));
                let _ = log.read_to_string(&mut text);
                let text = text.trim();
                text.chars()
                    .rev()
                    .take(800)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            },
            self.timeout,
        )?;
        let stream = TcpStream::connect(("127.0.0.1", port))?;
        stream.set_read_timeout(Some(self.timeout))?;
        let config = WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None);
        let (ws, _) = tungstenite::client::client_with_config(
            format!("ws://127.0.0.1:{port}{path}"),
            stream,
            Some(config),
        )
        .map_err(|e| cdp_err(format!("DevTools handshake failed: {e}")))?;
        self.ws = Some(ws);
        Ok(())
    }

    fn ws(&mut self) -> Result<&mut WebSocket<TcpStream>> {
        self.ws
            .as_mut()
            .ok_or_else(|| cdp_err("DevTools connection closed"))
    }

    fn write(&mut self, message: Value) -> Result<()> {
        self.ws()?
            .send(Message::text(message.to_string()))
            .map_err(|_| cdp_err("DevTools connection closed"))
    }

    /// Next JSON message, or `None` on a read timeout.
    fn read(&mut self) -> Result<Option<Value>> {
        loop {
            match self.ws()?.read() {
                Ok(Message::Text(text)) => {
                    return serde_json::from_str(&text)
                        .map(Some)
                        .map_err(|e| cdp_err(format!("Bad DevTools message: {e}")));
                }
                Ok(Message::Close(_)) => return Err(cdp_err("DevTools connection closed")),
                Ok(_) => continue,
                Err(e) if is_timeout(&e) => return Ok(None),
                Err(_) => return Err(cdp_err("DevTools connection closed")),
            }
        }
    }

    pub fn send(&mut self, method: &str, params: Value, session: Option<&str>) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session {
            message["sessionId"] = json!(s);
        }
        self.write(message)?;
        let deadline = Instant::now() + self.timeout;
        loop {
            if Instant::now() > deadline {
                return Err(cdp_err(format!("Timed out waiting for {method}")));
            }
            let Some(reply) = self.read()? else {
                continue;
            };
            if reply.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(err) = reply.get("error") {
                    let msg = err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("error");
                    return Err(cdp_err(format!("{method}: {msg}")));
                }
                return Ok(reply.get("result").cloned().unwrap_or(json!({})));
            }
            self.dispatch(&reply)?;
        }
    }

    /// Send without waiting for the reply.
    pub fn notify(&mut self, method: &str, params: Value, session: &str) -> Result<()> {
        self.next_id += 1;
        let message =
            json!({"id": self.next_id, "method": method, "params": params, "sessionId": session});
        self.write(message)
    }

    /// Dispatch events until `done()` is true. `Ok(false)` on timeout.
    pub fn pump_until(
        &mut self,
        mut done: impl FnMut() -> bool,
        timeout: Duration,
    ) -> Result<bool> {
        let deadline = Instant::now() + timeout;
        self.ws()?
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(250)))?;
        let result = (|| {
            while !done() {
                if Instant::now() > deadline {
                    return Ok(false);
                }
                if let Some(message) = self.read()? {
                    self.dispatch(&message)?;
                }
            }
            Ok(true)
        })();
        let full = self.timeout;
        if let Ok(ws) = self.ws() {
            ws.get_ref().set_read_timeout(Some(full))?;
        }
        result
    }

    pub fn on(&mut self, session: &str, handler: Handler) {
        self.handlers.insert(session.to_string(), handler);
    }

    pub fn off(&mut self, session: &str) {
        self.handlers.remove(session);
    }

    fn dispatch(&mut self, message: &Value) -> Result<()> {
        if message.get("method").is_none() {
            return Ok(());
        }
        let session = message
            .get("sessionId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let Some(handler) = self.handlers.get_mut(&session) else {
            return Ok(());
        };
        for (method, params) in handler(message) {
            self.notify(&method, params, &session)?;
        }
        Ok(())
    }

    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if let Some(mut ws) = self.ws.take() {
            let _ = ws.send(Message::text(
                json!({"id": 0, "method": "Browser.close"}).to_string(),
            ));
            let _ = ws.close(None);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if !matches!(self.child.try_wait(), Ok(None)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        // Windows may hold profile files briefly after exit.
        for _ in 0..10 {
            let _ = std::fs::remove_dir_all(&self.profile);
            if !self.profile.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.shutdown();
    }
}
