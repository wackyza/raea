//! `raea export layers EDIT.tsx` (docs/layers.feature LA4a): a composition as its layers and layers.json, the timeline
//! that places them, which every editor's export is written from. Its renders, each by this same program:
//!   1. as it is, with its manifest: the reference render, and what shows when (each file's runs, each sound clip, the
//!      words a caption says);
//!   2. RAEA_LAYER=graphics: what the page draws over the footage, less the captions, transparent;
//!   3. RAEA_LAYER=under, when there is footage or 3D: what's drawn under the page's footage and graphics (the
//!      backgrounds holding footage, and the 3D), opaque, kept unless it's black;
//!   4. RAEA_LAYER=captions, when a frame shows a caption: the captions alone, and the words as SubRip;
//!   5. RAEA_LAYER=behind:FILE, for each file shown as a picture in picture: what's drawn behind it (its card), kept
//!      only where it's a picture in picture, on a track between it and the footage under it.
//! The footage and sounds are clips of their own files where they are; each layer is clips of its movie, cut where it
//! draws something. A render that isn't an export draws once, as it always has (LA2o).

pub mod model;
pub mod plan;
pub mod srt;
pub mod xml;
#[cfg(test)]
mod composite;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use model::{Clip, File, FileKind, Layer, Layers, Track};
use raea_runtime_bundle::flag;
use serde_json::Value;

pub type R<T> = Result<T, String>;

pub const USAGE: &str = "raea export layers EDIT.tsx [--composition ID] [-o DIR] [--name NAME] [--alpha-codec prores|hevc|rle|png]   (the composition as its layers and the timeline that places them, layers.json: the captions and the graphics as transparent movies, what's under the footage (and any 3D) as an opaque one, the footage and sounds as clips of their own files where they are, the captions also as SubRip, and the reference render; into out/layers/NAME, or DIR)";

pub const USAGE_XML: &str = "raea export xml EDIT.tsx [--composition ID] [-o DIR] [--name NAME] [--alpha-codec prores|hevc|rle|png]   (the composition as an FCP7 XML project for Premiere Pro (File, Import) and DaVinci Resolve (File, Import, Timeline): its layers as raea export layers writes them, NAME.xml beside them placing them on a timeline, the footage and sounds as clips of their own files where they are, each clip's own sound linked to it; into out/xml/NAME, or DIR)";

/// `export layers ...` and `export xml ...`; None for anything else.
pub fn cli(args: &[String]) -> Option<R<String>> {
    if args.first().map(String::as_str) != Some("export") {
        return None;
    }
    match args.get(1).map(String::as_str) {
        Some("layers") => Some(export(&args[2..]).map(|(_, said)| said)),
        Some("xml") => Some(export_xml(&args[2..])),
        _ => None,
    }
}

/// `raea export xml`: the layers into DIR (out/xml/NAME unless -o says), then NAME.xml beside them (LA5a).
fn export_xml(args: &[String]) -> R<String> {
    let tsx = args.first().filter(|a| !a.starts_with('-')).ok_or(USAGE_XML)?;
    let edit = PathBuf::from(tsx).canonicalize().map_err(|e| format!("{tsx}: {e}"))?;
    let name = flag(args, "--name").unwrap_or_else(|| edit.file_stem().unwrap().to_string_lossy().into_owned());
    let dir = flag(args, "-o").map_or_else(|| edit.parent().unwrap().join("out/xml").join(&name), PathBuf::from);
    let mut layer_args = args.to_vec();
    if flag(args, "-o").is_none() {
        layer_args.extend(["-o".into(), dir.display().to_string()]);
    }
    let (layers, _) = export(&layer_args)?;
    let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
    let (text, notes) = xml::write(&layers, &dir);
    let out = dir.join(format!("{name}.xml"));
    std::fs::write(&out, text).map_err(|e| format!("{}: {e}", out.display()))?;
    let count = |l: Layer| layers.tracks.iter().filter(|t| t.layer == l).count();
    let mut said = format!(
        "APPLIED  FCP7 XML \"{name}\" at {}: {} video tracks, {} audio tracks; import it in Premiere Pro (File, Import) or DaVinci Resolve (File, Import, Timeline); the layers, layers.json{} and the reference render beside it",
        out.display(),
        layers.tracks.len() - count(Layer::Sound),
        count(Layer::Sound),
        layers.srt.as_ref().map_or(String::new(), |s| format!(", {s} (captions to upload)")),
    );
    for n in layers.notes.iter().chain(&notes) {
        said.push_str(&format!("\n  note  {n}"));
    }
    Ok(said)
}

/// `raea render ARGS` by this same program with only `env`'s layer settings: none of the person's own RAEA_LAYER or
/// RAEA_HIDE_MEDIA reaches a render it didn't ask for. Its failure is the export's.
pub fn render(args: &[String], env: &[(&str, &str)]) -> R<()> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut c = Command::new(exe);
    c.arg("render").args(args).env_remove("RAEA_LAYER").env_remove("RAEA_HIDE_MEDIA");
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        return Err(format!("the render failed:\n{}", said.trim()));
    }
    Ok(())
}

/// A layer's movie, rendered with RAEA_LAYER=`layer` into DIR/NAME.CODEC.mov (NAME the layer's, or `name` for one of
/// many), its frames in DIR/NAME.frames, removed unless `keep` (LA3a's test reads them).
fn pass(tsx: &Path, comp: &[String], dir: &Path, layer: &str, codec: raea_io_alpha::Codec, fps: f64, keep: bool, also: impl FnOnce(&Path) -> bool) -> R<Option<(PathBuf, raea_io_alpha::Made)>> {
    pass_named(tsx, comp, dir, layer, layer, codec, fps, keep, also)
}

#[allow(clippy::too_many_arguments)]
fn pass_named(tsx: &Path, comp: &[String], dir: &Path, name: &str, layer: &str, codec: raea_io_alpha::Codec, fps: f64, keep: bool, also: impl FnOnce(&Path) -> bool) -> R<Option<(PathBuf, raea_io_alpha::Made)>> {
    // the layer under the page is the bottom one, so it's drawn opaque: over 3D the page's background is the 3D's
    // clear, which --alpha would clear to nothing
    let alpha: &[String] = if layer == "under" { &[] } else { &["--alpha".to_string()] };
    let frames = dir.join(format!("{name}.frames"));
    let _ = std::fs::remove_dir_all(&frames);
    let out = dir.join(format!("{name}.{}.mov", codec_ext(codec)));
    let done = render(&[[tsx.display().to_string(), "-o".into(), format!("{}/", frames.display())].as_slice(), alpha, comp].concat(), &[("RAEA_LAYER", layer)])
        .and_then(|_| if also(&frames) { raea_io_alpha::encode_dir(&frames, &out, codec, fps, 0.9).map(|m| Some((out, m))) } else { Ok(None) });
    // the frames' render leaves its manifest beside them: the reference's is the export's
    let _ = std::fs::remove_file(dir.join(format!("{name}.render.json")));
    if !keep {
        let _ = std::fs::remove_dir_all(&frames);
    }
    done
}

/// The export: the layers and layers.json in DIR, and what it says.
pub fn export(args: &[String]) -> R<(Layers, String)> {
    let tsx = args.first().filter(|a| !a.starts_with('-')).ok_or(USAGE)?;
    let tsx = PathBuf::from(tsx).canonicalize().map_err(|e| format!("{tsx}: {e}"))?;
    let edit_dir = tsx.parent().unwrap().to_path_buf();
    let codec = flag(args, "--alpha-codec").map_or(Ok(raea_io_alpha::Codec::default_here()), |c| raea_io_alpha::Codec::parse(&c))?;
    let name = flag(args, "--name").unwrap_or_else(|| tsx.file_stem().unwrap().to_string_lossy().into_owned());
    let dir = flag(args, "-o").map_or_else(|| edit_dir.join("out/layers").join(&name), PathBuf::from);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
    let comp: Vec<String> = flag(args, "--composition").map(|c| vec!["--composition".into(), c]).unwrap_or_default();
    // LA3a's test keeps each pass's frames to composite them back
    let keep = std::env::var_os("RAEA_LAYERS_KEEP_FRAMES").is_some();

    // 1. as it is: the reference render and its manifest
    let (reference, manifest_path) = (dir.join("reference.mp4"), dir.join("render.json"));
    render(&[[tsx.display().to_string(), "-o".into(), reference.display().to_string(), "--manifest".into(), manifest_path.display().to_string()].as_slice(), &comp].concat(), &[])?;
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let (w, h, fps) = (manifest["width"].as_u64().unwrap_or(0) as u32, manifest["height"].as_u64().unwrap_or(0) as u32, manifest["fps"].as_f64().unwrap_or(30.0));
    let length = manifest["frames"].as_array().map_or(0, Vec::len) as i64;
    let runs = plan::runs(&manifest, fps);
    let said = srt::said(&manifest);

    // 2-4. the layers' passes
    // (its frames kept until the footage it covers is found)
    let graphics = pass(&tsx, &comp, &dir, "graphics", codec, fps, true, |_| true)?;
    // footage the graphics cover (opaque over most of its box at its middle frame) goes above them: the composition
    // draws it under something it draws in front of it
    let covered: Vec<bool> = runs.iter().map(|r| graphics.is_some() && opaque_over(&dir.join("graphics.frames"), r)).collect();
    if !keep {
        let _ = std::fs::remove_dir_all(dir.join("graphics.frames"));
    }
    // what's under the page: the backgrounds holding footage, and the 3D, which the whole page is drawn over
    let under = if runs.is_empty() && manifest["has3d"] != true { None } else { pass(&tsx, &comp, &dir, "under", codec, fps, keep, lit)? };
    let captions = if said.iter().any(|s| !s.is_empty()) { pass(&tsx, &comp, &dir, "captions", codec, fps, keep, |_| true)? } else { None };
    // what's behind each file shown as a picture in picture (its card), between it and the footage under it (LA2c):
    // a pass for each such file, kept only where it's a picture in picture
    let full = |r: &plan::Run| plan::full(&r.bx, w as f64, h as f64);
    let mut pips: Vec<String> = runs.iter().zip(&covered).filter(|(r, c)| !**c && !full(r)).map(|(r, _)| r.src.clone()).collect();
    pips.dedup();
    pips.sort();
    pips.dedup();
    let mut behind = Vec::new();
    for (k, src) in pips.iter().enumerate() {
        let made = pass_named(&tsx, &comp, &dir, &format!("behind-{}", k + 1), &format!("behind:{src}"), codec, fps, keep, |_| true)?;
        let shown: Vec<(i64, i64)> = runs.iter().zip(&covered).filter(|(r, c)| !**c && !full(r) && r.src == *src).map(|(r, _)| (r.start, r.end)).collect();
        behind.push((made, shown));
    }
    let srt_lines = srt::lines(&said);
    let srt_name = (!srt_lines.is_empty()).then(|| format!("{name}.srt"));
    if let Some(n) = &srt_name {
        std::fs::write(dir.join(n), srt::write(&srt_lines, fps)).map_err(|e| e.to_string())?;
    }

    // the timeline, top to bottom
    let mut files: Vec<File> = Vec::new();
    let mut index: HashMap<PathBuf, usize> = HashMap::new();
    let mut notes: Vec<String> = Vec::new();
    let mut sounds = plan::sounds(&manifest);
    let mut ids = Ids::default();
    let (mut main, mut rest, mut over, mut linked) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (r, cov) in runs.iter().zip(&covered) {
        let path = plan::resolve(&edit_dir, &r.src);
        let f = add(&mut files, &mut index, &path, video)?;
        let what = format!("{} (frames {}-{})", r.src, r.start, r.end - 1);
        if r.moved {
            notes.push(format!("{what}: moves or scales in its box; placed as its first frame shows it"));
        }
        if r.faded {
            notes.push(format!("{what}: fades; placed at its first frame's opacity"));
        }
        if r.fit == "fill" && files[f].width > 0 && ((r.bx[2] / r.bx[3].max(1.0)) - (files[f].width as f64 / files[f].height.max(1) as f64)).abs() > 0.01 * (files[f].width as f64 / files[f].height.max(1) as f64) {
            notes.push(format!("{what}: stretched to its {:.0}x{:.0} box (object-fit: fill); an editor may fit it inside instead", r.bx[2], r.bx[3]));
        }
        if *cov {
            notes.push(format!("{what}: raea draws over all of it from outside what holds it (a layer before it in the page); placed above the graphics, so their drawing over it is hidden in an editor"));
        }
        let mut clip = Clip { r#box: Some(r.bx), fit: Some(r.fit.clone()), opacity: r.opacity, speed: r.rate.unwrap_or(1.0).max(0.01), ..Clip::new(ids.next("v"), f, r.start, r.end - r.start, r.at) };
        // its own sound: the manifest's sound clip of the same file over the same frames from the same time
        if let Some(k) = sounds.iter().position(|c| plan::resolve(&edit_dir, &c.src) == path && (c.from - r.start).abs() <= 1 && (c.to - r.end).abs() <= 1 && (c.at - r.at).abs() <= 1.5 / fps) {
            let s = sounds.remove(k);
            let a = Clip { gain_db: s.gain_db, fade_in: s.fade_in, fade_out: s.fade_out, speed: s.rate, link: Some(clip.id.clone()), ..Clip::new(ids.next("a"), f, s.from, s.to - s.from, s.at) };
            clip.link = Some(a.id.clone());
            linked.push(a);
        }
        match (*cov, plan::full(&r.bx, w as f64, h as f64)) {
            (true, _) => over.push(clip),
            (false, true) => main.push(clip),
            (false, false) => rest.push(clip),
        }
    }
    let mut tracks: Vec<Track> = Vec::new();
    let movie = |files: &mut Vec<File>, made: &Option<(PathBuf, raea_io_alpha::Made)>, layer: Layer, name: &str, ids: &mut Ids, within: Option<&[(i64, i64)]>| -> Vec<Track> {
        let Some((path, m)) = made else { return Vec::new() };
        // where it draws something, and (behind a picture in picture) only within the frames given
        let spans: Vec<(i64, i64)> = match within {
            None => plan::spans(&m.drawn),
            Some(w) => plan::spans(&m.drawn).into_iter().flat_map(|(a, b)| w.iter().map(move |&(c, d)| (a.max(c), b.min(d)))).filter(|(a, b)| a < b).collect(),
        };
        if spans.is_empty() {
            let _ = std::fs::remove_file(path);
            return Vec::new();
        }
        files.push(File { path: path.file_name().unwrap().to_string_lossy().into_owned(), kind: FileKind::Video, seconds: m.frames as f64 / fps, width: m.width, height: m.height, has_audio: false, channels: 2, sample_rate: 48_000 });
        let k = files.len() - 1;
        let clips = spans.iter().map(|&(a, b)| Clip::new(ids.next(&name[..1]), k, a, b - a, a as f64 / fps)).collect();
        model::lay(layer, name, clips)
    };
    tracks.extend(movie(&mut files, &captions, Layer::Captions, "captions", &mut ids, None));
    tracks.extend(model::lay(Layer::Footage, "footage over", over));
    tracks.extend(movie(&mut files, &graphics, Layer::Graphics, "graphics", &mut ids, None));
    tracks.extend(model::lay(Layer::Footage, "picture in picture", rest));
    // every file's backing on as few tracks as hold them, under the pictures in picture
    let mut backing: Vec<Clip> = Vec::new();
    for (made, shown) in &behind {
        backing.extend(movie(&mut files, made, Layer::Backing, "behind picture in picture", &mut ids, Some(shown)).into_iter().flat_map(|t| t.clips));
    }
    backing.sort_by_key(|c| c.start);
    tracks.extend(model::lay(Layer::Backing, "behind picture in picture", backing));
    tracks.extend(model::lay(Layer::Footage, "footage", main));
    tracks.extend(movie(&mut files, &under, Layer::Under, "under", &mut ids, None));
    let mut audio = linked;
    for c in sounds {
        let f = add(&mut files, &mut index, &plan::resolve(&edit_dir, &c.src), sound)?;
        audio.push(Clip { gain_db: c.gain_db, fade_in: c.fade_in, fade_out: c.fade_out, speed: c.rate, ..Clip::new(ids.next("a"), f, c.from, c.to - c.from, c.at) });
    }
    audio.sort_by_key(|c| c.start);
    // the first sound track is the top one, so later ones are listed after it (lay lists the last laid first)
    let mut sound_tracks = model::lay(Layer::Sound, "sound", audio);
    sound_tracks.reverse();
    tracks.extend(sound_tracks);
    let notes_made = manifest["sounds"]["notes"].as_array().map_or(0, Vec::len);
    if notes_made > 0 {
        notes.push(format!("the Tone graph's music ({notes_made} notes) has no file to place: it is in the reference render's sound"));
    }

    let layers = Layers { version: model::VERSION, name: name.clone(), width: w, height: h, fps, frames: length, reference: "reference.mp4".into(), srt: srt_name.clone(), files, tracks, notes };
    let json = serde_json::to_string_pretty(&layers).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("layers.json"), json + "\n").map_err(|e| e.to_string())?;

    let count = |l: Layer| layers.tracks.iter().filter(|t| t.layer == l).map(|t| t.clips.len()).sum::<usize>();
    let mut said = format!(
        "APPLIED  layers \"{name}\" at {}: {} tracks; {}, {}, {}{}, {}, {}{}; {:.2} s at {fps} fps, {w}x{h}",
        dir.display(),
        layers.tracks.len(),
        many(count(Layer::Captions), "captions clip"),
        many(count(Layer::Graphics), "graphics clip"),
        many(count(Layer::Footage), "footage clip"),
        Some(count(Layer::Backing)).filter(|&k| k > 0).map_or(String::new(), |k| format!(", {} behind a picture in picture", many(k, "clip"))),
        if count(Layer::Under) == 1 { "1 clip under the footage".to_string() } else { format!("{} clips under the footage", count(Layer::Under)) },
        many(count(Layer::Sound), "sound"),
        srt_name.as_ref().map_or(String::new(), |n| format!("; {} in {n}", many(srt_lines.len(), "caption line"))),
        length as f64 / fps,
    );
    for n in &layers.notes {
        said.push_str(&format!("\n  note  {n}"));
    }
    Ok((layers, said))
}

/// Clip ids by kind: v1, v2... for footage, a1... for sounds, c1, g1, u1... for the captions', graphics' and under's.
#[derive(Default)]
struct Ids(HashMap<String, usize>);

impl Ids {
    fn next(&mut self, kind: &str) -> String {
        let n = self.0.entry(kind.to_string()).or_default();
        *n += 1;
        format!("{kind}{n}")
    }
}

/// "1 clip", "4 clips"
fn many(k: usize, what: &str) -> String {
    if k == 1 { format!("1 {what}") } else { format!("{k} {what}s") }
}

/// A file's place in `files`, probed the first time it's named.
fn add(files: &mut Vec<File>, index: &mut HashMap<PathBuf, usize>, p: &Path, probe: fn(&Path) -> R<File>) -> R<usize> {
    if let Some(i) = index.get(p) {
        return Ok(*i);
    }
    files.push(probe(p)?);
    index.insert(p.to_path_buf(), files.len() - 1);
    Ok(files.len() - 1)
}

/// A video file: its upright size and length from the first of raea's media backends that opens it, and whether a short
/// window of its sound decodes to samples.
pub fn video(path: &Path) -> R<File> {
    let mut why = Vec::new();
    for b in raea_io_media::preferred() {
        match b.open_video(path) {
            Ok(d) => {
                let i = d.info();
                let (has_audio, channels, sample_rate) = heard(path);
                return Ok(File { path: path.display().to_string(), kind: FileKind::Video, seconds: i.duration, width: i.width, height: i.height, has_audio, channels, sample_rate });
            }
            Err(e) => why.push(format!("{}: {e}", b.name())),
        }
    }
    Err(format!("{} didn't open as video ({})", path.display(), why.join("; ")))
}

/// A sound file: its length from its container.
pub fn sound(path: &Path) -> R<File> {
    let seconds = raea_io_media::probe_seconds(path).ok_or_else(|| format!("{}: no sound length", path.display()))?;
    let (_, channels, sample_rate) = heard(path);
    Ok(File { path: path.display().to_string(), kind: FileKind::Audio, seconds, width: 0, height: 0, has_audio: true, channels, sample_rate })
}

/// Whether a short window of a file's sound decodes to samples, and its channels and sample rate (stereo at 48 kHz
/// when it has none to read).
fn heard(path: &Path) -> (bool, u32, u32) {
    match raea_io_media::decode_audio_window(path, 0.0, 0.05) {
        Ok(a) if a.len() > 0 => (true, (a.channels.len() as u32).max(1), a.rate),
        _ => (false, 2, 48_000),
    }
}

pub fn codec_ext(c: raea_io_alpha::Codec) -> &'static str {
    match c {
        raea_io_alpha::Codec::ProRes => "prores",
        raea_io_alpha::Codec::Hevc => "hevc",
        raea_io_alpha::Codec::Rle => "rle",
        raea_io_alpha::Codec::Png => "png",
    }
}

/// Whether the graphics are opaque over 90% of a run's box at its middle frame (a grid of points inside it).
fn opaque_over(frames: &Path, r: &plan::Run) -> bool {
    let mid = (r.start + r.end - 1) / 2;
    let Ok((px, w, h)) = raea_io_alpha::read_png(&frames.join(format!("frame-{mid:05}.png"))) else { return false };
    let (mut n, mut opaque) = (0, 0);
    for j in 0..16 {
        for i in 0..16 {
            let x = (r.bx[0] + r.bx[2] * (i as f64 + 0.5) / 16.0) as i64;
            let y = (r.bx[1] + r.bx[3] * (j as f64 + 0.5) / 16.0) as i64;
            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                continue;
            }
            n += 1;
            opaque += usize::from(px[((y as usize) * w as usize + x as usize) * 4 + 3] >= 250);
        }
    }
    n > 0 && opaque * 10 >= n * 9
}

/// Whether any frame in `frames` draws anything but black, at any alpha (a grid of points on each).
pub fn lit(frames: &Path) -> bool {
    raea_io_alpha::frames_in(frames).iter().any(|p| {
        let Ok((px, w, h)) = raea_io_alpha::read_png(p) else { return true };
        (0..32).any(|j| {
            (0..32).any(|i| {
                let (x, y) = ((w as usize * (2 * i + 1)) / 64, (h as usize * (2 * j + 1)) / 64);
                let k = (y * w as usize + x) * 4;
                px[k + 3] >= 8 && px[k..k + 3].iter().any(|&c| c >= 8)
            })
        })
    })
}
