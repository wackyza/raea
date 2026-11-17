//! Layers hidden by key (RAEA_HIDE_LAYERS), as the layer pass's oracle needs them (bench/layers/oracle.sh): a page
//! element as CSS (this), a 3D part as its Visibility (runtime/scene). And the footage hidden (RAEA_HIDE_MEDIA), for
//! the CapCut export (docs/capcut.feature CC2b): `1` hides every <Video> drawn from a file and the backgrounds of what
//! holds one, so what's left is what raea draws over the footage; `under` keeps only those backgrounds, what raea
//! draws under it. And one layer of an export in layers (RAEA_LAYER, docs/layers.feature LA2a): `captions` draws only
//! what is marked data-raea-role="caption", `graphics` and `under` what RAEA_HIDE_MEDIA's `1` and `under` draw, less
//! the captions.

use crate::Layer;

/// What a caption is marked with (docs/layers.feature LA1a): its element and everything inside it.
pub const CAPTION: &str = r#"[data-raea-role="caption"], [data-raea-role="caption"] *"#;

/// What holds a caption, outside it: drawn in the captions layer only so the caption inside it is.
const HOLDS: &str = r#":has([data-raea-role="caption"]):not([data-raea-role="caption"] *)"#;

/// RAEA_HIDE_LAYERS: the layers keyed (a key a line or comma) drawn hidden, each element with its stamps and all
/// inside it; RAEA_HIDE_MEDIA: the footage (the page's only elements with data-raea-src) and what's over it or under
/// it, as above; RAEA_LAYER: one layer of an export, which wins over RAEA_HIDE_MEDIA; as page CSS. None when none of
/// them hides anything.
pub fn hidden_css() -> Option<String> {
    let var = |k: &str| std::env::var(k).ok();
    hiding_css(&var("RAEA_HIDE_LAYERS").unwrap_or_default(), var("RAEA_HIDE_MEDIA").as_deref(), var("RAEA_LAYER").as_deref())
}

/// `hidden_css` from its three settings' values, read from nowhere else. A RAEA_LAYER it doesn't know draws the page
/// as it is, and says so.
pub fn hiding_css(keys: &str, media: Option<&str>, layer: Option<&str>) -> Option<String> {
    let mut hide: Vec<String> = keys.split([',', '\n']).filter(|k| !k.trim().is_empty()).map(|k| layer_selector(k.trim())).collect();
    let mut css = String::new();
    // the footage hidden as RAEA_HIDE_MEDIA's `1` or `under` hides it
    fn footage(hide: &mut Vec<String>, css: &mut String, mode: &str) {
        match mode {
            // an element holding footage paints its background before it, under it
            "1" => {
                hide.push("[data-raea-src]".into());
                css.push_str(":has([data-raea-src]) { background: transparent !important; box-shadow: none !important; }\n");
            }
            "under" => {
                hide.extend(["[data-raea-src]".into(), ":not(:has([data-raea-src]))".into()]);
                css.push_str(":has([data-raea-src]) { color: transparent !important; text-shadow: none !important; }\n");
            }
            _ => {}
        }
    }
    match layer {
        Some("captions") => {
            // the page draws nothing inside a hidden element (vendor/blitz-paint render.rs), so what holds a caption is
            // cleared rather than hidden, as RAEA_HIDE_MEDIA's `under` clears what holds footage, and everything else
            // is hidden; the root's background is cleared too, which the canvas takes from it
            css.push_str("html, body { background: transparent !important; }\n");
            css.push_str(&format!("{HOLDS} {{ background: transparent !important; border-color: transparent !important; box-shadow: none !important; outline: none !important; }}\n"));
            hide.push(format!(":not({HOLDS}, [data-raea-role=\"caption\"], [data-raea-role=\"caption\"] *)"));
        }
        Some(mode @ ("graphics" | "under")) => {
            footage(&mut hide, &mut css, if mode == "graphics" { "1" } else { "under" });
            hide.push(CAPTION.into());
        }
        Some(other) => eprintln!("raea: RAEA_LAYER={other} isn't a layer (captions, graphics or under); the page is drawn whole"),
        None => footage(&mut hide, &mut css, media.unwrap_or_default()),
    }
    if !hide.is_empty() {
        css.push_str(&format!("{} {{ visibility: hidden !important; }}", hide.join(", ")));
    }
    (!css.is_empty()).then_some(css)
}

/// Whether an export's layer (RAEA_LAYER's value) draws the composition's 3D: only the layer under the page does
/// (docs/layers.feature LA2a). The 3D is drawn first and the whole page over it, footage included, so it's under
/// everything the page draws. It's drawn by Bevy, not the page, so the page's CSS can't hide it: runtime/scene leaves
/// its parts out of the captions and graphics layers instead.
pub fn draws_3d(layer: Option<&str>) -> bool {
    !matches!(layer, Some("captions" | "graphics"))
}

/// The CSS selector of the element a layer key names, and of everything inside it: "loc[#index][|use|use…]" as its
/// data-raea-loc, data-raea-idx and data-raea-use (engine/bundle stamp.rs)
pub fn layer_selector(key: &str) -> String {
    let l = Layer::of(key.to_string(), "element", String::new(), None, None);
    let q = |v: &str| serde_json::to_string(v).unwrap_or_default();
    let mut sel = format!("[data-raea-loc={}]", q(&l.loc));
    sel.push_str(&l.index.map_or_else(|| ":not([data-raea-idx])".to_string(), |i| format!("[data-raea-idx={}]", q(&i))));
    sel.push_str(&Some(l.uses.join("|")).filter(|u| !u.is_empty()).map_or_else(|| ":not([data-raea-use])".to_string(), |u| format!("[data-raea-use={}]", q(&u))));
    format!("{sel}, {sel} *")
}
