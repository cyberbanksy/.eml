"""Minimal Chrome DevTools Protocol client using only the standard library."""

from __future__ import annotations

import base64
import contextlib
import json
import os
import shutil
import socket
import struct
import subprocess
import tempfile
import time
from collections.abc import Callable
from typing import Any

Handler = Callable[[dict[str, Any]], None]


class CDPError(RuntimeError):
    pass


class _WebSocket:
    def __init__(self, port: int, path: str, timeout: float) -> None:
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=timeout)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall(
            (
                f"GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n"
                "Upgrade: websocket\r\nConnection: Upgrade\r\n"
                f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
            ).encode()
        )
        response = b""
        while b"\r\n\r\n" not in response:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise CDPError("DevTools closed the connection during handshake")
            response += chunk
        head, _, rest = response.partition(b"\r\n\r\n")
        if not head.startswith(b"HTTP/1.1 101"):
            raise CDPError(f"DevTools handshake failed: {head.splitlines()[0]!r}")
        self._buf = bytearray(rest)
        self._partial = bytearray()

    def _frame(self) -> tuple[int, bool, bytes] | None:
        """Pop one complete frame from the buffer, or None if more bytes are needed."""
        buf = self._buf
        if len(buf) < 2:
            return None
        n, offset = buf[1] & 0x7F, 2
        if n == 126:
            if len(buf) < 4:
                return None
            (n,) = struct.unpack_from(">H", buf, 2)
            offset = 4
        elif n == 127:
            if len(buf) < 10:
                return None
            (n,) = struct.unpack_from(">Q", buf, 2)
            offset = 10
        if len(buf) < offset + n:
            return None
        opcode, fin = buf[0] & 0x0F, bool(buf[0] & 0x80)
        payload = bytes(buf[offset : offset + n])
        del buf[: offset + n]
        return opcode, fin, payload

    def send(self, text: str) -> None:
        payload = text.encode()
        header = bytearray([0x81])  # FIN + text frame
        n = len(payload)
        if n < 126:
            header.append(0x80 | n)
        elif n < 1 << 16:
            header += bytes([0x80 | 126]) + struct.pack(">H", n)
        else:
            header += bytes([0x80 | 127]) + struct.pack(">Q", n)
        mask = os.urandom(4)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        self.sock.sendall(bytes(header) + mask + masked)

    def _send_control(self, opcode: int, payload: bytes = b"") -> None:
        mask = os.urandom(4)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        self.sock.sendall(bytes([0x80 | opcode, 0x80 | len(payload)]) + mask + masked)

    def recv(self) -> str:
        """Next text message. A socket timeout leaves the stream intact for a retry."""
        while True:
            frame = self._frame()
            if frame is None:
                chunk = self.sock.recv(1 << 20)
                if not chunk:
                    raise CDPError("DevTools connection closed")
                self._buf += chunk
                continue
            opcode, fin, payload = frame
            if opcode == 0x9:  # ping
                self._send_control(0xA, payload)
            elif opcode == 0x8:
                raise CDPError("DevTools connection closed")
            elif opcode in (0x0, 0x1, 0x2):
                self._partial += payload
                if fin:
                    message = self._partial.decode()
                    self._partial = bytearray()
                    return message

    def close(self) -> None:
        with contextlib.suppress(OSError):
            self._send_control(0x8)
        self.sock.close()


class Browser:
    """A headless browser process driven over DevTools."""

    def __init__(self, executable: str, *, timeout: float = 30.0) -> None:
        self.timeout = timeout
        self._next_id = 0
        self._handlers: dict[str, Handler] = {}
        self._profile = tempfile.mkdtemp(prefix="doteml-")
        args = [
            executable,
            "--headless=new",
            "--remote-debugging-port=0",
            f"--user-data-dir={self._profile}",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-sync",
            "--disable-default-apps",
            "--mute-audio",
            "--hide-scrollbars",
        ]
        if hasattr(os, "geteuid") and os.geteuid() == 0:
            args.append("--no-sandbox")  # Chrome refuses to run as root otherwise
        args.append("about:blank")
        # A file, not a pipe: Chrome logs a lot and would block once a pipe buffer fills.
        self._log = tempfile.TemporaryFile()  # noqa: SIM115 - closed in close()
        self._proc = subprocess.Popen(
            args, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=self._log
        )
        try:
            port, path = self._wait_for_port()
            self._ws = _WebSocket(port, path, timeout)
        except Exception:
            self.close()
            raise

    def _wait_for_port(self) -> tuple[int, str]:
        marker = os.path.join(self._profile, "DevToolsActivePort")
        deadline = time.monotonic() + self.timeout
        while time.monotonic() < deadline:
            if self._proc.poll() is not None:
                self._log.seek(0)
                err = self._log.read().decode(errors="replace").strip()
                raise CDPError(f"Browser exited on startup:\n{err[-800:]}")
            try:
                with open(marker) as f:
                    lines = f.read().split()
                if len(lines) >= 2:
                    return int(lines[0]), lines[1]
            except (OSError, ValueError):  # Windows locks the file while Chrome writes it
                pass
            time.sleep(0.05)
        raise CDPError("Timed out waiting for the browser to start")

    def send(
        self, method: str, params: dict[str, Any] | None = None, session: str | None = None
    ) -> dict[str, Any]:
        self._next_id += 1
        msg_id = self._next_id
        message: dict[str, Any] = {"id": msg_id, "method": method, "params": params or {}}
        if session:
            message["sessionId"] = session
        self._ws.send(json.dumps(message))
        deadline = time.monotonic() + self.timeout
        while True:
            if time.monotonic() > deadline:
                raise CDPError(f"Timed out waiting for {method}")
            reply = json.loads(self._ws.recv())
            if reply.get("id") == msg_id:
                if "error" in reply:
                    raise CDPError(f"{method}: {reply['error'].get('message')}")
                return reply.get("result", {})
            self._dispatch(reply)

    def notify(self, method: str, params: dict[str, Any], session: str) -> None:
        """Send without waiting for the reply (safe to call from an event handler)."""
        self._next_id += 1
        self._ws.send(
            json.dumps(
                {"id": self._next_id, "method": method, "params": params, "sessionId": session}
            )
        )

    def pump_until(self, done: Callable[[], bool], timeout: float) -> bool:
        """Dispatch events until ``done()`` is true; False on timeout."""
        deadline = time.monotonic() + timeout
        self._ws.sock.settimeout(0.25)
        try:
            while not done():
                if time.monotonic() > deadline:
                    return False
                try:
                    self._dispatch(json.loads(self._ws.recv()))
                except TimeoutError:
                    continue
            return True
        finally:
            self._ws.sock.settimeout(self.timeout)

    def on(self, session: str, handler: Handler) -> None:
        self._handlers[session] = handler

    def off(self, session: str) -> None:
        self._handlers.pop(session, None)

    def _dispatch(self, message: dict[str, Any]) -> None:
        handler = self._handlers.get(message.get("sessionId", ""))
        if handler and "method" in message:
            handler(message)

    def close(self) -> None:
        ws = getattr(self, "_ws", None)
        if ws:
            with contextlib.suppress(OSError):
                ws.send(json.dumps({"id": 0, "method": "Browser.close"}))
            ws.close()
        try:
            self._proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self._proc.kill()
            self._proc.wait()
        self._log.close()
        for _ in range(10):  # Windows may hold profile files briefly after exit
            shutil.rmtree(self._profile, ignore_errors=True)
            if not os.path.exists(self._profile):
                break
            time.sleep(0.1)
