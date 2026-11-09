# Chrome's debugging protocol, over whatever carries its messages.

import json


class Cdp:
    """Commands and their answers, over a `put` and a `get` of one message."""

    id = 0
    session = None

    def send(self, method, params=None):
        """`method`'s answer; an error raises."""
        self.id += 1
        msg = {"id": self.id, "method": method, "params": params or {}}
        if self.session:
            msg["sessionId"] = self.session
        self.put(json.dumps(msg).encode())
        while True:
            m = self.get()
            if m.get("id") == self.id:
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
