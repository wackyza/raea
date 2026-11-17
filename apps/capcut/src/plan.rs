//! The render's manifest as a CapCut timeline (docs/capcut.feature CC2): each run of frames showing one file as a
//! clip of it, each sound clip as an audio clip (or as its footage's own sound), each run of frames the overlay draws
//! as a clip of the overlay movie, and the tracks they go on. Pure: the export (lib.rs) renders and probes around it.
//! What reads the manifest is raea-app-layers' (every editor's export reads it the same way); where a clip sits and
//! the tracks it goes on are CapCut's.

use raea_io_capcut::{Clip, Kind, Place, Track};

pub use raea_app_layers::plan::{Run, Sound, full, resolve, runs, sounds, spans};

/// Where a picture of `mw` x `mh` sits to show as it does in its box with its object-fit, in CapCut's terms (scale 1:
/// fitted inside the canvas; a crop is fitted as the part it keeps). A warning when CapCut can't show it so.
pub fn place(bx: &[f64; 4], fit: &str, mw: f64, mh: f64, w: f64, h: f64, opacity: f64) -> (Place, Option<String>) {
    let (bw, bh) = (bx[2].max(1.0), bx[3].max(1.0));
    let (x, y) = (bx[0] + bw / 2.0, bx[1] + bh / 2.0);
    let fit0 = (w / mw).min(h / mh);
    let mut warn = None;
    let contain = (bw / mw).min(bh / mh);
    let (scale, crop) = match fit {
        "cover" if full(bx, w, h) => ((bw / mw).max(bh / mh) / fit0, [0.0, 0.0, 1.0, 1.0]),
        "cover" => {
            let s = (bw / mw).max(bh / mh);
            let (cw, ch) = (bw / (mw * s), bh / (mh * s));
            let fitc = (w / (mw * cw)).min(h / (mh * ch));
            (bw / (mw * cw * fitc), [(1.0 - cw) / 2.0, (1.0 - ch) / 2.0, (1.0 + cw) / 2.0, (1.0 + ch) / 2.0])
        }
        "none" => (1.0 / fit0, [0.0, 0.0, 1.0, 1.0]),
        "scale-down" => (contain.min(1.0) / fit0, [0.0, 0.0, 1.0, 1.0]),
        "contain" => (contain / fit0, [0.0, 0.0, 1.0, 1.0]),
        _ => {
            if ((bw / bh) - (mw / mh)).abs() > 0.01 * (mw / mh) {
                warn = Some(format!("stretched to its {bw:.0}x{bh:.0} box in raea (object-fit: fill), fitted inside it in CapCut"));
            }
            (contain / fit0, [0.0, 0.0, 1.0, 1.0])
        }
    };
    (Place { x, y, scale, opacity, crop }, warn)
}

/// Clips laid on as few tracks as hold them without overlap, first come first: `kind` tracks named `name`, `name 2`...
pub fn lay(kind: Kind, name: &str, clips: Vec<Clip>) -> Vec<Track> {
    let mut tracks: Vec<Track> = Vec::new();
    for c in clips {
        let end = |t: &Track| t.clips.last().map_or(i64::MIN, |l| l.start + l.frames);
        match tracks.iter_mut().find(|t| end(t) <= c.start) {
            Some(t) => t.clips.push(c),
            None => {
                let n = tracks.len();
                tracks.push(Track { kind, name: if n == 0 || name.is_empty() { name.to_string() } else { format!("{name} {}", n + 1) }, clips: vec![c] });
            }
        }
    }
    tracks
}
