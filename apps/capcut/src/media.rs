//! The draft's cover: the reference render's first frame as a small JPEG. (The files a draft uses are probed by the
//! layers export, raea-app-layers.)

use std::path::Path;

/// The first frame of `video`, at most 540 pixels on its long side, as a JPEG.
pub fn cover(video: &Path) -> Option<Vec<u8>> {
    let mut d = raea_io_media::preferred().into_iter().find_map(|b| b.open_video(video).ok())?;
    let f = d.frame_at(0.0).ok()?;
    let (w, h) = (f.width as usize, f.height as usize);
    let step = (w.max(h)).div_ceil(540).max(1);
    let (tw, th) = (w / step, h / step);
    let mut rgb = Vec::with_capacity(tw * th * 3);
    for y in 0..th {
        for x in 0..tw {
            let k = ((y * step) * w + x * step) * 4;
            rgb.extend_from_slice(&f.rgba[k..k + 3]);
        }
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode(&rgb, tw as u32, th as u32, image::ExtendedColorType::Rgb8).ok()?;
    Some(out)
}
