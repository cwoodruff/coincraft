//! Draws the CoinCraft app icon at every size the installers need.
//!
//!     cargo run --example make_icons
//!
//! Writes PNGs and a Windows .ico into assets/icons/. The design matches the logo in the
//! sidebar: a coral coin with a light ring and a small stacked coin, on a dark rounded square.

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::{ExtendedColorType, Rgba, RgbaImage};
use std::path::Path;

/// Supersampling per axis, for smooth edges.
const AA: u32 = 4;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Blends `top` over `base` (both straight alpha, 0..1 floats).
fn over(base: [f32; 4], top: [f32; 4]) -> [f32; 4] {
    let a = top[3] + base[3] * (1.0 - top[3]);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let mix = |i: usize| (top[i] * top[3] + base[i] * base[3] * (1.0 - top[3])) / a;
    [mix(0), mix(1), mix(2), a]
}

fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// The colour at one point of the icon, with x and y from 0 to 1.
fn shade(x: f32, y: f32) -> [f32; 4] {
    let mut px = [0.0f32; 4];

    // Dark rounded square, lighter at the top.
    let (inset, radius) = (0.04, 0.2);
    let qx = (x - 0.5).abs() - (0.5 - inset - radius);
    let qy = (y - 0.5).abs() - (0.5 - inset - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius;
    if outside <= 0.0 {
        let top = rgb(34, 47, 78);
        let bottom = rgb(12, 18, 34);
        let t = ((y - inset) / (1.0 - 2.0 * inset)).clamp(0.0, 1.0);
        px = [lerp(top[0], bottom[0], t), lerp(top[1], bottom[1], t), lerp(top[2], bottom[2], t), 1.0];
    }

    let d = |cx: f32, cy: f32| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    let (cx, cy) = (0.47, 0.53);

    // Coral coin with a soft lower edge.
    if d(cx, cy) <= 0.30 {
        let shadow = ((y - cy) / 0.30).clamp(0.0, 1.0) * 0.18;
        let coral = rgb(242, 95, 92);
        px = over(px, [coral[0] * (1.0 - shadow), coral[1] * (1.0 - shadow), coral[2] * (1.0 - shadow), 1.0]);
    }
    // Light ring on the coin.
    let ring = (d(cx, cy) - 0.205).abs();
    if ring <= 0.018 {
        px = over(px, [1.0, 0.82, 0.76, 0.95]);
    }
    // Small stacked coin, top right, with a dark outline so it reads at small sizes.
    let sd = d(cx + 0.17, cy - 0.17);
    if sd <= 0.135 {
        px = over(px, rgb(22, 32, 56));
    }
    if sd <= 0.11 {
        px = over(px, rgb(255, 190, 150));
    }
    px
}

fn draw(size: u32) -> RgbaImage {
    RgbaImage::from_fn(size, size, |x, y| {
        let mut acc = [0.0f32; 4];
        for sy in 0..AA {
            for sx in 0..AA {
                let fx = (x as f32 + (sx as f32 + 0.5) / AA as f32) / size as f32;
                let fy = (y as f32 + (sy as f32 + 0.5) / AA as f32) / size as f32;
                let c = shade(fx, fy);
                // Average in premultiplied form so edges don't darken.
                acc[0] += c[0] * c[3];
                acc[1] += c[1] * c[3];
                acc[2] += c[2] * c[3];
                acc[3] += c[3];
            }
        }
        let n = (AA * AA) as f32;
        let a = acc[3] / n;
        let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        if a <= 0.0 {
            Rgba([0, 0, 0, 0])
        } else {
            Rgba([to8(acc[0] / n / a), to8(acc[1] / n / a), to8(acc[2] / n / a), to8(a)])
        }
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("icons");
    std::fs::create_dir_all(&dir)?;

    for size in [32u32, 64, 128, 256, 512, 1024] {
        let path = dir.join(format!("{size}x{size}.png"));
        draw(size).save(&path)?;
        println!("wrote {}", path.display());
    }
    // macOS asks for "@2x" names for its high-density sizes.
    draw(256).save(dir.join("128x128@2x.png"))?;

    // Windows wants a single .ico holding several sizes.
    let frames = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&s| IcoFrame::as_png(draw(s).as_raw(), s, s, ExtendedColorType::Rgba8))
        .collect::<Result<Vec<_>, _>>()?;
    let ico = dir.join("icon.ico");
    IcoEncoder::new(std::fs::File::create(&ico)?).encode_images(&frames)?;
    println!("wrote {}", ico.display());
    Ok(())
}
