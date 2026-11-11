#!/usr/bin/env python3
# One measurement in the Safari of the iPhone safaridriver reaches.
#
#   ./script/perf/ios-safari.py <url> <dir>

import ios
from measure import measure
from webkit import WebKit


class IosSafari(WebKit):
    """The iPhone's Safari, as safaridriver's session opens it. Whether it
    is already running is not read from here. The state is the iPhone's."""

    @staticmethod
    def running():
        """safaridriver opens its own session on the iPhone, beside whatever
        Safari runs there, and tells nothing of it."""
        return None

    def __init__(self, url):
        super().__init__(url, {"browserName": "safari", "platformName": "iOS"}, None)

    def state(self):
        return ios.state()


# The run holds its device from before it starts (`measure.hold`).
ios.hold()
measure(IosSafari)
