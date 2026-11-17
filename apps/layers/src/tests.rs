use serde_json::json;

use crate::model::{self, Clip, Layer, Layers};
use crate::plan::*;
use crate::srt;

fn said(frames: &[&[(&str, bool)]]) -> serde_json::Value {
    let frames: Vec<_> = frames.iter().enumerate().map(|(i, t)| json!({"frame": i, "videos": [], "text": t.iter().map(|(w, cap)| if *cap { json!({"text": w, "caption": true, "opacity": 1.0}) } else { json!({"text": w, "opacity": 1.0}) }).collect::<Vec<_>>()})).collect();
    json!({"frames": frames})
}

#[test]
fn a_frames_captions_are_its_caption_words_alone() {
    // docs/layers.feature LA2b: words marked caption, in the page's order; a title's words aren't
    let m = said(&[&[("Jane Doe", false), ("Hello", true), ("  there ", true)], &[], &[("Jane Doe", false)]]);
    assert_eq!(srt::said(&m), ["Hello there", "", ""]);
}

#[test]
fn a_line_lasts_while_its_words_do_and_a_highlight_starts_none() {
    let s = |v: &[&str]| v.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    // the same words over frames 0-2 (a word's colour changing isn't in the words), a gap, then a line said word by word
    let lines = srt::lines(&s(&["Hello there", "Hello there", "Hello there", "", "This", "This is", "This is raea", "Next"]));
    assert_eq!(lines, [srt::Line { text: "Hello there".into(), from: 0, to: 3 }, srt::Line { text: "This is raea".into(), from: 4, to: 7 }, srt::Line { text: "Next".into(), from: 7, to: 8 }]);
    assert_eq!(srt::write(&lines[..2], 30.0), "1\n00:00:00,000 --> 00:00:00,100\nHello there\n\n2\n00:00:00,133 --> 00:00:00,233\nThis is raea\n");
    // timecodes past a minute and an hour
    assert_eq!(srt::write(&[srt::Line { text: "x".into(), from: 108_000, to: 108_015 }], 30.0), "1\n01:00:00,000 --> 01:00:00,500\nx\n");
}

#[test]
fn clips_go_on_as_few_tracks_as_hold_them_listed_top_down() {
    let c = |id: &str, start, frames| Clip::new(id.into(), 0, start, frames, 0.0);
    let t = model::lay(Layer::Footage, "footage", vec![c("v1", 0, 30), c("v2", 10, 30), c("v3", 30, 10), c("v4", 41, 5)]);
    // v2 overlaps v1, so it goes on a second track, above the first
    assert_eq!(t.iter().map(|t| (t.name.as_str(), t.clips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>())).collect::<Vec<_>>(), [("footage 2", vec!["v2"]), ("footage", vec!["v1", "v3", "v4"])]);
}

#[test]
fn layers_json_reads_back_as_it_was_written() {
    let mut v = Clip { r#box: Some([140.0, 160.0, 800.0, 470.0]), fit: Some("cover".into()), link: Some("a1".into()), ..Clip::new("v1".into(), 0, 0, 90, 1.5) };
    v.speed = 2.0;
    let a = Clip { gain_db: -6.0, fade_in: 0.5, link: Some("v1".into()), ..Clip::new("a1".into(), 0, 0, 90, 1.5) };
    let l = Layers {
        version: model::VERSION,
        name: "edit".into(),
        width: 1080,
        height: 1920,
        fps: 30.0,
        frames: 90,
        reference: "reference.mp4".into(),
        srt: Some("edit.srt".into()),
        files: vec![model::File { path: "/m/clip.mp4".into(), kind: model::FileKind::Video, seconds: 3.0, width: 640, height: 360, has_audio: true }],
        tracks: vec![model::Track { layer: Layer::Footage, name: "footage".into(), clips: vec![v] }, model::Track { layer: Layer::Sound, name: "sound".into(), clips: vec![a] }],
        notes: vec![],
    };
    let text = serde_json::to_string(&l).unwrap();
    assert_eq!(serde_json::from_str::<Layers>(&text).unwrap(), l);
    // what a clip doesn't have isn't written: a sound has no box, footage no gain
    let j: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(j["tracks"][0]["layer"], "footage");
    assert!(j["tracks"][0]["clips"][0].get("gain_db").is_none() && j["tracks"][1]["clips"][0].get("box").is_none(), "{j}");
}

// the manifest read as runs, sounds and spans (moved from apps/capcut, docs/capcut.feature CC2a-c)

/// 90 frames at 30 fps: one take cut into two pieces (a jump in its source time at frame 45), another file in a
/// corner from frame 60, moving one pixel at frame 70. Source times in whole milliseconds, as the manifest has them.
fn manifest() -> serde_json::Value {
    let ms = |t: f64| (t * 1000.0).round() / 1000.0;
    let frames: Vec<_> = (0..90)
        .map(|f| {
            let mut v = vec![json!({"src": "media/a.mp4", "at": ms(if f < 45 { 0.5 + f as f64 / 30.0 } else { 3.0 + (f - 45) as f64 / 30.0 }), "box": [0.0, 0.0, 1080.0, 1920.0], "fit": "cover", "opacity": 1.0})];
            if f >= 60 {
                v.push(json!({"src": "media/pip.mp4", "at": ms((f - 60) as f64 / 15.0), "box": [if f >= 70 { 662.0 } else { 660.0 }, 200.0, 360.0, 360.0], "opacity": 1.0}));
            }
            json!({"frame": f, "videos": v})
        })
        .collect();
    json!({"width": 1080, "height": 1920, "fps": 30.0, "frames": frames, "sounds": {"clips": [{"src": "/m/music.wav", "from": 0, "to": 90, "at": 0.0, "gainDb": -6.0, "fadeOut": 0.5}]}})
}

#[test]
fn footage_runs_break_where_the_source_jumps() {
    let r = runs(&manifest(), 30.0);
    assert_eq!(r.iter().map(|r| (r.src.as_str(), r.start, r.end)).collect::<Vec<_>>(), [("media/a.mp4", 0, 45), ("media/a.mp4", 45, 90), ("media/pip.mp4", 60, 90)]);
    assert!((r[1].at - 3.0).abs() < 1e-9);
    assert_eq!((r[0].rate, r[1].rate), (Some(1.0), Some(1.0)), "a take at its own speed, though each step is 33 or 34 ms");
    assert!((r[2].rate.unwrap() - 2.0).abs() < 1e-6, "the corner plays at twice its speed");
    assert!(r[2].moved && !r[0].moved);
    assert_eq!(r[2].fit, "fill");
}

#[test]
fn sounds_keep_gain_and_fades() {
    let s = sounds(&manifest());
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].from, s[0].to, s[0].gain_db, s[0].fade_out), (0, 90, -6.0, 0.5));
}

#[test]
fn overlay_spans_are_runs_of_drawn_frames() {
    assert_eq!(spans(&[false, true, true, false, false, true]), [(1, 3), (5, 6)]);
    assert!(spans(&[false; 4]).is_empty());
}

/// staticFile("x.mp4") is "/media/x.mp4" in the manifest: the export finds it where the render did, in media/ beside
/// the edit, as it finds a relative path; an absolute path stays as it is
#[test]
fn a_static_file_is_found_in_the_edits_media_folder() {
    let dir = std::env::temp_dir().join(format!("raea-layers-resolve-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("media")).unwrap();
    std::fs::write(dir.join("media/a.mp4"), "").unwrap();
    let want = dir.join("media/a.mp4").canonicalize().unwrap();
    assert_eq!(resolve(&dir, "/media/a.mp4"), want);
    assert_eq!(resolve(&dir, "media/a.mp4"), want);
    assert_eq!(resolve(&dir, want.to_str().unwrap()), want);
    std::fs::remove_dir_all(&dir).unwrap();
}
