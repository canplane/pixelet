# A page in Safari, through safaridriver.

import base64
import json
import socket
import subprocess
import time
import urllib.error
import urllib.request

from measure import CAPTURE_S, polled


class WebKit:
    """`url` in the Safari that `capabilities` name, its window at `size`
    where it has a window to size: what a Safari's reach (measure.py) shares.
    The screen is the session's screenshot; the state is the vendor's."""

    def __init__(self, url, capabilities, size):
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            self.port = s.getsockname()[1]
        self.driver = subprocess.Popen(["safaridriver", "-p", str(self.port)])
        self.here = None
        try:
            session = None
            while session is None:
                try:
                    session = self.call(
                        "POST",
                        "/session",
                        {"capabilities": {"alwaysMatch": capabilities}},
                    )["sessionId"]
                except urllib.error.URLError:
                    time.sleep(0.2)
            self.here = f"/session/{session}"
            if size:
                self.call("POST", f"{self.here}/window/rect", size)
            self.call("POST", f"{self.here}/url", {"url": url})
        except BaseException:
            self.close()
            raise
        self.lines = polled(
            lambda e: self.call(
                "POST", f"{self.here}/execute/sync", {"script": f"return {e}", "args": []}
            )
        )

    def call(self, method, path, body=None, timeout=None):
        """`method` on `path`. safaridriver runs a session's commands one at a
        time, in the order they come, from whichever thread sends them."""
        request = urllib.request.Request(
            f"http://127.0.0.1:{self.port}{path}",
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
            method=method,
        )
        try:
            with urllib.request.urlopen(request, timeout=timeout) as r:
                return json.load(r)["value"]
        except urllib.error.HTTPError as e:
            raise RuntimeError(e.read().decode())

    def shot(self, path):
        png = self.call("GET", f"{self.here}/screenshot", timeout=CAPTURE_S)
        with open(path, "wb") as f:
            f.write(base64.b64decode(png))

    def close(self):
        try:
            if self.here:
                self.call("DELETE", self.here, timeout=CAPTURE_S)
        finally:
            self.driver.terminate()
            self.driver.wait()
