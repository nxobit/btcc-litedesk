use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::OnceLock,
};

pub const SIZE: usize = 32;
const OUTLINE: &str = "0a0a14";
const HEAD_OUT: &str = "14141f";
const EYE_DX: isize = 2;
const TOTAL_SUPPLY: usize = 21_000;

const PALETTE: &[&str] = &[
    "", "c5b9a1", "ffffff", "cfc2ab", "63a0f9", "807f7e", "caeff9", "5648ed", "5a423f", "b9185c",
    "cbc1bc", "b87b11", "fffdf2", "4b4949", "343235", "1f1d29", "068940", "867c1d", "ae3208",
    "9f21a0", "f98f30", "fe500c", "d26451", "fd8b5b", "5a65fa", "d22209", "e9265c", "c54e38",
    "80a72d", "4bea69", "34ac80", "eed811", "62616d", "ff638d", "8bc0c5", "c4da53", "000000",
    "f3322c", "ffae1a", "ffc110", "505a5c", "ffef16", "fff671", "fff449", "db8323", "df2c39",
    "f938d8", "5c25fb", "2a86fd", "45faff", "38dd56", "ff3a0e", "d32a09", "903707", "6e3206",
    "552e05", "e8705b", "f38b7c", "e4a499", "667af9", "648df9", "7cc4f2", "97f2fb", "a3efd0",
    "87e4d9", "71bde4", "ff1a0b", "f78a18", "2b83f6", "d62149", "834398", "ffc925", "d9391f",
    "bd2d24", "ff7216", "254efb", "e5e5de", "00a556", "c5030e", "abf131", "fb4694", "e7a32c",
    "fff0ee", "009c59", "0385eb", "00499c", "e11833", "26b1f3", "fff0be", "d8dadf", "d7d3cd",
    "1929f4", "eab118", "0b5027", "f9f5cb", "cfc9b8", "feb9d5", "f8d689", "5d6061", "76858b",
    "757576", "ff0e0e", "0adc4d", "fdf8ff", "70e890", "f7913d", "ff1ad2", "ff82ad", "535a15",
    "fa6fe2", "ffe939", "ab36be", "adc8cc", "604666", "f20422", "abaaa8", "4b65f7", "a19c9a",
    "58565c", "da42cb", "027c92", "cec189", "909b0e", "74580d", "027ee6", "b2958d", "efad81",
    "7d635e", "eff2fa", "6f597a", "d4b7b2", "d18687", "cd916d", "6b3f39", "4d271b", "85634f",
    "f9f4e6", "f8ddb0", "b92b3c", "d08b11", "257ced", "a3baed", "5fd4fb", "c16710", "a28ef4",
    "3a085b", "67b1e3", "1e3445", "ffd067", "962236", "769ca9", "5a6b7b", "7e5243", "a86f60",
    "8f785e", "cc0595", "42ffb0", "d56333", "b8ced2", "f39713", "e8e8e2", "ec5b43", "235476",
    "b2a8a5", "d6c3be", "49b38b", "fccf25", "f59b34", "375dfc", "99e6de", "27a463", "554543",
    "b19e00", "d4a015", "9f4b27", "f9e8dd", "6b7212", "9d8e6e", "4243f8", "fa5e20", "f82905",
    "555353", "876f69", "410d66", "552d1d", "f71248", "fee3f3", "c16923", "2b2834", "0079fc",
    "d31e14", "f83001", "8dd122", "fffdf4", "ffa21e", "e4afa3", "fbc311", "aa940c", "eedc00",
    "fff006", "9cb4b8", "a38654", "ae6c0a", "2bb26b", "e2c8c0", "f89865", "f86100", "dcd8d3",
    "049d43", "d0aea9", "f39d44", "eeb78c", "f9f5e9", "5d3500", "c3a199", "aaa6a4", "caa26a",
    "fde7f5", "fdf008", "fdcef2", "f681e6", "018146", "d19a54", "9eb5e1", "f5fcff", "3f9323",
    "00fcff", "4a5358", "fbc800", "d596a6", "ffb913", "e9ba12", "767c0e", "f9f6d1", "d29607",
    "f8ce47", "395ed1", "ffc5f0", "d4cfc0",
];

const DIAMOND: &[&str] = &["..T..", ".LMD.", "LMMMD", ".LMD.", "..M.."];

#[derive(Clone, Copy)]
pub struct EyeColor {
    pub m: &'static str,
    pub hi: &'static str,
    pub gl: &'static str,
    pub ol: &'static str,
}

#[derive(Clone, Copy)]
pub struct BodyColor {
    pub m: &'static str,
    pub d: &'static str,
}

#[derive(Clone, Copy)]
pub struct DiamondColor {
    pub t: &'static str,
    pub m: &'static str,
    pub d: &'static str,
    pub l: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NftInfo {
    pub id: String,
    pub index: u32,
    pub body_index: usize,
    pub head_index: usize,
    pub head_name: String,
    pub eye: String,
    pub eye_color: String,
    pub rarity: String,
    pub grad: [String; 2],
    pub body_color: String,
    pub head_hue: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NftSortMode {
    NumberAsc,
    NumberDesc,
    RarityAsc,
    RarityDesc,
}

#[derive(Debug, Clone, Deserialize)]
struct RarityRankingFile {
    items: Vec<NftRarityEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NftRarityEntry {
    pub index: u32,
    pub rarity: String,
    pub rarity_rank: u32,
}

#[derive(Deserialize)]
struct TraitsAssets {
    images: AssetImages,
}

#[derive(Deserialize)]
struct AssetImages {
    bodies: Vec<RleImage>,
    heads: Vec<RleImage>,
}

#[derive(Deserialize)]
struct RleImage {
    filename: String,
    data: String,
}

type Canvas = Vec<Option<String>>;

pub const TOTAL_SUPPLY_U32: u32 = TOTAL_SUPPLY as u32;

fn assets() -> &'static TraitsAssets {
    static ASSETS: OnceLock<TraitsAssets> = OnceLock::new();
    ASSETS.get_or_init(|| serde_json::from_str(include_str!("traits.json")).expect("valid traits.json"))
}

fn collection() -> &'static [NftInfo] {
    static COLLECTION: OnceLock<Vec<NftInfo>> = OnceLock::new();
    COLLECTION
        .get_or_init(|| serde_json::from_str(include_str!("collection_manifest.json")).expect("valid collection manifest"))
        .as_slice()
}

fn rarity_ranking() -> &'static RarityRankingFile {
    static RANKING: OnceLock<RarityRankingFile> = OnceLock::new();
    RANKING.get_or_init(|| {
        let json = include_str!("rarity_ranking.json").trim_start_matches('\u{feff}');
        serde_json::from_str(json).expect("valid rarity ranking")
    })
}

fn rarity_matches(nft: &NftInfo, rarity: Option<&str>) -> bool {
    rarity.is_none_or(|value| nft.rarity == value)
}

fn ranked_nft(entry: &NftRarityEntry) -> NftInfo {
    collection()[entry.index.saturating_sub(1) as usize].clone()
}

pub fn get_eye_colors() -> HashMap<&'static str, EyeColor> {
    HashMap::from([
        ("cyan", EyeColor { m: "2bb3a0", hi: "caeff9", gl: "7affe8", ol: "0a3a34" }),
        ("gold", EyeColor { m: "e0a81a", hi: "fff4c2", gl: "ffce3a", ol: "5a3e08" }),
        ("red", EyeColor { m: "e0344f", hi: "ffb8c8", gl: "ff5a7a", ol: "4a0a1c" }),
        ("purple", EyeColor { m: "8a5aff", hi: "c9b3ff", gl: "9a6aff", ol: "2a1452" }),
        ("silver", EyeColor { m: "b8c4d4", hi: "ffffff", gl: "e0e8f0", ol: "3a4452" }),
        ("green", EyeColor { m: "3ac46a", hi: "c2ffd6", gl: "6aff9a", ol: "0a3a1c" }),
        ("orange", EyeColor { m: "ff8a3a", hi: "ffd6b8", gl: "ffaa5a", ol: "5a2e08" }),
    ])
}

pub fn get_body_colors() -> HashMap<&'static str, BodyColor> {
    HashMap::from([
        ("ember", BodyColor { m: "d8513a", d: "8a2a1c" }),
        ("rust", BodyColor { m: "c46a2a", d: "7a3a12" }),
        ("amber", BodyColor { m: "d8a02a", d: "8a5e12" }),
        ("olive", BodyColor { m: "9aa83a", d: "5a6a1c" }),
        ("fern", BodyColor { m: "5aae4a", d: "2e6a2a" }),
        ("jade", BodyColor { m: "3aae7a", d: "1c6a48" }),
        ("teal", BodyColor { m: "2aa8a0", d: "126a64" }),
        ("ocean", BodyColor { m: "3a8ad8", d: "1c4e8a" }),
        ("azure", BodyColor { m: "4a6ae0", d: "2a3a9a" }),
        ("indigo", BodyColor { m: "6a5ad8", d: "3a2e8a" }),
        ("violet", BodyColor { m: "9a5ad8", d: "5e2e8a" }),
        ("orchid", BodyColor { m: "c45ac4", d: "7a2e7a" }),
        ("rose", BodyColor { m: "d85a8a", d: "8a2e54" }),
        ("coral", BodyColor { m: "e07a6a", d: "9a4438" }),
        ("sand", BodyColor { m: "c7b48a", d: "8a7a52" }),
        ("clay", BodyColor { m: "b08868", d: "6e5238" }),
        ("slate", BodyColor { m: "5a9ad8", d: "2e5e8a" }),
        ("steel", BodyColor { m: "4aaed0", d: "2a6e8a" }),
        ("ash", BodyColor { m: "d8b84a", d: "8a701c" }),
        ("ink", BodyColor { m: "4a4a5a", d: "24242e" }),
        ("mint", BodyColor { m: "7aceae", d: "3e8a6e" }),
        ("sky", BodyColor { m: "7ab4e8", d: "3e6ea8" }),
        ("lilac", BodyColor { m: "b09ae0", d: "6e5aaa" }),
        ("peach", BodyColor { m: "e8a87a", d: "a86a44" }),
    ])
}

pub fn get_diamond_colors() -> HashMap<&'static str, DiamondColor> {
    HashMap::from([
        ("cyan", DiamondColor { t: "ffffff", m: "7affe8", d: "2bb3a0", l: "caeff9" }),
        ("red", DiamondColor { t: "ffffff", m: "ff5a7a", d: "c01a44", l: "ffb8c8" }),
        ("gold", DiamondColor { t: "ffffff", m: "ffce3a", d: "b8901a", l: "fff4c2" }),
    ])
}

pub fn parse_nft_id(id: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 4 {
        return None;
    }
    Some((parts[2].parse().ok()?, parts[3].parse().ok()?))
}

pub fn create_nft_id(index: u32, salt: u32) -> String {
    format!("CC-STAMP-{index:05}-{salt}")
}

pub fn traits_from_seed(seed: &str) -> NftInfo {
    collection()
        .iter()
        .find(|item| item.id == seed)
        .cloned()
        .unwrap_or_else(|| {
            let (index, _salt) = parse_nft_id(seed).unwrap_or((1, 0));
            let entry = collection().get(index.saturating_sub(1) as usize).cloned().unwrap_or_else(|| collection()[0].clone());
            NftInfo { id: seed.to_string(), index, ..entry }
        })
}

fn set_pixel(canvas: &mut Canvas, x: isize, y: isize, color: impl Into<String>) {
    if x < 0 || y < 0 || x >= SIZE as isize || y >= SIZE as isize {
        return;
    }
    canvas[y as usize * SIZE + x as usize] = Some(color.into());
}

fn get_pixel(grid: &[Option<String>], x: usize, y: usize) -> Option<&str> {
    grid[y * SIZE + x].as_deref()
}

fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    (r, g, b)
}

fn rgb_to_hex((r, g, b): (u8, u8, u8)) -> String {
    format!("{r:02x}{g:02x}{b:02x}")
}

fn adjust(hex: &str, factor: f32) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    let scale = |value: u8| ((value as f32 * factor).clamp(0.0, 255.0)) as u8;
    rgb_to_hex((scale(r), scale(g), scale(b)))
}

fn rgb_to_hsv((r, g, b): (u8, u8, u8)) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let sat = if max == 0.0 { 0.0 } else { delta / max };
    (hue, sat, max)
}

fn hsv_to_rgb((h, s, v): (f32, f32, f32)) -> (u8, u8, u8) {
    if s == 0.0 {
        let value = (v * 255.0) as u8;
        return (value, value, value);
    }
    let h = h.rem_euclid(360.0) / 60.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    )
}

fn hue_rotate(hex: &str, deg: u32, sat_mul: f32) -> String {
    if deg == 0 {
        return hex.to_string();
    }
    let (h, s, v) = rgb_to_hsv(hex_to_rgb(hex));
    if s < 0.12 {
        return hex.to_string();
    }
    rgb_to_hex(hsv_to_rgb(((h + deg as f32).rem_euclid(360.0), (s * sat_mul).min(1.0), v)))
}

fn decode_rle(hexstr: &str) -> Vec<Option<String>> {
    let bytes = match hex::decode(&hexstr[2..]) {
        Ok(bytes) => bytes,
        Err(_) => return vec![None; SIZE * SIZE],
    };
    if bytes.len() < 5 {
        return vec![None; SIZE * SIZE];
    }
    let top = bytes[1] as usize;
    let right = bytes[2] as usize;
    let left = bytes[4] as usize;
    let mut grid = vec![None; SIZE * SIZE];
    let mut i = 5;
    let mut x = left;
    let mut y = top;
    while i + 1 < bytes.len() {
        let length = bytes[i] as usize;
        let color_idx = bytes[i + 1] as usize;
        i += 2;
        let color = PALETTE.get(color_idx).copied().filter(|value| !value.is_empty());
        for _ in 0..length {
            if x < SIZE && y < SIZE {
                grid[y * SIZE + x] = color.map(str::to_string);
            }
            x += 1;
            if x >= right {
                x = left;
                y += 1;
            }
        }
    }
    grid
}

fn draw_pixels(canvas: &mut Canvas, pixels: &[(isize, isize, String)], outline: bool, clip: bool, outline_color: &str) {
    let translated: Vec<(isize, isize, String)> = pixels
        .iter()
        .filter_map(|(x, y, color)| {
            let x = if clip { *x + EYE_DX } else { *x };
            let y = *y;
            (x >= 0 && y >= 0 && x < SIZE as isize && y < SIZE as isize).then(|| (x, y, color.clone()))
        })
        .collect();

    if outline {
        for (x, y, _) in &translated {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= SIZE as isize || ny >= SIZE as isize {
                        continue;
                    }
                    if translated.iter().all(|(px, py, _)| *px != nx || *py != ny) {
                        set_pixel(canvas, nx, ny, outline_color.to_string());
                    }
                }
            }
        }
    }

    for (x, y, color) in translated {
        set_pixel(canvas, x, y, color);
    }
}

fn stamp_diamond(canvas: &mut Canvas, colors: DiamondColor, cx: isize, top: isize) {
    let ox = cx - (DIAMOND.iter().map(|row| row.len()).max().unwrap_or(0) as isize / 2);
    let mut pixels = Vec::new();
    for (j, row) in DIAMOND.iter().enumerate() {
        for (i, ch) in row.chars().enumerate() {
            let color = match ch {
                'T' => Some(colors.t),
                'M' => Some(colors.m),
                'D' => Some(colors.d),
                'L' => Some(colors.l),
                _ => None,
            };
            if let Some(color) = color {
                pixels.push((ox + i as isize, top + j as isize, color.to_string()));
            }
        }
    }
    draw_pixels(canvas, &pixels, true, false, OUTLINE);
}

fn eye_vr(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    for x in 8..22 {
        for y in 12..15 {
            pixels.push((x, y, c.m.to_string()));
        }
    }
    for x in 8..22 {
        pixels.push((x, 12, c.hi.to_string()));
    }
    for x in (9..21).step_by(3) {
        pixels.push((x, 13, c.gl.to_string()));
    }
    draw_pixels(canvas, &pixels, true, true, c.ol);
}

fn eye_shades(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    for x in 8..14 {
        pixels.push((x, 12, c.m.to_string()));
        pixels.push((x, 13, c.m.to_string()));
    }
    for x in 16..22 {
        pixels.push((x, 12, c.m.to_string()));
        pixels.push((x, 13, c.m.to_string()));
    }
    pixels.extend([
        (8, 11, c.m.to_string()),
        (16, 11, c.m.to_string()),
        (14, 12, c.m.to_string()),
        (15, 12, c.m.to_string()),
        (9, 12, c.gl.to_string()),
        (17, 12, c.gl.to_string()),
    ]);
    draw_pixels(canvas, &pixels, true, true, c.ol);
}

fn eye_round(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    let mut circ = |cx: isize| {
        for (x, y) in [(cx, 11), (cx + 1, 11), (cx - 1, 12), (cx + 2, 12), (cx - 1, 13), (cx + 2, 13), (cx, 14), (cx + 1, 14)] {
            pixels.push((x, y, c.m.to_string()));
        }
        for (x, y) in [(cx, 12), (cx + 1, 12), (cx, 13), (cx + 1, 13)] {
            pixels.push((x, y, c.gl.to_string()));
        }
    };
    circ(9);
    circ(17);
    for x in [14, 15, 16] {
        pixels.push((x, 12, c.m.to_string()));
    }
    draw_pixels(canvas, &pixels, true, true, c.ol);
}

fn eye_3d(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    for x in 8..14 {
        pixels.push((x, 12, "d8344f".to_string()));
        pixels.push((x, 13, "d8344f".to_string()));
    }
    for x in 16..22 {
        pixels.push((x, 12, "3a6ad8".to_string()));
        pixels.push((x, 13, "3a6ad8".to_string()));
    }
    for x in 14..16 {
        pixels.push((x, 12, OUTLINE.to_string()));
    }
    draw_pixels(canvas, &pixels, true, true, c.ol);
}

fn eye_laser(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    for x in -2..34 {
        pixels.push((x, 13, c.m.to_string()));
    }
    for (x, y) in [(10, 13), (11, 13), (18, 13), (19, 13)] {
        pixels.push((x, y, c.gl.to_string()));
    }
    draw_pixels(canvas, &pixels, false, true, OUTLINE);
}

fn eye_cyclops(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    for x in 10..20 {
        pixels.push((x, 12, c.m.to_string()));
        pixels.push((x, 14, c.m.to_string()));
    }
    for x in 13..17 {
        for y in 12..15 {
            pixels.push((x, y, c.m.to_string()));
        }
    }
    pixels.extend([(14, 13, c.gl.to_string()), (15, 13, c.hi.to_string())]);
    draw_pixels(canvas, &pixels, true, true, c.ol);
}

fn eye_pixel8bit(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    for cx in [8, 16] {
        for x in cx..cx + 6 {
            pixels.push((x, 11, c.m.to_string()));
            pixels.push((x, 14, c.m.to_string()));
        }
        for y in 11..15 {
            pixels.push((cx, y, c.m.to_string()));
            pixels.push((cx + 5, y, c.m.to_string()));
        }
        pixels.push((cx + 2, 12, c.gl.to_string()));
        pixels.push((cx + 3, 12, c.hi.to_string()));
    }
    pixels.push((14, 12, c.m.to_string()));
    pixels.push((14, 13, c.m.to_string()));
    draw_pixels(canvas, &pixels, true, true, c.ol);
}

fn eye_scouter(canvas: &mut Canvas, c: EyeColor) {
    let mut pixels = Vec::new();
    for x in 15..21 {
        for y in 11..15 {
            pixels.push((x, y, c.m.to_string()));
        }
    }
    pixels.extend([
        (16, 12, c.gl.to_string()),
        (17, 12, c.hi.to_string()),
        (18, 13, c.gl.to_string()),
    ]);
    for x in 9..15 {
        pixels.push((x, 13, c.m.to_string()));
    }
    draw_pixels(canvas, &pixels, true, true, c.ol);
}

fn draw_eye(canvas: &mut Canvas, eye: &str, color: EyeColor) {
    match eye {
        "vr" => eye_vr(canvas, color),
        "shades" => eye_shades(canvas, color),
        "round" => eye_round(canvas, color),
        "3d" => eye_3d(canvas, color),
        "laser" => eye_laser(canvas, color),
        "cyclops" => eye_cyclops(canvas, color),
        "pixel8bit" => eye_pixel8bit(canvas, color),
        "scouter" => eye_scouter(canvas, color),
        _ => eye_vr(canvas, color),
    }
}

fn render_canvas(info: &NftInfo, with_symbol: bool) -> Canvas {
    let images = &assets().images;
    let body_colors = get_body_colors();
    let eye_colors = get_eye_colors();
    let diamond_colors = get_diamond_colors();

    let body_color = *body_colors.get(info.body_color.as_str()).unwrap_or(&body_colors["ember"]);
    let eye_color = *eye_colors.get(info.eye_color.as_str()).unwrap_or(&eye_colors["cyan"]);
    let diamond_color = *diamond_colors.get(info.rarity.as_str()).unwrap_or(&diamond_colors["cyan"]);

    let mut canvas = vec![None; SIZE * SIZE];

    let body_grid = decode_rle(&images.bodies[info.body_index].data);
    let body_rows: Vec<usize> = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .filter_map(|(x, y)| get_pixel(&body_grid, x, y).map(|_| y))
        .collect();
    let body_bottom = body_rows.iter().copied().max().unwrap_or(SIZE - 1);
    for y in 0..SIZE {
        for x in 0..SIZE {
            if get_pixel(&body_grid, x, y).is_some() {
                let color = if y >= body_bottom.saturating_sub(1) { body_color.d } else { body_color.m };
                set_pixel(&mut canvas, x as isize, y as isize, color.to_string());
            }
        }
    }

    let head = &images.heads[info.head_index];
    let _ = &head.filename;
    let head_grid = decode_rle(&head.data);
    let head_rows: Vec<usize> = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .filter_map(|(x, y)| get_pixel(&head_grid, x, y).map(|_| y))
        .collect();
    if let (Some(y_top), Some(y_bottom)) = (head_rows.iter().copied().min(), head_rows.iter().copied().max()) {
        for y in 0..SIZE {
            for x in 0..SIZE {
                if get_pixel(&head_grid, x, y).is_none() {
                    continue;
                }
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx < 0 || ny < 0 || nx >= SIZE as isize || ny >= SIZE as isize {
                            continue;
                        }
                        let nxu = nx as usize;
                        let nyu = ny as usize;
                        if get_pixel(&head_grid, nxu, nyu).is_none() {
                            set_pixel(&mut canvas, nx, ny, HEAD_OUT.to_string());
                        }
                    }
                }
            }
        }

        for y in 0..SIZE {
            for x in 0..SIZE {
                if let Some(color) = get_pixel(&head_grid, x, y) {
                    let mut color = hue_rotate(color, info.head_hue, 1.08);
                    if y <= y_top + 1 {
                        color = adjust(&color, 1.28);
                    } else if y >= y_bottom {
                        color = adjust(&color, 0.62);
                    } else if y >= y_bottom.saturating_sub(2) {
                        color = adjust(&color, 0.82);
                    }
                    set_pixel(&mut canvas, x as isize, y as isize, color);
                }
            }
        }
    }

    draw_eye(&mut canvas, &info.eye, eye_color);

    if with_symbol {
        stamp_diamond(&mut canvas, diamond_color, 15, 23);
    }

    canvas
}

pub fn render_svg(seed: &str) -> String {
    let info = traits_from_seed(seed);
    let canvas = render_canvas(&info, true);
    let [c1, c2] = info.grad.clone();

    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {SIZE} {SIZE}' shape-rendering='crispEdges'>"
    );
    svg.push_str(&format!(
        "<defs><linearGradient id='g' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#{c1}'/><stop offset='1' stop-color='#{c2}'/></linearGradient></defs>"
    ));
    svg.push_str(&format!("<rect width='{SIZE}' height='{SIZE}' fill='url(#g)'/>"));
    for y in 0..SIZE {
        for x in 0..SIZE {
            if let Some(color) = &canvas[y * SIZE + x] {
                svg.push_str(&format!("<rect x='{x}' y='{y}' width='1' height='1' fill='#{color}'/>"));
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

pub fn render_svg_path(seed: &str) -> anyhow::Result<PathBuf> {
    let output_dir = std::env::temp_dir().join("btcc-litedesk").join("nft");
    fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join(format!("{seed}.svg"));
    if !output_path.exists() {
        fs::write(&output_path, render_svg(seed))?;
    }
    Ok(output_path)
}

pub fn get_nft_range_sorted(
    sort_mode: NftSortMode,
    rarity_filter: Option<&str>,
    start: u32,
    count: u32,
) -> Vec<NftInfo> {
    let start_idx = start.saturating_sub(1) as usize;
    match sort_mode {
        NftSortMode::NumberAsc => collection()
            .iter()
            .filter(|nft| rarity_matches(nft, rarity_filter))
            .skip(start_idx)
            .take(count as usize)
            .cloned()
            .collect(),
        NftSortMode::NumberDesc => collection()
            .iter()
            .rev()
            .filter(|nft| rarity_matches(nft, rarity_filter))
            .skip(start_idx)
            .take(count as usize)
            .cloned()
            .collect(),
        NftSortMode::RarityDesc => rarity_ranking()
            .items
            .iter()
            .filter(|entry| rarity_filter.is_none_or(|value| entry.rarity == value))
            .skip(start_idx)
            .take(count as usize)
            .map(ranked_nft)
            .collect(),
        NftSortMode::RarityAsc => rarity_ranking()
            .items
            .iter()
            .rev()
            .filter(|entry| rarity_filter.is_none_or(|value| entry.rarity == value))
            .skip(start_idx)
            .take(count as usize)
            .map(ranked_nft)
            .collect(),
    }
}

pub fn total_supply_by_rarity(rarity: &str) -> u32 {
    collection()
        .iter()
        .filter(|nft| nft.rarity == rarity)
        .count() as u32
}

pub fn find_sorted_position(
    index: u32,
    sort_mode: NftSortMode,
    rarity_filter: Option<&str>,
) -> Option<u32> {
    match sort_mode {
        NftSortMode::NumberAsc => collection()
            .iter()
            .filter(|nft| rarity_matches(nft, rarity_filter))
            .position(|nft| nft.index == index)
            .map(|position| position as u32),
        NftSortMode::NumberDesc => collection()
            .iter()
            .rev()
            .filter(|nft| rarity_matches(nft, rarity_filter))
            .position(|nft| nft.index == index)
            .map(|position| position as u32),
        NftSortMode::RarityDesc => rarity_ranking()
            .items
            .iter()
            .filter(|entry| rarity_filter.is_none_or(|value| entry.rarity == value))
            .position(|entry| entry.index == index)
            .map(|position| position as u32),
        NftSortMode::RarityAsc => rarity_ranking()
            .items
            .iter()
            .rev()
            .filter(|entry| rarity_filter.is_none_or(|value| entry.rarity == value))
            .position(|entry| entry.index == index)
            .map(|position| position as u32),
    }
}

pub fn rarity_rank(index: u32) -> Option<u32> {
    rarity_ranking()
        .items
        .iter()
        .find(|entry| entry.index == index)
        .map(|entry| entry.rarity_rank)
}

pub fn total_supply() -> u32 {
    TOTAL_SUPPLY_U32
}

mod hex {
    pub fn decode(input: &str) -> Result<Vec<u8>, ()> {
        if input.len() % 2 != 0 {
            return Err(());
        }
        let mut out = Vec::with_capacity(input.len() / 2);
        let bytes = input.as_bytes();
        let value = |b: u8| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            b'A'..=b'F' => Ok(b - b'A' + 10),
            _ => Err(()),
        };
        for chunk in bytes.chunks_exact(2) {
            out.push((value(chunk[0])? << 4) | value(chunk[1])?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn svg_hash(seed: &str) -> String {
        let svg = render_svg(seed);
        let digest = Sha256::digest(svg.as_bytes());
        format!("{digest:x}")
    }

    #[test]
    fn matches_reference_svg_hashes() {
        assert_eq!(
            svg_hash("CC-STAMP-00001-0"),
            "6d958b92b90ec910adae4c4b245c1ab087af63b6445930f3f3548e01a18fd786"
        );
        assert_eq!(
            svg_hash("CC-STAMP-01024-0"),
            "055fc1d9dc6f23c3830545976c895ba140c247070bf1343cb4dbd612990b9229"
        );
        assert_eq!(
            svg_hash("CC-STAMP-21000-0"),
            "4c77b78c0c3034ada85afca2dad3605ea4a7ed98f49f60c99390795c7389ae14"
        );
    }
}
