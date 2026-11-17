//! The render's manifest read as what an export places (docs/layers.feature LA4a, docs/capcut.feature CC2): each run of
//! frames showing one file, each sound clip, the runs of frames a layer draws, and where a file the manifest names is.
//! Pure: the export (lib.rs) renders and probes around it.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// One file shown over a run of frames, as the manifest's frames say: its first source time and rate, its box and
/// fit at its first frame, its place in the page's order (lower draws first), and whether it moved or faded.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub src: String,
    pub key: String,
    pub start: i64,
    pub end: i64,
    pub at: f64,
    pub last_at: f64,
    pub rate: Option<f64>,
    pub bx: [f64; 4],
    pub fit: String,
    pub opacity: f64,
    pub z: usize,
    pub moved: bool,
    pub faded: bool,
}

/// One sound clip of the manifest: its file, frames, source time, gain, rate and fades.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub src: String,
    pub from: i64,
    pub to: i64,
    pub at: f64,
    pub gain_db: f64,
    pub rate: f64,
    pub fade_in: f64,
    pub fade_out: f64,
}

/// The footage's runs, frame by frame: a run goes on while the same element shows the same file with its source time
/// advancing at one rate; a jump in source time (a cut within one take) starts another.
pub fn runs(manifest: &Value, fps: f64) -> Vec<Run> {
    let mut done: Vec<Run> = Vec::new();
    let mut open: Vec<Run> = Vec::new();
    let frames = manifest["frames"].as_array().cloned().unwrap_or_default();
    for (i, fr) in frames.iter().enumerate() {
        let f = fr["frame"].as_i64().unwrap_or(i as i64);
        let mut next = Vec::new();
        for (z, v) in fr["videos"].as_array().into_iter().flatten().enumerate() {
            let src = v["src"].as_str().unwrap_or_default().to_string();
            let key = format!("{src}|{}|{}|{}", v["loc"], v["use"], v["index"]);
            let at = v["at"].as_f64().unwrap_or(0.0);
            let bx: [f64; 4] = std::array::from_fn(|k| v["box"][k].as_f64().unwrap_or(0.0));
            let opacity = v["opacity"].as_f64().unwrap_or(1.0);
            let continues = |r: &Run| {
                let step = at - r.last_at;
                r.key == key && r.end == f && step > 0.0 && match r.rate {
                    Some(rate) => (step - rate / fps).abs() <= 0.6 / fps,
                    None => step * fps <= 8.0,
                }
            };
            if let Some(k) = open.iter().position(continues) {
                let mut r = open.remove(k);
                r.rate.get_or_insert((at - r.last_at) * fps);
                r.moved |= r.bx.iter().zip(&bx).any(|(a, b)| (a - b).abs() > 1.0);
                r.faded |= (r.opacity - opacity).abs() > 0.02;
                (r.end, r.last_at) = (f + 1, at);
                next.push(r);
            } else {
                next.push(Run { src, key, start: f, end: f + 1, at, last_at: at, rate: None, bx, fit: v["fit"].as_str().unwrap_or("fill").to_string(), opacity, z, moved: false, faded: false });
            }
        }
        done.append(&mut open);
        open = next;
    }
    done.append(&mut open);
    // the rate over the whole run (source times are whole milliseconds, so one step's can be 1% off), to hundredths
    for r in &mut done {
        if r.end - r.start > 1 {
            r.rate = Some(((r.last_at - r.at) * fps / (r.end - r.start - 1) as f64 * 100.0).round() / 100.0);
        }
    }
    done.sort_by_key(|r| (r.start, r.z));
    done
}

/// The manifest's sound clips.
pub fn sounds(manifest: &Value) -> Vec<Sound> {
    let s = &manifest["sounds"]["clips"];
    s.as_array()
        .into_iter()
        .flatten()
        .map(|c| Sound {
            src: c["src"].as_str().unwrap_or_default().to_string(),
            from: c["from"].as_i64().unwrap_or(0),
            to: c["to"].as_i64().unwrap_or(0),
            at: c["at"].as_f64().unwrap_or(0.0),
            gain_db: c["gainDb"].as_f64().unwrap_or(0.0),
            rate: c["rate"].as_f64().unwrap_or(1.0),
            fade_in: c["fadeIn"].as_f64().unwrap_or(0.0),
            fade_out: c["fadeOut"].as_f64().unwrap_or(0.0),
        })
        .filter(|c| c.to > c.from)
        .collect()
}

/// Runs of frames that draw something.
pub fn spans(drawn: &[bool]) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let mut f = 0;
    while f < drawn.len() {
        if !drawn[f] {
            f += 1;
            continue;
        }
        let a = f;
        while f < drawn.len() && drawn[f] {
            f += 1;
        }
        out.push((a as i64, f as i64));
    }
    out
}

/// Whether a box covers the whole canvas.
pub fn full(bx: &[f64; 4], w: f64, h: f64) -> bool {
    bx[0] <= 1.0 && bx[1] <= 1.0 && bx[0] + bx[2] >= w - 1.0 && bx[1] + bx[3] >= h - 1.0
}

/// A file the manifest names, as a path, found as the renderer finds it: a staticFile path (/media/x) in the edit's
/// media/, then the project's media/ and assets/; else absolute; else relative to the composition's folder.
pub fn resolve(dir: &Path, src: &str) -> PathBuf {
    let p = raea_core_scene::media_path(dir, src);
    p.canonicalize().unwrap_or(p)
}
