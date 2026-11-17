use serde_json::json;

use crate::model::{self, Clip, Layer, Layers};
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
