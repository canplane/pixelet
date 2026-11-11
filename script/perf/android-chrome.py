#!/usr/bin/env python3
# One measurement in the Chrome of the phone adb reaches, started afresh, its
# site's data cleared and the cache off.
#
#   ./script/perf/android-chrome.py <url> <dir>

import json
import os
import socket
import time
import urllib.parse
import urllib.request

import android
from android import adb
from cdp import Cdp
from measure import CAPTURE_S, measure, polled

PACKAGE = "com.android.chrome"
CHROME = f"{PACKAGE}/com.google.android.apps.chrome.Main"


class Tab(Cdp):
    """A tab of the phone's Chrome on `url`, started afresh and stopped as the
    run ends, spoken to over a WebSocket through adb's forward: a reach
    (measure.py). A URL on this machine reaches it through adb's reverse. The
    state is the phone's."""

    reverse = port = tab = sock = None

    @staticmethod
    def running():
        """The run stops the phone's Chrome before it starts it, as Android
        keeps a Chrome the user closed cached, so nothing running is shared."""
        return None

    def __init__(self, url):
        try:
            self.open(url)
        except BaseException:
            self.close()
            raise
        self.lines = polled(self.ev)

    def tabs(self, url):
        """The ids of the phone's pages on `url`."""
        with urllib.request.urlopen(f"{self.base}/list") as r:
            return [
                t["id"]
                for t in json.load(r)
                if t["type"] == "page" and t["url"].startswith(url)
            ]

    def open(self, url):
        parts = urllib.parse.urlsplit(url)
        if parts.hostname in ("localhost", "127.0.0.1"):
            self.reverse = f"tcp:{parts.port}"
            adb("reverse", self.reverse, self.reverse)
        adb("shell", "am", "force-stop", PACKAGE)
        adb("shell", "am", "start", "-n", CHROME)
        self.port = adb("forward", "tcp:0", "localabstract:chrome_devtools_remote").strip()
        self.base = f"http://127.0.0.1:{self.port}/json"
        while True:
            try:
                urllib.request.urlopen(f"{self.base}/version").close()
                break
            except OSError:
                time.sleep(0.2)
        for tab in self.tabs(url):
            urllib.request.urlopen(f"{self.base}/close/{tab}").close()
        adb("shell", "am", "start", "-a", "android.intent.action.VIEW", "-n", CHROME, "-d", url)
        while self.tab is None:
            found = self.tabs(url)
            if found:
                self.tab = found[0]
            else:
                time.sleep(0.2)
        urllib.request.urlopen(f"{self.base}/activate/{self.tab}").close()
        self.sock = socket.create_connection(("127.0.0.1", int(self.port)))
        self.sock.sendall(
            (
                f"GET /devtools/page/{self.tab} HTTP/1.1\r\n"
                "Host: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                "Sec-WebSocket-Key: cGl4ZWxldC1wZXJmLWtleQ==\r\n"
                "Sec-WebSocket-Version: 13\r\n\r\n"
            ).encode()
        )
        self.reader = self.sock.makefile("rb")
        while self.reader.readline() not in (b"\r\n", b""):
            pass
        self.send("Network.enable")
        self.send("Network.setCacheDisabled", {"cacheDisabled": True})
        self.send(
            "Storage.clearDataForOrigin",
            {"origin": f"{parts.scheme}://{parts.netloc}", "storageTypes": "all"},
        )
        self.send("Page.reload", {"ignoreCache": True})

    def put(self, message):
        n = len(message)
        length = bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + n.to_bytes(2, "big")
        mask = os.urandom(4)
        body = bytes(b ^ mask[i % 4] for i, b in enumerate(message))
        self.sock.sendall(b"\x81" + length + mask + body)

    def get(self):
        message = b""
        while True:
            head = self.reader.read(2)
            if len(head) < 2:
                raise RuntimeError("Chrome closed")
            n = head[1] & 0x7F
            if n >= 126:
                n = int.from_bytes(self.reader.read(2 if n == 126 else 8), "big")
            body = self.reader.read(n)
            if head[0] & 0x0F in (0, 1):
                message += body
                if head[0] & 0x80:
                    return json.loads(message)

    def state(self):
        return android.state()

    def close(self):
        """Ends each thing the tab opened, though one before fails."""
        try:
            if self.sock:
                self.reader.close()
                self.sock.close()
            if self.tab:
                urllib.request.urlopen(
                    f"{self.base}/close/{self.tab}", timeout=CAPTURE_S
                ).close()
        finally:
            try:
                if self.port:
                    adb("forward", "--remove", f"tcp:{self.port}")
                if self.reverse:
                    adb("reverse", "--remove", self.reverse)
            finally:
                adb("shell", "am", "force-stop", PACKAGE)


# The run holds its device from before it starts (`measure.hold`).
android.hold()
measure(Tab)
