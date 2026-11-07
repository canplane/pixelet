# prof.py <out dir> <tag> <url> : one fresh page as script/compare-perf.py loads it, its drive and its report.
import importlib.util, os, sys, time
spec = importlib.util.spec_from_file_location("compare", "/Users/sanghoon/git/canplane/pixelet/script/compare-perf.py")
c = importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
out, tag, url = sys.argv[1:4]
os.makedirs(out, exist_ok=True)
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
    page.ev(f"pxl.perf.drive({c.DRIVE_S})")
    open(os.path.join(out, tag + "-report.txt"), "w").write(str(page.ev("pxl.perf.report()")))
    print(tag, "reported", flush=True)
finally:
    page.close()
