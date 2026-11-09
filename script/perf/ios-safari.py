#!/usr/bin/env python3
# One measurement in the Safari of the iPhone safaridriver reaches.
#
#   ./script/perf/ios-safari.py <url> <dir>

from measure import measure
from webkit import webkit


def ios_safari(url):
    yield from webkit(url, {"browserName": "safari", "platformName": "iOS"}, None)


measure(ios_safari)
