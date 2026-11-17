//! `raea export capcut EDIT.tsx` (docs/capcut.feature CC2, docs/layers.feature LA4b): the composition as a CapCut draft
//! ready in its timeline. And `raea render ... -o OUT.mov --alpha` (CC1), the transparent movie its overlays are, for
//! any editor. The composition is exported in layers first (raea-app-layers: the reference render and its manifest,
//! the graphics, captions and what's under the footage, layers.json); layers.json is then written as CapCut's tracks
//! (from_layers.rs) and raea-io-capcut writes the draft, with the reference render copied into its folder.

mod from_layers;
mod media;
pub mod plan;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use raea_io_capcut::Kind;
use raea_runtime_bundle::flag;

pub type R<T> = Result<T, String>;

pub const USAGE: &str = "raea export capcut EDIT.tsx [--composition ID] [-o DIR] [--name NAME] [--alpha-codec prores|hevc|rle|png] [--no-register]   (the composition as a CapCut draft ready in its timeline: its footage and sounds as clips of their own files at their trims and gains, everything raea draws as transparent overlay clips above them; the captions on a track of their own above it; into CapCut's projects, entered in its project list while CapCut is closed, or a draft folder in DIR; the reference render copied into the draft's folder, and it and the layers kept in out/capcut/NAME)";

/// `export capcut ...` (or what export takes), and `render ... -o OUT.mov --alpha` through `render`, the CLI's own;
/// None for anything else.
pub fn cli(args: &[String], render: impl Fn(&[String]) -> R<String>) -> Option<R<String>> {
    match args.first().map(String::as_str) {
        Some("export") => Some(match args.get(1).map(String::as_str) {
            Some("capcut") => export(&args[2..]),
            other => Err(format!("raea export takes where to export to{}: capcut, layers or xml\nusage: {USAGE}\n       {}\n       {}", other.map_or(String::new(), |o| format!(", not {o:?}")), raea_app_layers::USAGE, raea_app_layers::USAGE_XML)),
        }),
        Some("render") => movie(args, render),
        _ => None,
    }
}

/// `render ... -o OUT.mov --alpha`: the frames as transparent PNGs, then one movie of them, the overlay an editor lays
/// over footage (raea-io-alpha, docs/capcut.feature CC1); None for any other render.
fn movie(args: &[String], render: impl Fn(&[String]) -> R<String>) -> Option<R<String>> {
    let out = flag(args, "-o").filter(|o| args.iter().any(|a| a == "--alpha") && o.to_lowercase().ends_with(".mov"))?;
    let quality = match flag(args, "--quality").map(|q| raea_io_media::parse_quality(&q)).transpose() {
        Ok(q) => q.unwrap_or(0.9),
        Err(e) => return Some(Err(e.to_string())),
    };
    Some(raea_io_alpha::render_movie(args, Path::new(&out), flag(args, "--alpha-codec").as_deref(), flag(args, "--manifest").map(PathBuf::from), quality, render))
}

fn export(args: &[String]) -> R<String> {
    let tsx = args.first().filter(|a| !a.starts_with('-')).ok_or(USAGE)?;
    let tsx = PathBuf::from(tsx).canonicalize().map_err(|e| format!("{tsx}: {e}"))?;
    let dir = tsx.parent().unwrap().to_path_buf();
    let codec = flag(args, "--alpha-codec").map_or(Ok(raea_io_alpha::Codec::default_here()), |c| raea_io_alpha::Codec::parse(&c))?;
    let (root, in_store) = match flag(args, "-o") {
        Some(o) => (PathBuf::from(o), false),
        None => {
            let s = raea_io_capcut::store_root().ok_or("CapCut keeps its projects only on a Mac or Windows: -o DIR writes the draft folder anywhere")?;
            if !s.parent().is_some_and(Path::exists) {
                return Err(format!("CapCut hasn't kept a project on this machine ({} isn't there): open CapCut once, or write the draft folder with -o DIR", s.display()));
            }
            if !args.iter().any(|a| a == "--no-register") && raea_io_capcut::capcut_running() {
                return Err("CapCut is open: it rewrites its project list when it quits, which would drop the new draft from it. Quit CapCut and export again, or pass --no-register (the draft lands in its folder, unlisted) or -o DIR".into());
            }
            (s, true)
        }
    };
    let stem = tsx.file_stem().unwrap().to_string_lossy().into_owned();
    let mut name = flag(args, "--name").unwrap_or(stem);
    if root.join(&name).exists() {
        let base = name.clone();
        name = (2..).map(|k| format!("{base} {k}")).find(|n| !root.join(n).exists()).unwrap();
    }
    let work = dir.join("out/capcut").join(&name);

    // 1. the layers and layers.json (raea export layers), in out/capcut/NAME
    let mut layer_args = vec![tsx.display().to_string(), "-o".into(), work.display().to_string(), "--name".into(), name.clone(), "--alpha-codec".into(), raea_app_layers::codec_ext(codec).into()];
    if let Some(c) = flag(args, "--composition") {
        layer_args.extend(["--composition".into(), c]);
    }
    let (layers, _) = raea_app_layers::export(&layer_args)?;
    let reference = work.join(&layers.reference);

    // 2. its tracks as CapCut's, and the draft
    let tl = from_layers::timeline(&layers, &work);
    let mut notes = layers.notes.clone();
    let markers = raea_io_capcut::seed(&root);
    let written = raea_io_capcut::write(&tl, &root, &name, &markers, media::cover(&reference).as_deref())?;
    notes.extend(written.warnings.iter().cloned());
    // the reference render in the draft's folder, on no track, to set beside what CapCut plays (LA4b)
    std::fs::copy(&reference, written.dir.join("reference.mp4")).map_err(|e| format!("{}: {e}", written.dir.join("reference.mp4").display()))?;
    let listed = if !in_store || args.iter().any(|a| a == "--no-register") {
        "not entered in CapCut's project list".to_string()
    } else {
        match raea_io_capcut::register(&root, &written.dir, &written.meta, written.info_bytes, &backups()) {
            Ok(b) => format!("entered in CapCut's project list (its list before: {})", b.display()),
            Err(e) => format!("not entered in CapCut's project list: {e}"),
        }
    };
    let count = |k: Kind, n: &str| tl.tracks.iter().filter(|t| t.kind == k && t.name.starts_with(n)).map(|t| t.clips.len()).sum::<usize>();
    let bytes = |layer: &str| std::fs::metadata(work.join(format!("{layer}.{}.mov", raea_app_layers::codec_ext(codec)))).map_or(0.0, |m| m.len() as f64 / 1e6);
    let mut said = format!(
        "APPLIED  CapCut draft \"{name}\" at {}: {} tracks, {}, {} ({}, {:.1} MB){}{}, {}; {:.2} s at {} fps, {}x{}; {listed}; markers from {}\n         the layers, layers.json and the reference render (also in the draft's folder): {}",
        written.dir.display(),
        tl.tracks.len(),
        many(count(Kind::Video, "") - count(Kind::Video, "raea "), "footage clip"),
        many(count(Kind::Video, "raea overlay"), "overlay clip"),
        codec.name(),
        bytes("graphics"),
        Some(count(Kind::Video, "raea captions")).filter(|&k| k > 0).map_or(String::new(), |k| format!(", {} ({:.1} MB){}", many(k, "captions clip"), bytes("captions"), layers.srt.as_ref().map_or(String::new(), |s| format!(" and {s}")))),
        Some(count(Kind::Video, "raea under")).filter(|&k| k > 0).map_or(String::new(), |k| format!(", {} under the footage ({:.1} MB)", many(k, "clip"), bytes("under"))),
        many(count(Kind::Audio, ""), "sound"),
        written.duration_us as f64 / 1e6,
        layers.fps,
        layers.width,
        layers.height,
        markers.from.as_deref().map_or("CapCut 8.5.0 (no draft CapCut made is in the store)".to_string(), |d| format!("the draft \"{d}\" CapCut made")),
        work.display()
    );
    for n in notes {
        said.push_str(&format!("\n  note  {n}"));
    }
    Ok(said)
}

/// "1 clip", "4 clips"
fn many(k: usize, what: &str) -> String {
    if k == 1 { format!("1 {what}") } else { format!("{k} {what}s") }
}

/// Where CapCut's project list is copied before a draft is entered in it: the person's own folder, ~/.raea/capcut/backups
/// ($RAEA_CAPCUT_BACKUPS; %LOCALAPPDATA%\raea\capcut\backups on Windows, which sets no HOME). Never raea's home, which
/// is its install or, from a checkout, the source tree (11-11: a list's backup landed in the repository).
fn backups() -> PathBuf {
    match std::env::var_os("LOCALAPPDATA").filter(|d| cfg!(windows) && !d.is_empty() && std::env::var_os("RAEA_CAPCUT_BACKUPS").is_none()) {
        Some(d) => PathBuf::from(d).join("raea").join("capcut").join("backups"),
        None => raea_io_home::env_path("RAEA_CAPCUT_BACKUPS", ".raea/capcut/backups"),
    }
}
