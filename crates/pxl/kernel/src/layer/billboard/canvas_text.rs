// pxl-kernel/layer/billboard/canvas_text.rs

//! Text helpers shared by every world-anchored billboard that draws its own
//! content rather than sourcing an image asset — the chat bubble today. The
//! measurement is the host's (a canvas context's `measureText` with its font
//! set), handed in; what is decided here is where the lines break.

/// Greedy word wrap against `measure`, then a per-character fallback pass for
/// any line still too wide on its own — a single long word, or (the common
/// case that broke this the first time around) a Korean message typed with no
/// spacing between words at all, which the word pass alone treats as one
/// unbreakable "word" spanning the whole message.
pub fn wrap_text(measure: &dyn Fn(&str) -> f64, text: &str, max_width: f64) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }

    let mut word_lines: Vec<String> = Vec::new();
    let mut current = words[0].to_string();
    for word in &words[1..] {
        let candidate = format!("{current} {word}");
        if measure(&candidate) <= max_width {
            current = candidate;
        } else {
            word_lines.push(std::mem::replace(&mut current, word.to_string()));
        }
    }
    word_lines.push(current);

    let mut lines = Vec::new();
    for line in word_lines {
        if measure(&line) <= max_width {
            lines.push(line);
            continue;
        }
        let mut chunk = String::new();
        for ch in line.chars() {
            let mut candidate = chunk.clone();
            candidate.push(ch);
            if !chunk.is_empty() && measure(&candidate) > max_width {
                lines.push(std::mem::take(&mut chunk));
                chunk.push(ch);
            } else {
                chunk = candidate;
            }
        }
        if !chunk.is_empty() {
            lines.push(chunk);
        }
    }
    lines
}
