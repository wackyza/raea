//! CC2a without a render: where a picture sits in CapCut's terms, and layers.json as CapCut's tracks (LA4b). The
//! manifest's runs, sounds and spans are tested where they're read, in raea-app-layers.

use std::path::Path;

use raea_app_layers::model::{self, Clip as LClip, File, FileKind, Layer, Layers, Track as LTrack};

use crate::from_layers;
use crate::plan::*;

#[test]
fn pictures_sit_as_their_fit_shows_them() {
    // a landscape take covering a portrait canvas: scaled past the fitted width until it fills the height
    let (p, w) = place(&[0.0, 0.0, 1080.0, 1920.0], "cover", 1920.0, 1080.0, 1080.0, 1920.0, 1.0);
    assert!(w.is_none() && (p.scale - (1920.0 / 1080.0) / 0.5625).abs() < 1e-9 && p.crop == [0.0, 0.0, 1.0, 1.0]);
    assert_eq!((p.x, p.y), (540.0, 960.0));
    // a square in a corner, contained: a third of the canvas's width
    let (p, _) = place(&[660.0, 200.0, 360.0, 360.0], "contain", 640.0, 640.0, 1080.0, 1920.0, 1.0);
    assert!((p.scale - 1.0 / 3.0).abs() < 1e-9 && (p.x, p.y) == (840.0, 380.0));
    // a landscape take covering a square box: cropped to its middle square, which then fits as a square
    let (p, _) = place(&[0.0, 0.0, 540.0, 540.0], "cover", 1920.0, 1080.0, 1080.0, 1920.0, 1.0);
    assert!((p.crop[0] - 0.21875).abs() < 1e-9 && (p.crop[2] - 0.78125).abs() < 1e-9 && p.crop[1] == 0.0);
    assert!((p.scale - 0.5).abs() < 1e-9);
    // stretched in raea: said
    assert!(place(&[0.0, 0.0, 500.0, 100.0], "fill", 640.0, 640.0, 1080.0, 1920.0, 1.0).1.is_some());
}

#[test]
fn layers_json_goes_bottom_first_with_footage_carrying_its_own_sound() {
    let file = |path: &str, kind, w, h| File { path: path.into(), kind, seconds: 3.0, width: w, height: h, has_audio: kind == FileKind::Video };
    let clip = |id: &str, f, start, frames| LClip::new(id.into(), f, start, frames, 0.0);
    let t = |layer, name: &str, clips| LTrack { layer, name: name.into(), clips };
    let v1 = LClip { r#box: Some([0.0, 0.0, 1080.0, 1920.0]), fit: Some("cover".into()), link: Some("a1".into()), ..clip("v1", 0, 0, 90) };
    let a1 = LClip { gain_db: -6.0, link: Some("v1".into()), ..clip("a1", 0, 0, 90) };
    let l = Layers {
        version: model::VERSION,
        name: "edit".into(),
        width: 1080,
        height: 1920,
        fps: 30.0,
        frames: 90,
        reference: "reference.mp4".into(),
        srt: Some("edit.srt".into()),
        files: vec![file("/m/a.mp4", FileKind::Video, 1920, 1080), file("captions.prores.mov", FileKind::Video, 1080, 1920), file("graphics.prores.mov", FileKind::Video, 1080, 1920), file("/m/music.wav", FileKind::Audio, 0, 0)],
        tracks: vec![
            t(Layer::Captions, "captions", vec![clip("c1", 1, 10, 30)]),
            t(Layer::Graphics, "graphics", vec![clip("g1", 2, 0, 90)]),
            t(Layer::Footage, "footage", vec![v1]),
            t(Layer::Sound, "sound", vec![a1, LClip { gain_db: -12.0, fade_out: 0.5, ..clip("a2", 3, 0, 90) }]),
        ],
        notes: vec![],
    };
    let tl = from_layers::timeline(&l, Path::new("/out/edit"));
    // bottom first: the footage is CapCut's main track (no name), then the overlay, then the captions; sound after
    assert_eq!(tl.tracks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["", "raea overlay", "raea captions", "sound"]);
    // a layer's movie is found beside layers.json; the footage where it is
    assert_eq!(tl.media[1].path, Path::new("/out/edit/captions.prores.mov"));
    assert_eq!(tl.media[0].path, Path::new("/m/a.mp4"));
    // the footage carries its own sound at its gain (-6 dB), so that sound isn't a clip of its own; the music is
    let foot = &tl.tracks[0].clips[0];
    assert!((foot.volume - 10f64.powf(-6.0 / 20.0)).abs() < 1e-9);
    assert_eq!(tl.tracks[3].clips.len(), 1);
    assert!((tl.tracks[3].clips[0].volume - 10f64.powf(-12.0 / 20.0)).abs() < 1e-9 && tl.tracks[3].clips[0].fade_out == 0.5);
    // the layers' movies fill the canvas, silent
    let cap = &tl.tracks[2].clips[0];
    assert_eq!((cap.start, cap.frames, cap.volume, cap.place.scale, cap.place.x, cap.place.y), (10, 30, 0.0, 1.0, 540.0, 960.0));
    // footage with no linked sound plays silent
    let mut quiet = l.clone();
    quiet.tracks[2].clips[0].link = None;
    quiet.tracks[3].clips.remove(0);
    assert_eq!(from_layers::timeline(&quiet, Path::new("/out/edit")).tracks[0].clips[0].volume, 0.0);
}
