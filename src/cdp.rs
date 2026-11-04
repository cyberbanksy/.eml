//! Minimal Chrome DevTools Protocol client.
//!
//! On Unix, Chrome is driven over a private pipe (`--remote-debugging-pipe`), so no
//! local TCP port is ever open while an email renders. Windows uses the DevTools
//! WebSocket on 127.0.0.1, because pipe mode relies on inherited Unix descriptors.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::error::{Error, Result};

/// Event handler for one session. Returns notifications to send back (method, params).
pub type Handler = Box<dyn FnMut(&Value) -> Vec<(String, Value)>>;

const MAX_MESSAGE_BYTES: usize = 256 * 1024 * 1024;

pub struct Browser {
    child: Child,
    log: File,
    profile: PathBuf,
    transport: Option<Transport>,
    next_id: u64,
    handlers: HashMap<String, Handler>,
    timeout: Duration,
    closed: bool,
}

fn cdp_err(msg: impl Into<String>) -> Error {
    Error::Cdp(msg.into())
}

fn closed() -> Error {
    cdp_err("DevTools connection closed")
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

// --- transports ---

enum Transport {
    #[cfg(unix)]
    Pipe(pipe::Pipe),
    #[cfg(not(unix))]
    Ws(ws::Ws),
}

impl Transport {
    fn send(&mut self, text: &str) -> Result<()> {
        match self {
            #[cfg(unix)]
            Transport::Pipe(p) => p.send(text),
            #[cfg(not(unix))]
            Transport::Ws(w) => w.send(text),
        }
    }

    /// Next message, or `None` if nothing arrived within `timeout`.
    fn recv(&mut self, timeout: Duration) -> Result<Option<String>> {
        match self {
            #[cfg(unix)]
            Transport::Pipe(p) => p.recv(timeout),
            #[cfg(not(unix))]
            Transport::Ws(w) => w.recv(timeout),
        }
    }

    fn close(self) {
        match self {
            #[cfg(unix)]
            Transport::Pipe(p) => p.close(),
            #[cfg(not(unix))]
            Transport::Ws(w) => w.close(),
        }
    }
}

#[cfg(unix)]
mod pipe {
    use std::fs::File;
    use std::io::Read;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::process::Command;
    use std::time::Duration;

    use super::{MAX_MESSAGE_BYTES, cdp_err, closed};
    use crate::error::Result;

    /// Chrome reads JSON messages from fd 3 and writes them to fd 4, each `\0`-terminated.
    pub struct Pipe {
        reader: File,
        writer: Option<File>,
        buf: Vec<u8>,
        /// The child's ends, held open until spawn so pre_exec can dup2 them.
        child_ends: Option<(OwnedFd, OwnedFd)>,
    }

    fn pair() -> std::io::Result<(OwnedFd, OwnedFd)> {
        let mut fds = [0; 2];
        // SAFETY: `fds` is a valid out-pointer for two descriptors.
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: the descriptors were just created and are owned by nobody else.
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        for fd in [&r, &w] {
            // Don't leak the parent's ends into the child; pre_exec dup2s the right ones.
            if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok((r, w))
    }

    /// Set up `cmd` to talk over fds 3/4 and return the parent's ends.
    pub fn attach(cmd: &mut Command) -> std::io::Result<Pipe> {
        use std::os::unix::process::CommandExt;
        let (chrome_reads, we_write) = pair()?;
        let (we_read, chrome_writes) = pair()?;
        let (in_fd, out_fd) = (chrome_reads.as_raw_fd(), chrome_writes.as_raw_fd());
        cmd.arg("--remote-debugging-pipe");
        // SAFETY: only async-signal-safe calls (fcntl, dup2, close) run between fork and exec.
        unsafe {
            cmd.pre_exec(move || {
                // The pipe ends may themselves be fds 3/4 (dup2 onto itself keeps CLOEXEC
                // and the fd would vanish at exec), so park copies high up first.
                let a = libc::fcntl(in_fd, libc::F_DUPFD, 10);
                let b = libc::fcntl(out_fd, libc::F_DUPFD, 10);
                if a < 0 || b < 0 || libc::dup2(a, 3) < 0 || libc::dup2(b, 4) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                libc::close(a);
                libc::close(b);
                Ok(())
            });
        }
        Ok(Pipe {
            reader: File::from(we_read),
            writer: Some(File::from(we_write)),
            buf: Vec::new(),
            child_ends: Some((chrome_reads, chrome_writes)),
        })
    }

    impl Pipe {
        /// Drop the child's ends after spawn, so EOF reaches us when Chrome exits.
        pub fn release_child_ends(&mut self) {
            self.child_ends.take();
        }

        pub fn send(&mut self, text: &str) -> Result<()> {
            use std::io::Write;
            let w = self.writer.as_mut().ok_or_else(closed)?;
            w.write_all(text.as_bytes())
                .and_then(|()| w.write_all(b"\0"))
                .map_err(|_| closed())
        }

        pub fn recv(&mut self, timeout: Duration) -> Result<Option<String>> {
            let deadline = std::time::Instant::now() + timeout;
            loop {
                if let Some(pos) = self.buf.iter().position(|&b| b == 0) {
                    let msg: Vec<u8> = self.buf.drain(..=pos).collect();
                    return Ok(Some(
                        String::from_utf8_lossy(&msg[..msg.len() - 1]).into_owned(),
                    ));
                }
                if self.buf.len() > MAX_MESSAGE_BYTES {
                    return Err(cdp_err("DevTools message too large"));
                }
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() {
                    return Ok(None);
                }
                let mut pfd = libc::pollfd {
                    fd: self.reader.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: `pfd` is a valid pollfd for the duration of the call.
                let ready = unsafe {
                    libc::poll(&mut pfd, 1, left.as_millis().min(i32::MAX as u128) as i32)
                };
                if ready == 0 {
                    return Ok(None);
                }
                if ready < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(err.into());
                }
                let mut chunk = [0u8; 65536];
                match self.reader.read(&mut chunk) {
                    Ok(0) => return Err(closed()),
                    Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err(closed()),
                }
            }
        }

        pub fn close(mut self) {
            self.writer.take(); // EOF on fd 3 tells Chrome to shut down
        }
    }
}

#[cfg(not(unix))]
mod ws {
    use std::net::TcpStream;
    use std::time::Duration;

    use tungstenite::protocol::WebSocketConfig;
    use tungstenite::{Message, WebSocket};

    use super::{MAX_MESSAGE_BYTES, cdp_err, closed};
    use crate::error::Result;

    pub struct Ws(WebSocket<TcpStream>);

    pub fn connect(port: u16, path: &str, timeout: Duration) -> Result<Ws> {
        let stream = TcpStream::connect(("127.0.0.1", port))?;
        stream.set_read_timeout(Some(timeout))?;
        // Screenshots of long emails arrive as one large base64 message; cap, don't disable.
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (ws, _) = tungstenite::client::client_with_config(
            format!("ws://127.0.0.1:{port}{path}"),
            stream,
            Some(config),
        )
        .map_err(|e| cdp_err(format!("DevTools handshake failed: {e}")))?;
        Ok(Ws(ws))
    }

    fn is_timeout(err: &tungstenite::Error) -> bool {
        matches!(err, tungstenite::Error::Io(e)
            if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
    }

    impl Ws {
        pub fn send(&mut self, text: &str) -> Result<()> {
            self.0.send(Message::text(text)).map_err(|_| closed())
        }

        pub fn recv(&mut self, timeout: Duration) -> Result<Option<String>> {
            self.0.get_ref().set_read_timeout(Some(timeout))?;
            loop {
                match self.0.read() {
                    Ok(Message::Text(text)) => return Ok(Some(text.to_string())),
                    Ok(Message::Close(_)) => return Err(closed()),
                    Ok(_) => continue,
                    Err(e) if is_timeout(&e) => return Ok(None),
                    Err(_) => return Err(closed()),
                }
            }
        }

        pub fn close(mut self) {
            let _ = self.0.close(None);
        }
    }
}

impl Browser {
    pub fn launch(executable: &Path, timeout: Duration) -> Result<Browser> {
        let profile = crate::fsutil::private_tempdir("doteml-")?.keep();
        // A file, not a pipe: Chrome logs a lot and would block once a pipe buffer fills.
        let log = tempfile::tempfile()?;
        let mut cmd = Command::new(executable);
        cmd.args([
            "--headless=new",
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
        #[cfg(not(unix))]
        cmd.arg("--remote-debugging-port=0");
        #[cfg(unix)]
        let pipe_transport = pipe::attach(&mut cmd)?;
        cmd.arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log.try_clone()?));
        let child = cmd.spawn()?;
        #[cfg(unix)]
        let mut pipe_transport = pipe_transport;
        #[cfg(unix)]
        pipe_transport.release_child_ends();

        let mut browser = Browser {
            child,
            log,
            profile,
            transport: None,
            next_id: 0,
            handlers: HashMap::new(),
            timeout,
            closed: false,
        };
        #[cfg(unix)]
        {
            browser.transport = Some(Transport::Pipe(pipe_transport));
        }
        match browser.connect() {
            Ok(()) => Ok(browser),
            Err(e) => {
                browser.shutdown();
                Err(e)
            }
        }
    }

    fn log_tail(&mut self) -> String {
        let mut text = String::new();
        let _ = self.log.seek(SeekFrom::Start(0));
        let _ = self.log.read_to_string(&mut text);
        let text = text.trim();
        text.chars()
            .rev()
            .take(800)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    #[cfg(unix)]
    fn connect(&mut self) -> Result<()> {
        // The pipe is live from the start; a failed launch shows up as EOF on the first read.
        if self.child_exited() {
            return Err(cdp_err(format!(
                "Browser exited on startup:\n{}",
                self.log_tail()
            )));
        }
        Ok(())
    }

    #[cfg(not(unix))]
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
        self.transport = Some(Transport::Ws(ws::connect(port, &path, self.timeout)?));
        Ok(())
    }

    fn transport(&mut self) -> Result<&mut Transport> {
        self.transport.as_mut().ok_or_else(closed)
    }

    fn write(&mut self, message: Value) -> Result<()> {
        self.transport()?.send(&message.to_string())
    }

    /// Next JSON message, or `None` if nothing arrived within `timeout`.
    fn read(&mut self, timeout: Duration) -> Result<Option<Value>> {
        let text = match self.transport()?.recv(timeout) {
            Ok(Some(text)) => text,
            Ok(None) => return Ok(None),
            Err(e) => {
                if self.child_exited() {
                    return Err(cdp_err(format!("Browser exited:\n{}", self.log_tail())));
                }
                return Err(e);
            }
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| cdp_err(format!("Bad DevTools message: {e}")))
    }

    fn child_exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
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
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(cdp_err(format!("Timed out waiting for {method}")));
            }
            let Some(reply) = self.read(left.min(Duration::from_millis(250)))? else {
                if self.child_exited() {
                    return Err(cdp_err(format!("Browser exited:\n{}", self.log_tail())));
                }
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
        while !done() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(false);
            }
            if let Some(message) = self.read(left.min(Duration::from_millis(250)))? {
                self.dispatch(&message)?;
            }
        }
        Ok(true)
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
        if let Some(mut t) = self.transport.take() {
            let _ = t.send(&json!({"id": 0, "method": "Browser.close"}).to_string());
            t.close();
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
