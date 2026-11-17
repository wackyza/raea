//! Where a picture sits in CapCut's terms (docs/capcut.feature CC2a). The manifest is read by raea-app-layers, the same
//! for every editor; this is CapCut's own.

use raea_app_layers::plan::full;
use raea_io_capcut::Place;

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
