//! docs/layers.feature LA3a: the layers put back together are the render. Each fixture (apps/layers/fixtures) is
//! exported in layers by a built raea, its passes' frames kept; at each clip's first, middle and last frame the layers
//! are composited bottom to top (under, footage as layers.json places it, graphics, captions) and compared with the
//! reference render's frame. A test of the method, run by hand on a machine with a GPU:
//!   cargo test -p raea-app-layers composite -- --ignored --nocapture   (RAEA_BIN: the raea, default target/release/raea;
//!   RAEA_LAYERS_PROJECT=DIR/EDIT.tsx checks that composition, its media/ beside it, in place of the fixtures)

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::{Clip, Layer, Layers};

/// The least PSNR (dB) a composited frame has against the reference render's: the reference is an H.264 movie and the
/// footage is decoded and scaled here as the page does it, so a frame is never exact.
const MIN_PSNR: f64 = 30.0;

/// An RGB picture, 0..1, row by row.
struct Picture {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
}

impl Picture {
    fn black(w: usize, h: usize) -> Picture {
        Picture { w, h, px: vec![[0.0; 3]; w * h] }
    }

    /// `rgba` (straight alpha, 8 bits) laid over this at full size, times `opacity`.
    fn over(&mut self, rgba: &[u8], opacity: f32) {
        for (k, p) in self.px.iter_mut().enumerate() {
            let a = rgba[k * 4 + 3] as f32 / 255.0 * opacity;
            for c in 0..3 {
                p[c] = p[c] * (1.0 - a) + rgba[k * 4 + c] as f32 / 255.0 * a;
            }
        }
    }

    /// A picture of `mw` x `mh` (RGBA) drawn into `bx` as CSS's object-fit `fit` draws it (centred, clipped to its
    /// box), sampled bilinearly, times `opacity`.
    fn draw(&mut self, rgba: &[u8], mw: usize, mh: usize, bx: [f64; 4], fit: &str, opacity: f32) {
        let [bx0, by0, bw, bh] = bx;
        let (sx, sy) = (bw / mw as f64, bh / mh as f64);
        let (sx, sy) = match fit {
            "cover" => (sx.max(sy), sx.max(sy)),
            "contain" => (sx.min(sy), sx.min(sy)),
            "none" => (1.0, 1.0),
            "scale-down" => (sx.min(sy).min(1.0), sx.min(sy).min(1.0)),
            _ => (sx, sy),
        };
        let (ox, oy) = (bx0 + (bw - mw as f64 * sx) / 2.0, by0 + (bh - mh as f64 * sy) / 2.0);
        let (x0, y0) = (bx0.max(0.0) as usize, by0.max(0.0) as usize);
        let (x1, y1) = (((bx0 + bw).ceil() as usize).min(self.w), ((by0 + bh).ceil() as usize).min(self.h));
        let at = |x: usize, y: usize, c: usize| rgba[(y.min(mh - 1) * mw + x.min(mw - 1)) * 4 + c] as f32 / 255.0;
        for y in y0..y1 {
            for x in x0..x1 {
                let (u, v) = ((x as f64 + 0.5 - ox) / sx - 0.5, (y as f64 + 0.5 - oy) / sy - 0.5);
                if u < -0.5 || v < -0.5 || u > mw as f64 - 0.5 || v > mh as f64 - 0.5 {
                    continue;
                }
                let (u, v) = (u.max(0.0), v.max(0.0));
                let (iu, iv, fu, fv) = (u as usize, v as usize, (u.fract()) as f32, (v.fract()) as f32);
                let p = &mut self.px[y * self.w + x];
                for c in 0..3 {
                    let s = at(iu, iv, c) * (1.0 - fu) * (1.0 - fv) + at(iu + 1, iv, c) * fu * (1.0 - fv) + at(iu, iv + 1, c) * (1.0 - fu) * fv + at(iu + 1, iv + 1, c) * fu * fv;
                    p[c] = p[c] * (1.0 - opacity) + s * opacity;
                }
            }
        }
    }

    /// Peak signal-to-noise ratio against `rgba`, in dB (infinite when they're the same).
    fn psnr(&self, rgba: &[u8]) -> f64 {
        let mut se = 0.0f64;
        for (k, p) in self.px.iter().enumerate() {
            for c in 0..3 {
                let d = p[c] as f64 - rgba[k * 4 + c] as f64 / 255.0;
                se += d * d;
            }
        }
        let mse = se / (self.px.len() * 3) as f64;
        if mse == 0.0 { f64::INFINITY } else { 10.0 * (1.0 / mse).log10() }
    }
}

/// A movie's frame at `secs`, as RGBA, with its size.
fn frame(path: &Path, secs: f64) -> Result<(Vec<u8>, usize, usize), String> {
    let mut d = raea_io_media::preferred().into_iter().find_map(|b| b.open_video(path).ok()).ok_or_else(|| format!("{}: no backend opens it", path.display()))?;
    let f = d.frame_at(secs).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((f.rgba.to_vec(), f.width as usize, f.height as usize))
}

/// Frame `f` of the export in `dir` composited from its layers, bottom to top.
fn composite(dir: &Path, l: &Layers, f: i64) -> Result<Picture, String> {
    let mut pic = Picture::black(l.width as usize, l.height as usize);
    let on = |c: &&Clip| c.start <= f && f < c.end();
    for t in l.tracks.iter().rev() {
        for c in t.clips.iter().filter(on) {
            match t.layer {
                Layer::Sound => {}
                Layer::Footage => {
                    let file = &l.files[c.file];
                    let secs = c.source_in + (f - c.start) as f64 / l.fps * c.speed + 0.25 / l.fps;
                    let (rgba, w, h) = frame(Path::new(&file.path), secs)?;
                    pic.draw(&rgba, w, h, c.r#box.unwrap(), c.fit.as_deref().unwrap_or("fill"), c.opacity as f32);
                }
                _ => {
                    // a layer's movie is NAME.CODEC.mov, its frames NAME.frames
                    let name = l.files[c.file].path.split('.').next().unwrap_or_default().to_string();
                    let png = dir.join(format!("{name}.frames/frame-{f:05}.png"));
                    let (rgba, _, _) = raea_io_alpha::read_png(&png)?;
                    pic.over(&rgba, 1.0);
                }
            }
        }
    }
    Ok(pic)
}

/// Each fixture exported by `bin` into its own folder under `work`: its layers.json and the folder.
fn export(bin: &Path, fixture: &Path, work: &Path) -> Result<(Layers, PathBuf), String> {
    let name = fixture.file_stem().unwrap().to_string_lossy().into_owned();
    let src = work.join(&name);
    let _ = std::fs::remove_dir_all(&src);
    std::fs::create_dir_all(src.join("media")).map_err(|e| e.to_string())?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // a project of its own is exported where it is; a fixture is copied with the test clip beside it
    let (src, edit) = if fixture.parent() == Some(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")) {
        std::fs::copy(fixture, src.join("edit.tsx")).map_err(|e| e.to_string())?;
        std::fs::copy(root.join("engine/rune/media/tests/fixtures/clip.mp4"), src.join("media/clip.mp4")).map_err(|e| e.to_string())?;
        (src, "edit.tsx".to_string())
    } else {
        (fixture.parent().unwrap().to_path_buf(), fixture.file_name().unwrap().to_string_lossy().into_owned())
    };
    let out = work.join(&name).join("layers");
    let run = Command::new(bin)
        .args(["export", "layers", edit.as_str(), "-o"])
        .arg(&out)
        .current_dir(&src)
        .env("RAEA_LAYERS_KEEP_FRAMES", "1")
        .env("RAEA_EXPORT_CACHE", "0")
        .output()
        .map_err(|e| format!("{}: {e}", bin.display()))?;
    if !run.status.success() {
        return Err(format!("{name}: {}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr)));
    }
    println!("{name}: {}", String::from_utf8_lossy(&run.stdout).trim());
    let l: Layers = serde_json::from_str(&std::fs::read_to_string(out.join("layers.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok((l, out))
}

#[test]
#[ignore]
fn composite_the_layers_back_and_they_are_the_render() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bin = std::env::var_os("RAEA_BIN").map_or_else(|| root.join("target/release/raea"), PathBuf::from);
    let work = std::env::temp_dir().join(format!("raea-layers-la3a-{}", std::process::id()));
    let mut fixtures: Vec<PathBuf> = match std::env::var_os("RAEA_LAYERS_PROJECT") {
        Some(p) => vec![PathBuf::from(p)],
        None => std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "tsx")).collect(),
    };
    fixtures.sort();
    let mut failed = Vec::new();
    for fx in &fixtures {
        let name = fx.file_stem().unwrap().to_string_lossy().into_owned();
        let (l, dir) = match export(&bin, fx, &work) {
            Ok(x) => x,
            Err(e) => {
                failed.push(e);
                continue;
            }
        };
        // each clip's first, middle and last frame
        let mut at: Vec<i64> = l.tracks.iter().filter(|t| t.layer != Layer::Sound).flat_map(|t| t.clips.iter()).flat_map(|c| [c.start, c.start + c.frames / 2, c.end() - 1]).filter(|f| (0..l.frames).contains(f)).collect();
        at.sort();
        at.dedup();
        for f in at {
            let got = composite(&dir, &l, f).and_then(|pic| frame(&dir.join(&l.reference), (f as f64 + 0.25) / l.fps).map(|r| (pic, r)));
            match got {
                Ok((pic, (reference, w, h))) if (w, h) == (pic.w, pic.h) => {
                    let db = pic.psnr(&reference);
                    println!("  {name} frame {f}: {db:.1} dB");
                    if db < MIN_PSNR {
                        failed.push(format!("{name} frame {f}: the layers composited are {db:.1} dB from the reference (at least {MIN_PSNR})"));
                    }
                }
                Ok((pic, (_, w, h))) => failed.push(format!("{name} frame {f}: the reference is {w}x{h}, the layers {}x{}", pic.w, pic.h)),
                Err(e) => failed.push(format!("{name} frame {f}: {e}")),
            }
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
