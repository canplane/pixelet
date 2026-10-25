# cpp/amalgamation.awk
#
# The library in two files, as SQLite ships its own: xpute.hpp, every header
# of xpute/ after the headers it includes, and xpute.cpp, which includes it
# and then holds every source. The library's own includes are dropped, since
# what they name is already above; a system one is kept where it first
# appears. The Makefile passes the files, sorted, as paths from cpp/:
#
#   awk -f amalgamation.awk -v part=hpp <headers>             > xpute.hpp
#   awk -f amalgamation.awk -v part=cpp <headers> <sources>   > xpute.cpp
#   awk -f amalgamation.awk -v part=program <headers> <file>  > file
#
# `program` is a program of the library's own written against the two
# files: its includes of the library become one of xpute.hpp, its other
# includes left as they are. A header is found as the compiler finds it
# under -I cpp: beside the file that names it ("ring.hpp"), or from cpp/
# ("xpute/kit/status/bug.hpp"). A quoted include that the library names and
# no file given answers is an error, so a header left out of the list fails
# here and not in a program that includes xpute.hpp.

BEGIN {
    if (part != "hpp" && part != "cpp" && part != "program") fail("part is hpp, cpp or program, not '" part "'")
    last = ARGC - 1
    for (i = 1; i < ARGC; i++) {
        f = ARGV[i]
        if (part == "program" && i == last) program = f
        else library[f] = 1
        n[f] = 0
        while ((got = (getline l < f)) > 0) line[f, ++n[f]] = l
        if (got < 0) fail(f ": cannot be read")
        close(f)
    }
    if (part == "hpp") {
        print "// xpute.hpp"
        print "//"
        print "// GENERATED from cpp/xpute/ by `make -C cpp amalgamation` (cpp/amalgamation.awk) — do not edit."
        print "//"
        print "// xpute's C++ library in one header: every header of cpp/xpute/, each after"
        print "// the ones it includes. Compile xpute.cpp, beside it, once into the program."
        print ""
        print "#pragma once"
        for (i = 1; i < ARGC; i++) emit(ARGV[i])
    } else if (part == "cpp") {
        print "// xpute.cpp"
        print "//"
        print "// GENERATED from cpp/xpute/ by `make -C cpp amalgamation` (cpp/amalgamation.awk) — do not edit."
        print "//"
        print "// Every source of xpute's C++ library, as one translation unit over xpute.hpp."
        print ""
        print "#include \"xpute.hpp\""
        # What the header already includes, a source need not again.
        for (i = 1; i < ARGC; i++) if (ARGV[i] ~ /\.hpp$/) scan(ARGV[i])
        for (i = 1; i < ARGC; i++) if (ARGV[i] !~ /\.hpp$/) body(ARGV[i])
    } else {
        body(program)
    }
    exit 0
}

function fail(message) {
    print "amalgamation: " message > "/dev/stderr"
    exit 1
}

# The quoted path an include line names, or "" for any other line.
function quoted(l) {
    if (l !~ /^[ \t]*#[ \t]*include[ \t]*"/) return ""
    sub(/^[^"]*"/, "", l)
    sub(/".*$/, "", l)
    return l
}

function system_include(l) {
    return l ~ /^[ \t]*#[ \t]*include[ \t]*</
}

# The library file an include in `f` names, or "" when it names none.
function resolve(f, p, d) {
    if (p ~ /\//) return (p in library) ? p : ""
    d = f
    if (!sub(/\/[^\/]*$/, "", d)) d = "."
    return ((d "/" p) in library) ? (d "/" p) : ""
}

# A header, after every header it includes.
function emit(f, k, p, r) {
    if (f in emitted) return
    if (f in open_) fail(f ": includes itself through another header")
    open_[f] = 1
    for (k = 1; k <= n[f]; k++) if ((p = quoted(line[f, k])) != "") {
        if ((r = resolve(f, p)) == "") fail(f ":" k ": \"" p "\" is not a header of the library")
        emit(r)
    }
    delete open_[f]
    emitted[f] = 1
    body(f)
}

# The system includes a header holds outside any conditional.
function scan(f, k, depth, l) {
    depth = 0
    for (k = 1; k <= n[f]; k++) {
        l = line[f, k]
        if (l ~ /^[ \t]*#[ \t]*if/) depth++
        else if (l ~ /^[ \t]*#[ \t]*endif/) depth--
        else if (depth == 0 && system_include(l)) seen[l] = 1
    }
}

# A file's lines, without its includes of the library or its `#pragma once`,
# and without a second blank line where a dropped one leaves two.
function body(f, k, l, p, r, depth, blank) {
    depth = 0
    print ""
    blank = 1
    for (k = 1; k <= n[f]; k++) {
        l = line[f, k]
        if (l ~ /^[ \t]*#[ \t]*if/) depth++
        else if (l ~ /^[ \t]*#[ \t]*endif/) depth--
        if (l ~ /^[ \t]*#[ \t]*pragma[ \t]+once/) continue
        p = quoted(l)
        r = p != "" ? resolve(f, p) : ""
        if (p != "" && f != program && r == "") fail(f ":" k ": \"" p "\" is not a header of the library")
        if (r != "") {
            if (depth != 0) fail(f ":" k ": the library is included under a conditional, which one file cannot keep")
            if (f != program || swapped) continue
            swapped = 1
            l = "#include \"xpute.hpp\""
        } else if (depth == 0 && system_include(l)) {
            if (l in seen) continue
            seen[l] = 1
        }
        if (l == "" && blank) continue
        blank = (l == "")
        print l
    }
}
