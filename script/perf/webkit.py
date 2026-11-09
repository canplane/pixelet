# A page in Safari, through safaridriver.

import json
import socket
import subprocess
import time
import urllib.error
import urllib.request

from measure import polled


def webkit(url, capabilities, size):
    """The lines of `url` in the Safari that `capabilities` name, its window
    at `size` where it has a window to size."""
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    driver = subprocess.Popen(["safaridriver", "-p", str(port)])

    def call(method, path, body=None):
        request = urllib.request.Request(
            f"http://127.0.0.1:{port}{path}",
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
            method=method,
        )
        try:
            with urllib.request.urlopen(request) as r:
                return json.load(r)["value"]
        except urllib.error.HTTPError as e:
            raise RuntimeError(e.read().decode())

    session = None
    try:
        while session is None:
            try:
                session = call(
                    "POST",
                    "/session",
                    {"capabilities": {"alwaysMatch": capabilities}},
                )["sessionId"]
            except urllib.error.URLError:
                time.sleep(0.2)
        here = f"/session/{session}"
        if size:
            call("POST", f"{here}/window/rect", size)
        call("POST", f"{here}/url", {"url": url})
        yield from polled(
            lambda e: call(
                "POST", f"{here}/execute/sync", {"script": f"return {e}", "args": []}
            )
        )
    finally:
        if session:
            call("DELETE", f"/session/{session}")
        driver.terminate()
        driver.wait()
