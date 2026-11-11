# Chrome's debugging protocol, over whatever carries its messages.

import base64
import json
import threading


class Cdp:
    """Commands and their answers, over a `put` and a `get` of one message.
    The capture's thread sends beside `lines` (measure.py), so one thread at
    a time reads, and keeps an answer that is another's for it: a command
    that is never answered holds up no other. The screen is the page's
    screenshot. A script has one."""

    id = 0
    session = None
    cv = threading.Condition()
    reading = False
    answers = {}

    def send(self, method, params=None):
        """`method`'s answer; an error raises."""
        with self.cv:
            self.id += 1
            mine = self.id
            msg = {"id": mine, "method": method, "params": params or {}}
            if self.session:
                msg["sessionId"] = self.session
            self.put(json.dumps(msg).encode())
        while True:
            with self.cv:
                while mine not in self.answers and self.reading:
                    self.cv.wait()
                if mine in self.answers:
                    m = self.answers.pop(mine)
                    break
                self.reading = True
            m = None
            try:
                m = self.get()
            finally:
                with self.cv:
                    self.reading = False
                    if m is not None and "id" in m:
                        self.answers[m["id"]] = m
                    self.cv.notify_all()
        if "error" in m:
            raise RuntimeError(f"{method}: {m['error'].get('message')}")
        return m["result"]

    def ev(self, expression):
        r = self.send(
            "Runtime.evaluate",
            {"expression": expression, "awaitPromise": True, "returnByValue": True},
        )
        if "exceptionDetails" in r:
            raise RuntimeError(r["exceptionDetails"]["text"])
        return r["result"].get("value")

    def shot(self, path):
        png = self.send("Page.captureScreenshot", {"format": "png"})["data"]
        with open(path, "wb") as f:
            f.write(base64.b64decode(png))
