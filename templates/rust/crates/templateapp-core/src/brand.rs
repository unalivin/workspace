use crate::theme::Color;

pub const BACKGROUND: Color = Color { r: 0x1F, g: 0x3A, b: 0x5F };
pub const FOREGROUND: Color = Color { r: 0xF5, g: 0xF7, b: 0xFA };
pub const ACCENT: Color = Color { r: 0x4F, g: 0xC3, b: 0xA1 };

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub r: f64,
    pub tint: Color,
}

const fn shape(x: f64, y: f64, w: f64, h: f64, r: f64, tint: Color) -> Shape {
    Shape { x, y, w, h, r, tint }
}

pub const MOTIF: &[Shape] = &[
    shape(0.22, 0.26, 0.10, 0.10, 0.05, ACCENT),
    shape(0.40, 0.26, 0.38, 0.10, 0.05, FOREGROUND),
    shape(0.22, 0.45, 0.10, 0.10, 0.05, ACCENT),
    shape(0.40, 0.45, 0.38, 0.10, 0.05, FOREGROUND),
    shape(0.22, 0.64, 0.10, 0.10, 0.05, ACCENT),
    shape(0.40, 0.64, 0.24, 0.10, 0.05, FOREGROUND),
];

pub const SMALL_MOTIF: &[Shape] = &[
    shape(0.19, 0.25, 0.16, 0.19, 0.06, ACCENT),
    shape(0.44, 0.25, 0.37, 0.19, 0.06, FOREGROUND),
    shape(0.19, 0.56, 0.16, 0.19, 0.06, ACCENT),
    shape(0.44, 0.56, 0.37, 0.19, 0.06, FOREGROUND),
];

pub const PLATE: Shape = shape(0.04, 0.04, 0.92, 0.92, 0.20, BACKGROUND);

pub const SMALL_LIMIT: usize = 24;
pub const SIZES: &[usize] = &[16, 24, 32, 48, 64, 128, 256, 512, 1024];
pub const ICO_SIZES: &[usize] = &[16, 32, 48, 256];
pub const ICNS_ENTRIES: &[(&str, usize)] =
    &[("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024)];

const SAMPLES: usize = 4;

pub fn parts(size: usize) -> &'static [Shape] {
    if size <= SMALL_LIMIT { SMALL_MOTIF } else { MOTIF }
}

fn scaled(part: Shape, scale: f64) -> Shape {
    Shape {
        x: part.x * scale,
        y: part.y * scale,
        w: part.w * scale,
        h: part.h * scale,
        r: part.r * scale,
        tint: part.tint,
    }
}

fn coverage(px: f64, py: f64, part: Shape) -> f64 {
    let mut inside = 0;
    for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let x = px + (sx as f64 + 0.5) / SAMPLES as f64;
            let y = py + (sy as f64 + 0.5) / SAMPLES as f64;
            let dx = (part.x + part.r - x).max(x - (part.x + part.w - part.r)).max(0.0);
            let dy = (part.y + part.r - y).max(y - (part.y + part.h - part.r)).max(0.0);
            let within = x >= part.x && x <= part.x + part.w && y >= part.y && y <= part.y + part.h;
            if within && dx * dx + dy * dy <= part.r * part.r {
                inside += 1;
            }
        }
    }
    f64::from(inside) / (SAMPLES * SAMPLES) as f64
}

fn blend(base: Color, top: Color, share: f64) -> Color {
    let mix = |from: u8, to: u8| (f64::from(from) * (1.0 - share) + f64::from(to) * share).round() as u8;
    Color { r: mix(base.r, top.r), g: mix(base.g, top.g), b: mix(base.b, top.b) }
}

pub fn icon(size: usize) -> Vec<u8> {
    let scale = size as f64;
    let plate = scaled(PLATE, scale);
    let motif: Vec<Shape> = parts(size).iter().map(|part| scaled(*part, scale)).collect();
    let mut pixels = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let share = coverage(x as f64, y as f64, plate);
            if share == 0.0 {
                continue;
            }
            let mut pixel = BACKGROUND;
            for part in &motif {
                let inside = coverage(x as f64, y as f64, *part);
                if inside > 0.0 {
                    pixel = blend(pixel, part.tint, inside);
                }
            }
            let at = (y * size + x) * 4;
            pixels[at..at + 4].copy_from_slice(&[pixel.r, pixel.g, pixel.b, (share * 255.0).round() as u8]);
        }
    }
    pixels
}

fn number(value: f64) -> String {
    const UNIT: f64 = 256.0;
    format!("{}", (value * UNIT * 100.0).round() / 100.0)
}

fn drawing(title: &str, shapes: &[Shape]) -> String {
    let mut text = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 256 256\" width=\"256\" height=\"256\" role=\"img\">\n",
    );
    text.push_str(&format!("  <title>{title}</title>\n"));
    for part in std::iter::once(&PLATE).chain(shapes) {
        text.push_str(&format!(
            "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\"/>\n",
            number(part.x),
            number(part.y),
            number(part.w),
            number(part.h),
            number(part.r),
            part.tint.hex()
        ));
    }
    text.push_str("</svg>\n");
    text
}

pub fn svg(title: &str) -> String {
    drawing(title, MOTIF)
}

pub fn small_svg(title: &str) -> String {
    drawing(title, SMALL_MOTIF)
}

pub fn desktop_entry(name: &str, comment: &str, slug: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName={name}\nComment={comment}\nExec={slug}\nIcon={slug}\nTerminal=false\nCategories=Utility;\n"
    )
}

pub fn png(size: usize) -> Vec<u8> {
    crate::png::encode(size, size, &icon(size))
}

pub fn ico() -> Vec<u8> {
    let pictures: Vec<Vec<u8>> = ICO_SIZES.iter().map(|size| png(*size)).collect();
    let mut out = Vec::new();
    for value in [0u16, 1, ICO_SIZES.len() as u16] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    let mut offset = 6 + 16 * ICO_SIZES.len();
    for (size, picture) in ICO_SIZES.iter().zip(&pictures) {
        out.extend_from_slice(&[(size % 256) as u8, (size % 256) as u8, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(picture.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += picture.len();
    }
    for picture in &pictures {
        out.extend_from_slice(picture);
    }
    out
}

pub fn icns() -> Vec<u8> {
    let mut body = Vec::new();
    for (kind, size) in ICNS_ENTRIES {
        let picture = png(*size);
        body.extend_from_slice(kind.as_bytes());
        body.extend_from_slice(&(picture.len() as u32 + 8).to_be_bytes());
        body.extend_from_slice(&picture);
    }
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&(body.len() as u32 + 8).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

pub fn text_files(name: &str, comment: &str, slug: &str) -> Vec<(String, String)> {
    vec![
        ("icon.svg".to_string(), svg(&format!("{name} icon"))),
        ("icon-16.svg".to_string(), small_svg(&format!("{name} icon, simplified for small sizes"))),
        (format!("{slug}.desktop"), desktop_entry(name, comment, slug)),
        (format!("linux/{slug}.svg"), svg(&format!("{name} icon"))),
    ]
}

pub fn files(name: &str, comment: &str, slug: &str) -> Vec<(String, Vec<u8>)> {
    let mut list: Vec<(String, Vec<u8>)> =
        text_files(name, comment, slug).into_iter().map(|(path, text)| (path, text.into_bytes())).collect();
    list.push(("icon.ico".to_string(), ico()));
    list.push(("icon.icns".to_string(), icns()));
    list.push(("icon.png".to_string(), png(512)));
    list.push((format!("linux/{slug}.png"), png(256)));
    for size in SIZES {
        list.push((format!("png/icon-{size}.png"), png(*size)));
    }
    list.sort_by(|first, second| first.0.cmp(&second.0));
    list
}

pub fn write_files(
    directory: &std::path::Path,
    list: &[(String, Vec<u8>)],
    check: bool,
) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for (name, wanted) in list {
        let path = directory.join(name);
        let shown = path.display().to_string();
        if check {
            let current = std::fs::read(&path).map_err(|_| format!("{shown} is missing: run make assets"))?;
            if &current != wanted {
                return Err(format!("{shown} is out of date: run make assets"));
            }
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|problem| format!("{}: {problem}", parent.display()))?;
        }
        std::fs::write(&path, wanted).map_err(|problem| format!("{shown}: {problem}"))?;
        written.push(shown);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta;
    use std::collections::HashSet;

    fn pixel(picture: &[u8], size: usize, x: usize, y: usize) -> [u8; 4] {
        let at = (y * size + x) * 4;
        [picture[at], picture[at + 1], picture[at + 2], picture[at + 3]]
    }

    #[test]
    fn the_icon_has_the_plate_and_the_motif_at_every_size() {
        for size in SIZES.iter().copied().filter(|size| *size <= 256) {
            let picture = icon(size);
            assert_eq!(picture.len(), size * size * 4);
            assert_eq!(pixel(&picture, size, 0, 0)[3], 0, "size {size}: the corner is not transparent");
            assert_eq!(
                pixel(&picture, size, size / 2, size / 8),
                [BACKGROUND.r, BACKGROUND.g, BACKGROUND.b, 255],
                "size {size}: the plate"
            );
            let tints: HashSet<[u8; 3]> = picture.chunks(4).map(|rgba| [rgba[0], rgba[1], rgba[2]]).collect();
            for wanted in [FOREGROUND, ACCENT] {
                assert!(tints.contains(&[wanted.r, wanted.g, wanted.b]), "size {size} lacks {}", wanted.hex());
            }
        }
    }

    #[test]
    fn small_sizes_use_the_simplified_motif() {
        assert!(parts(16).len() < parts(32).len());
        assert_eq!(small_svg("x").matches("<rect").count(), SMALL_MOTIF.len() + 1);
        assert_eq!(svg("x").matches("<rect").count(), MOTIF.len() + 1);
    }

    #[test]
    fn the_svg_sources_are_plain_shapes_with_a_title() {
        for source in [svg("Icon"), small_svg("Icon")] {
            assert!(source.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
            assert!(source.ends_with("</svg>\n"));
            assert!(source.contains("<title>Icon</title>"));
            assert!(!source.contains("<image"));
            assert_eq!(source.matches('<').count(), source.matches('>').count());
        }
        assert!(svg("Icon").contains(
            "<rect x=\"10.24\" y=\"10.24\" width=\"235.52\" height=\"235.52\" rx=\"51.2\" fill=\"#1F3A5F\"/>"
        ));
    }

    fn number(data: &[u8], at: usize, little: bool) -> usize {
        let bytes = [data[at], data[at + 1], data[at + 2], data[at + 3]];
        (if little { u32::from_le_bytes(bytes) } else { u32::from_be_bytes(bytes) }) as usize
    }

    #[test]
    fn the_png_pictures_decode_to_the_drawn_pixels() {
        for size in [16, 24, 32, 48, 64] {
            let (width, height, pixels) = crate::png::decode(&png(size)).unwrap();
            assert_eq!((width, height), (size, size));
            assert_eq!(pixels, icon(size));
        }
        assert!(png(256).len() < 40_000, "{} bytes at 256 pixels", png(256).len());
    }

    #[test]
    fn the_ico_file_lists_valid_pictures() {
        let data = ico();
        assert_eq!(&data[..6], &[0, 0, 1, 0, 4, 0]);
        for (index, size) in ICO_SIZES.iter().enumerate() {
            let entry = &data[6 + 16 * index..];
            let length = number(entry, 8, true);
            let offset = number(entry, 12, true);
            let (width, height, _) = crate::png::decode(&data[offset..offset + length]).unwrap();
            let listed = if entry[0] == 0 { 256 } else { usize::from(entry[0]) };
            assert_eq!((width, height, listed), (*size, *size, *size));
        }
    }

    #[test]
    fn the_icns_file_lists_valid_pictures() {
        let data = icns();
        assert_eq!(&data[..4], b"icns");
        assert_eq!(number(&data, 4, false), data.len());
        let mut found = Vec::new();
        let mut position = 8;
        while position < data.len() {
            let length = number(&data, position + 4, false);
            assert_eq!(&data[position + 8..position + 16], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
            found.push(String::from_utf8_lossy(&data[position..position + 4]).to_string());
            position += length;
        }
        assert_eq!(position, data.len());
        assert_eq!(found, ICNS_ENTRIES.iter().map(|(kind, _)| kind.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn the_generator_writes_and_checks_every_file() {
        let folder = crate::testkit::TempDir::new("brand");
        let list: Vec<(String, Vec<u8>)> =
            vec![("a.txt".to_string(), b"one".to_vec()), ("deep/b.bin".to_string(), vec![0, 1, 2])];
        assert!(write_files(folder.path(), &list, true).unwrap_err().contains("is missing: run make assets"));
        assert_eq!(write_files(folder.path(), &list, false).unwrap().len(), 2);
        assert_eq!(write_files(folder.path(), &list, true).unwrap(), Vec::<String>::new());
        std::fs::write(folder.path().join("deep/b.bin"), [9]).unwrap();
        assert!(write_files(folder.path(), &list, true).unwrap_err().contains("is out of date: run make assets"));
    }

    #[test]
    fn the_committed_assets_match_the_generator() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let files = files(meta::APP_NAME, meta::TAGLINE, meta::SLUG);
        assert_eq!(files.len(), 8 + SIZES.len());
        assert_eq!(write_files(&assets, &files, true), Ok(Vec::new()));
        let files = text_files(meta::APP_NAME, meta::TAGLINE, meta::SLUG);
        let entry = &files[2].1;
        for line in [
            format!("Name={}", meta::APP_NAME),
            format!("Exec={}", meta::SLUG),
            format!("Icon={}", meta::SLUG),
            "Type=Application".to_string(),
        ] {
            assert!(entry.contains(&format!("{line}\n")), "the desktop entry lacks {line}");
        }
    }
}
