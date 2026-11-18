# rest.py <out dir> <tag> <url> : a fresh page, 8 s of the drive, then 65 s untouched: the window read holds only standing frames; shots during the drive and at rest.
import base64, importlib.util, os, sys, time
spec = importlib.util.spec_from_file_location("compare", "~/git/canplane/pixelet/script/compare-perf.py")
c = importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
out, tag, url = sys.argv[1:4]
os.makedirs(out, exist_ok=True)
def shot(page, name):
    r = page.send("Page.captureScreenshot", {"format": "jpeg", "quality": 70})
    open(os.path.join(out, f"{tag}-{name}.jpg"), "wb").write(base64.b64decode(r["data"]))
page = c.Page(url)
try:
    last, quiet, t = 0, 0, 0
    while quiet < c.QUIET_S and t < c.LOAD_CAP_S:
        time.sleep(1); t += 1
        try:
            end = page.ev(c.LAST_END) or 0
        except RuntimeError:
            end = 0
        quiet = 0 if end > last else quiet + 1
        last = max(last, end)
    shot(page, "load")
    page.ev("window.__d = pxl.perf.drive(8); 1")
    for k in (3, 6):
        time.sleep(3); shot(page, f"drive{k}")
    page.ev("window.__d")
    shot(page, "stop")
    time.sleep(65)
    open(os.path.join(out, tag + "-rest.txt"), "w").write(str(page.ev("pxl.perf.report()")))
    shot(page, "rest")
    print(tag, "done", flush=True)
finally:
    page.close()
