// pxl-render/build/cut.rs

//! A part's text as it goes out, and a module cut to what its entry points reach. Each reads
//! WGSL line by line as `strip_comments` leaves it.

/// `text` without its line comments, its indentation and its blank lines. WGSL has no string
/// literals, so `//` always starts a comment. Lines are never joined, so two tokens cannot run
/// together. Block comments are kept.
pub(crate) fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let code = match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        };
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        out.push_str(code);
        out.push('\n');
    }
    out
}

/// `text` with only the functions its entry points reach. `text` is read as `strip_comments`
/// leaves it, one statement a line. A function runs from its `fn` line to the line where its
/// braces close. An entry point is a function whose attributes stand on its own line or on the
/// line before.
pub(crate) fn read_fns(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let lines: Vec<&str> = text.lines().collect();
    // Each function's name, with its first and last line.
    let mut fns = std::collections::BTreeMap::<&str, (usize, usize)>::new();
    let mut entries = Vec::new();
    let mut k = 0;
    while k < lines.len() {
        // Where `fn` begins on a function's first line, after its attributes when they stand
        // on that line.
        let at = if lines[k].starts_with("fn ") {
            Some(0)
        } else if lines[k].starts_with('@') {
            lines[k].find(" fn ").map(|p| p + 1)
        } else {
            None
        };
        let Some(rest) = at.map(|p| &lines[k][p + 3..]) else {
            k += 1;
            continue;
        };
        let name = &rest[..rest.find(|c: char| !is_word(c)).unwrap_or(rest.len())];
        let (mut depth, mut end, mut opened) = (0i32, k, false);
        for (j, l) in lines.iter().enumerate().skip(k) {
            for c in l.chars() {
                match c {
                    '{' => (depth, opened) = (depth + 1, true),
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if opened && depth == 0 {
                end = j;
                break;
            }
        }
        let attrs = lines[k].starts_with('@') || (k > 0 && lines[k - 1].starts_with('@'));
        let entry = [&lines[k], if k > 0 { &lines[k - 1] } else { &"" }]
            .iter()
            .any(|l| l.contains("@compute") || l.contains("@vertex") || l.contains("@fragment"));
        if attrs && entry {
            entries.push(name);
        }
        fns.insert(name, (k, end));
        k = end + 1;
    }
    let mut reached = std::collections::BTreeSet::new();
    while let Some(f) = entries.pop() {
        if !reached.insert(f) {
            continue;
        }
        let (first, last) = fns[f];
        for l in &lines[first..=last] {
            for w in l.split(|c: char| !is_word(c)) {
                if fns.contains_key(w) && !reached.contains(w) {
                    entries.push(w);
                }
            }
        }
    }
    let mut out = Vec::with_capacity(lines.len());
    let mut k = 0;
    while k < lines.len() {
        let dropped = fns
            .iter()
            .find(|(name, (first, _))| *first == k && !reached.contains(*name));
        match dropped {
            Some((_, &(_, last))) => k = last + 1,
            None => {
                out.push(lines[k]);
                k += 1;
            }
        }
    }
    out.join("\n") + "\n"
}

/// `text` with only the structs something in it names. `text` is read as `strip_comments`
/// leaves it: a struct runs from its `struct` line to the line where its brace closes.
pub(crate) fn read_structs(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut lines: Vec<&str> = text.lines().collect();
    loop {
        // Each struct's name, with its first and last line.
        let mut structs = Vec::new();
        let mut k = 0;
        while k < lines.len() {
            let Some(rest) = lines[k].strip_prefix("struct ") else {
                k += 1;
                continue;
            };
            let name = &rest[..rest.find(|c: char| !is_word(c)).unwrap_or(rest.len())];
            // The brace closes on the struct's first line or at the start of a later line.
            let end = (k..lines.len())
                .find(|&j| j == k && lines[j].contains('}') || lines[j].starts_with('}'))
                .expect("a struct's close");
            structs.push((name, k, end));
            k = end + 1;
        }
        let unread = structs.iter().find(|(name, first, last)| {
            !lines.iter().enumerate().any(|(j, l)| {
                (j < *first || j > *last) && l.split(|c: char| !is_word(c)).any(|w| w == *name)
            })
        });
        let Some(&(_, first, last)) = unread else {
            return lines.join("\n") + "\n";
        };
        lines.drain(first..=last);
    }
}

/// `text` with only the module's constants something in it reads.
pub(crate) fn read_consts(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let name_of = |line: &str| {
        let rest = line.strip_prefix("const ")?;
        rest.find(|c: char| !is_word(c))
            .map(|end| rest[..end].to_string())
    };
    let mut lines: Vec<&str> = text.lines().collect();
    loop {
        let mut names = std::collections::BTreeMap::<&str, u32>::new();
        for w in lines.iter().flat_map(|l| l.split(|c: char| !is_word(c))) {
            *names.entry(w).or_default() += 1;
        }
        let (mut read, mut unread) = (Vec::with_capacity(lines.len()), false);
        let mut through = false;
        for l in &lines {
            if through {
                through = !l.contains(';');
            } else if name_of(l).is_some_and(|name| names[name.as_str()] == 1) {
                through = !l.contains(';');
                unread = true;
            } else {
                read.push(*l);
            }
        }
        lines = read;
        if !unread {
            return lines.join("\n") + "\n";
        }
    }
}
