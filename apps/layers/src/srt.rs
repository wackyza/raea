//! Captions as SubRip (docs/layers.feature LA2b): each run of frames a caption shows one line of words, from the
//! render manifest's text marked `caption` (engine/page manifest.rs).

use serde_json::Value;

/// One SubRip entry: its words, its first frame and the frame after its last.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub from: i64,
    pub to: i64,
}

/// The words the captions show on each frame, joined in the page's order; empty where none shows.
pub fn said(manifest: &Value) -> Vec<String> {
    manifest["frames"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            let words: Vec<String> = f["text"].as_array().into_iter().flatten().filter(|t| t["caption"] == true && t["opacity"].as_f64().unwrap_or(1.0) > 0.01).filter_map(|t| t["text"].as_str()).map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty()).collect();
            words.join(" ")
        })
        .collect()
}

/// The lines: a run of frames showing the same words is one, and so is a line whose words come in one by one (each
/// frame's words starting with the last's) or go one by one, shown whole for the run. A word's colour changing (a
/// highlight as it's said) isn't in the words, so it never starts a line.
pub fn lines(said: &[String]) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for (f, words) in said.iter().enumerate() {
        let f = f as i64;
        if words.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(l) if l.to == f && (words.starts_with(l.text.as_str()) || l.text.starts_with(words.as_str())) => {
                if words.len() > l.text.len() {
                    l.text = words.clone();
                }
                l.to = f + 1;
            }
            _ => out.push(Line { text: words.clone(), from: f, to: f + 1 }),
        }
    }
    out
}

/// SubRip's text: each line numbered from 1, its times as HH:MM:SS,mmm at `fps`.
pub fn write(lines: &[Line], fps: f64) -> String {
    let t = |f: i64| {
        let ms = (f as f64 * 1000.0 / fps).round() as i64;
        format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
    };
    lines.iter().enumerate().map(|(k, l)| format!("{}\n{} --> {}\n{}\n", k + 1, t(l.from), t(l.to), l.text)).collect::<Vec<_>>().join("\n")
}
