//! layers.json as an FCP7 XML project (xmeml version 5, docs/layers.feature LA5a), which Premiere Pro (File, Import)
//! and DaVinci Resolve (File, Import, Timeline) both read: one sequence at the composition's rate and size, its video
//! tracks bottom first (under, the footage, what's behind a picture in picture, the pictures in picture, graphics,
//! captions), its audio tracks below. Footage points at its file where it is, placed as raea drew it (Basic Motion's
//! scale and centre, a Crop where object-fit: cover cuts it, its Opacity); a footage clip and its own sound are linked
//! clipitems, which Premiere's Unlink and Resolve's Link Clips separate. Sound is laid out as Premiere writes it (read
//! from an XML Premiere Pro 2026 exported): a stereo file's channels each a mono clipitem on a track of its own, each
//! panned to its side by an Audio Pan on the clip, as Resolve and FCP7 write it (both editors import a channel centred
//! without it; Premiere's own track pan, PannerCurrentValue, was tried and neither read it). A layer's movie fills the frame, its alpha
//! straight. Pure: the export (lib.rs) makes the layers and writes this beside them.

use std::fmt::Write as _;
use std::path::Path;

use crate::model::{Clip, FileKind, Layer, Layers};

/// xmeml's rate: a whole timebase and whether it runs at 1000/1001 of it (29.97 is 30 NTSC).
fn rate(fps: f64) -> (u32, bool) {
    let base = fps.round().max(1.0);
    (base as u32, (fps - base).abs() > 0.001 && (fps * 1001.0 / 1000.0 - base).abs() < 0.01)
}

fn rate_xml(fps: f64) -> String {
    let (tb, ntsc) = rate(fps);
    format!("<rate><timebase>{tb}</timebase><ntsc>{}</ntsc></rate>", if ntsc { "TRUE" } else { "FALSE" })
}

/// A timecode from 0, as Premiere gives a sequence and every file.
fn timecode(fps: f64) -> String {
    let drop = rate(fps).1;
    format!("<timecode>{}<string>{}</string><frame>0</frame><displayformat>{}</displayformat></timecode>", rate_xml(fps), if drop { "00;00;00;00" } else { "00:00:00:00" }, if drop { "DF" } else { "NDF" })
}

/// A UUID-shaped id for the sequence, the same for the same name (FNV-1a over it, twice).
fn uuid(name: &str) -> String {
    let h = |seed: u64| name.bytes().fold(seed, |k, b| (k ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let (a, b) = (h(0xcbf2_9ce4_8422_2325), h(0x8422_2325_cbf2_9ce4));
    format!("{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}", a >> 32, (a >> 16) & 0xffff, a & 0xfff, (b >> 52) & 0xfff, b & 0xffff_ffff_ffff)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A file's path as xmeml's pathurl: file://localhost and the absolute path, each byte outside the unreserved set
/// percent-encoded.
fn pathurl(p: &Path) -> String {
    let mut out = String::from("file://localhost");
    for b in p.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Where a picture of `mw` x `mh` sits in `bx` with CSS's object-fit `fit`, in Basic Motion's terms on a `w` x `h`
/// sequence: its scale (100: the media's own pixels), its centre (0, 0: the frame's; a unit is the frame's width or
/// height) and what Crop cuts from each side (percent of the media: left, right, top, bottom); and a note when it
/// can't be shown so (a stretched picture).
pub fn motion(bx: [f64; 4], fit: &str, mw: f64, mh: f64, w: f64, h: f64) -> (f64, [f64; 2], [f64; 4], Option<String>) {
    let [x, y, bw, bh] = bx;
    let (sx, sy) = (bw / mw, bh / mh);
    let mut note = None;
    let s = match fit {
        "cover" => sx.max(sy),
        "none" => 1.0,
        "scale-down" => sx.min(sy).min(1.0),
        "contain" => sx.min(sy),
        _ => {
            if (sx - sy).abs() > 0.01 * sx.max(sy) {
                note = Some(format!("stretched to its {bw:.0}x{bh:.0} box (object-fit: fill); fitted inside it"));
            }
            sx.min(sy)
        }
    };
    // what's drawn past its box is cut, each side by the share of the scaled picture that overhangs it
    let (dw, dh) = (mw * s, mh * s);
    let cut = |d: f64, b: f64| ((d - b) / 2.0 / d * 100.0).max(0.0);
    let (cx, cy) = (cut(dw, bw), cut(dh, bh));
    let centre = [(x + bw / 2.0 - w / 2.0) / w, (y + bh / 2.0 - h / 2.0) / h];
    (s * 100.0, centre, [cx, cx, cy, cy], note)
}

fn param(id: &str, name: &str, min: f64, max: f64, value: &str) -> String {
    format!("<parameter><parameterid>{id}</parameterid><name>{name}</name><valuemin>{min}</valuemin><valuemax>{max}</valuemax><value>{value}</value></parameter>")
}

fn effect(name: &str, id: &str, category: &str, kind: &str, media: &str, params: &str) -> String {
    format!("<filter><effect><name>{name}</name><effectid>{id}</effectid><effectcategory>{category}</effectcategory><effecttype>{kind}</effecttype><mediatype>{media}</mediatype>{params}</effect></filter>")
}

/// The XML, and what it can't carry. `dir` is where layers.json is: its movies' paths are relative to it.
pub fn write(l: &Layers, dir: &Path) -> (String, Vec<String>) {
    let mut notes = Vec::new();
    let fps = l.fps;
    let frames = |secs: f64| (secs * fps).round() as i64;
    let path = |k: usize| {
        let p = Path::new(&l.files[k].path);
        if p.is_absolute() { p.to_path_buf() } else { dir.join(p) }
    };
    let name_of = |k: usize| path(k).file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());

    // the tracks bottom first, xmeml's order; sounds apart, each sound track as one mono track a channel, as Premiere
    // writes a stereo clip: its left channel on one track, its right on the next, two clipitems linked
    let video: Vec<_> = l.tracks.iter().rev().filter(|t| t.layer != Layer::Sound).collect();
    let audio: Vec<_> = l.tracks.iter().filter(|t| t.layer == Layer::Sound).collect();
    let channels = |c: &Clip| l.files[c.file].channels.clamp(1, 2) as usize;
    let mut mono: Vec<Vec<(String, &Clip, usize)>> = Vec::new();
    for t in &audio {
        let base = mono.len();
        mono.resize_with(base + t.clips.iter().map(channels).max().unwrap_or(1), Vec::new);
        for c in &t.clips {
            for ch in 1..=channels(c) {
                mono[base + ch - 1].push((if ch == 1 { c.id.clone() } else { format!("{}-{ch}", c.id) }, c, ch));
            }
        }
    }
    // each clipitem's place: (video?, track index from 1, clip index from 1), for links
    let mut at: std::collections::HashMap<String, (bool, usize, usize)> = std::collections::HashMap::new();
    for (ti, t) in video.iter().enumerate() {
        for (ci, c) in t.clips.iter().enumerate() {
            at.insert(c.id.clone(), (true, ti + 1, ci + 1));
        }
    }
    for (ti, m) in mono.iter().enumerate() {
        for (ci, (id, _, _)) in m.iter().enumerate() {
            at.insert(id.clone(), (false, ti + 1, ci + 1));
        }
    }
    let by_id: std::collections::HashMap<&str, &Clip> = l.tracks.iter().flat_map(|t| t.clips.iter()).map(|c| (c.id.as_str(), c)).collect();
    // a clip's clipitems: footage's one, a sound's one a channel
    let items = |c: &Clip| -> Vec<String> { std::iter::once(c.id.clone()).chain((2..=2).map(|ch| format!("{}-{ch}", c.id))).filter(|id| at.contains_key(id)).collect() };
    // every clipitem of a clip and of the clip it's linked to, linked together (the video first), as Premiere links them
    let links = |c: &Clip| -> String {
        let mut all = items(c);
        if let Some(other) = c.link.as_deref().and_then(|o| by_id.get(o)) {
            all.extend(items(other));
        }
        if all.len() < 2 {
            return String::new();
        }
        all.sort_by_key(|id| (!at[id].0, at[id].1));
        all.iter()
            .map(|id| {
                let (v, ti, ci) = at[id];
                format!("<link><linkclipref>{id}</linkclipref><mediatype>{}</mediatype><trackindex>{ti}</trackindex><clipindex>{ci}</clipindex></link>", if v { "video" } else { "audio" })
            })
            .collect()
    };
    // a file is described in full the first time a clip names it, by its id after; its sound a channel at a time
    let mut described = std::collections::HashSet::new();
    let mut file = |k: usize| -> String {
        let id = format!("file-{}", k + 1);
        if !described.insert(k) {
            return format!("<file id=\"{id}\"/>");
        }
        let f = &l.files[k];
        let mut media = String::new();
        if f.kind == FileKind::Video {
            let _ = write!(media, "<video><samplecharacteristics>{}<width>{}</width><height>{}</height></samplecharacteristics></video>", rate_xml(fps), f.width, f.height);
        }
        if f.has_audio {
            let n = f.channels.clamp(1, 2);
            for ch in 1..=n {
                let label = match (n, ch) {
                    (1, _) => "mono",
                    (_, 1) => "left",
                    _ => "right",
                };
                let _ = write!(media, "<audio><samplecharacteristics><depth>16</depth><samplerate>{}</samplerate></samplecharacteristics><channelcount>1</channelcount><layout>{}</layout><audiochannel><sourcechannel>{ch}</sourcechannel><channellabel>{label}</channellabel></audiochannel></audio>", f.sample_rate, if n == 1 { "mono" } else { "stereo" });
            }
        }
        format!("<file id=\"{id}\"><name>{}</name><pathurl>{}</pathurl>{}<duration>{}</duration>{}<media>{media}</media></file>", esc(&name_of(k)), pathurl(&path(k)), rate_xml(fps), frames(f.seconds), timecode(fps))
    };
    let clipitem = |id: &str, attrs: &str, c: &Clip, body: &str, file: String| -> String {
        let src_in = frames(c.source_in);
        format!(
            "<clipitem id=\"{id}\"{attrs}><masterclipid>masterclip-{}</masterclipid><name>{}</name><enabled>TRUE</enabled><duration>{}</duration>{}<start>{}</start><end>{}</end><in>{src_in}</in><out>{}</out>{file}{body}{}</clipitem>",
            c.file + 1,
            esc(&name_of(c.file)),
            frames(l.files[c.file].seconds),
            rate_xml(fps),
            c.start,
            c.end(),
            src_in + (c.frames as f64 * c.speed).round() as i64,
            links(c)
        )
    };

    let mut v = String::new();
    for t in &video {
        let _ = write!(v, "<track>");
        for c in &t.clips {
            let mut body = String::new();
            if (c.speed - 1.0).abs() > 0.001 {
                notes.push(format!("{} (frames {}-{}): plays at {:.2}x in raea; placed at its own speed", name_of(c.file), c.start, c.end() - 1, c.speed));
            }
            match (t.layer, c.r#box) {
                (Layer::Footage, Some(bx)) => {
                    let f = &l.files[c.file];
                    let (scale, centre, crop, note) = motion(bx, c.fit.as_deref().unwrap_or("fill"), f.width.max(1) as f64, f.height.max(1) as f64, l.width as f64, l.height as f64);
                    if let Some(n) = note {
                        notes.push(format!("{} (frames {}-{}): {n}", name_of(c.file), c.start, c.end() - 1));
                    }
                    body.push_str(&effect("Basic Motion", "basic", "motion", "motion", "video", &format!("{}{}", param("scale", "Scale", 0.0, 1000.0, &format!("{scale:.4}")), format!("<parameter><parameterid>center</parameterid><name>Center</name><value><horiz>{:.6}</horiz><vert>{:.6}</vert></value></parameter>", centre[0], centre[1]))));
                    if crop.iter().any(|&k| k > 0.001) {
                        body.push_str(&effect("Crop", "crop", "motion", "motion", "video", &["left", "right", "top", "bottom"].iter().zip(crop).map(|(side, k)| param(side, &format!("{}{}", side[..1].to_uppercase(), &side[1..]), 0.0, 100.0, &format!("{k:.4}"))).collect::<String>()));
                    }
                    if c.opacity < 0.999 {
                        body.push_str(&effect("Opacity", "opacity", "motion", "motion", "video", &param("opacity", "Opacity", 0.0, 100.0, &format!("{:.2}", c.opacity * 100.0))));
                    }
                }
                // a layer's movie fills the frame, its alpha straight
                _ => body.push_str("<alphatype>straight</alphatype>"),
            }
            let _ = write!(v, "{}", clipitem(&c.id, "", c, &body, file(c.file)));
        }
        let _ = write!(v, "<enabled>TRUE</enabled><locked>FALSE</locked></track>");
    }
    let mut a = String::new();
    for (ti, m) in mono.iter().enumerate() {
        // a stereo pair's left track goes to output channel 1, its right to 2
        // which side of a stereo pair the track is: 1 left, 2 right; none for a mono file's own track
        let side = if m.iter().any(|(_, _, ch)| *ch == 2) { Some(2) } else if mono.get(ti + 1).is_some_and(|n| n.iter().any(|(_, _, ch)| *ch == 2)) { Some(1) } else { None };
        let out = side.map_or(String::new(), |k| format!("<outputchannelindex>{k}</outputchannelindex>"));
        let _ = write!(a, "<track currentExplodedTrackIndex=\"0\" totalExplodedTrackCount=\"1\" premiereTrackType=\"Mono\">");
        for (id, c, ch) in m {
            let gain = 10f64.powf(c.gain_db / 20.0);
            let mut body = format!("<sourcetrack><mediatype>audio</mediatype><trackindex>{ch}</trackindex></sourcetrack>");
            if (gain - 1.0).abs() > 1e-6 {
                body.push_str(&effect("Audio Levels", "audiolevels", "audiolevels", "audiolevels", "audio", &param("level", "Level", 0.00003162, 31.6228, &format!("{gain:.6}"))));
            }
            // a stereo file's channel panned to its own side, as FCP7's Audio Pan on the clip (-1 left, 1 right): Premiere
            // and Resolve both import each channel centred without it, and both read it (heard 11-16); a mono file's
            // stays centred
            if channels(c) == 2 {
                body.push_str(&effect("Audio Pan", "audiopan", "audiopan", "audiopan", "audio", &param("pan", "Pan", -1.0, 1.0, if *ch == 1 { "-1" } else { "1" })));
            }
            if *ch == 1 && (c.fade_in > 0.0 || c.fade_out > 0.0) {
                notes.push(format!("{} (frames {}-{}): its fades ({:.2} s in, {:.2} s out) aren't carried; set them in the editor", name_of(c.file), c.start, c.end() - 1, c.fade_in, c.fade_out));
            }
            let _ = write!(a, "{}", clipitem(id, " premiereChannelType=\"mono\"", c, &body, file(c.file)));
        }
        let _ = write!(a, "<enabled>TRUE</enabled><locked>FALSE</locked>{out}</track>");
    }
    let format = format!("<format><samplecharacteristics>{}<width>{}</width><height>{}</height><pixelaspectratio>square</pixelaspectratio><anamorphic>FALSE</anamorphic><fielddominance>none</fielddominance></samplecharacteristics></format>", rate_xml(fps), l.width, l.height);
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n<xmeml version=\"5\"><sequence id=\"sequence-1\" explodedTracks=\"true\"><uuid>{}</uuid><name>{}</name><duration>{}</duration>{}{}<media><video>{format}{v}</video><audio><numOutputChannels>2</numOutputChannels><format><samplecharacteristics><depth>16</depth><samplerate>48000</samplerate></samplecharacteristics></format><outputs><group><index>1</index><numchannels>1</numchannels><downmix>0</downmix><channel><index>1</index></channel></group><group><index>2</index><numchannels>1</numchannels><downmix>0</downmix><channel><index>2</index></channel></group></outputs>{a}</audio></media></sequence></xmeml>\n",
        uuid(&l.name),
        esc(&l.name),
        l.frames,
        rate_xml(fps),
        timecode(fps),
    );
    (xml, notes)
}
