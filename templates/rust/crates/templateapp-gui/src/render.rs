use std::collections::HashMap;

use egui::epaint::{ClippedPrimitive, ImageDelta, Mesh, Primitive, TextureId, Vertex, textures::TextureFilter};
use egui::{Color32, ColorImage, Context, Event, ImageData, Pos2, RawInput, Rect, TexturesDelta, Ui, pos2, vec2};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Picture {
    pub fn new(width: usize, height: usize) -> Picture {
        Picture { width, height, pixels: vec![0; width * height * 4] }
    }

    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let at = (y * self.width + x) * 4;
        [self.pixels[at], self.pixels[at + 1], self.pixels[at + 2], self.pixels[at + 3]]
    }

    fn blend(&mut self, x: usize, y: usize, source: [f32; 4]) {
        let at = (y * self.width + x) * 4;
        let keep = 1.0 - source[3] / 255.0;
        for (channel, value) in source.iter().enumerate() {
            let mixed = value + f32::from(self.pixels[at + channel]) * keep;
            self.pixels[at + channel] = mixed.round().clamp(0.0, 255.0) as u8;
        }
    }
}

struct Texture {
    image: ColorImage,
    smooth: bool,
}

#[derive(Default)]
pub struct Textures {
    known: HashMap<TextureId, Texture>,
}

impl Textures {
    pub fn update(&mut self, delta: &TexturesDelta) {
        for (id, changes) in &delta.set {
            for change in changes {
                self.set(*id, change);
            }
        }
        for id in &delta.free {
            self.known.remove(id);
        }
    }

    fn set(&mut self, id: TextureId, change: &ImageDelta) {
        let ImageData::Color(image) = &change.image;
        let smooth = change.options.magnification == TextureFilter::Linear;
        match (change.pos, self.known.get_mut(&id)) {
            (Some([left, top]), Some(texture)) => {
                for y in 0..image.size[1] {
                    for x in 0..image.size[0] {
                        if left + x < texture.image.size[0] && top + y < texture.image.size[1] {
                            texture.image.pixels[(top + y) * texture.image.size[0] + left + x] =
                                image.pixels[y * image.size[0] + x];
                        }
                    }
                }
            }
            _ => {
                self.known.insert(id, Texture { image: ColorImage::clone(image), smooth });
            }
        }
    }

    fn sample(&self, id: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some(texture) = self.known.get(&id) else {
            return [255.0; 4];
        };
        let [width, height] = texture.image.size;
        if width == 0 || height == 0 {
            return [255.0; 4];
        }
        let texel = |x: isize, y: isize| -> [f32; 4] {
            let column = x.clamp(0, width as isize - 1) as usize;
            let row = y.clamp(0, height as isize - 1) as usize;
            texture.image.pixels[row * width + column].to_array().map(f32::from)
        };
        let x = u * width as f32 - 0.5;
        let y = v * height as f32 - 0.5;
        if !texture.smooth {
            return texel(x.round() as isize, y.round() as isize);
        }
        let (left, top) = (x.floor(), y.floor());
        let (across, down) = (x - left, y - top);
        let corners = [
            (texel(left as isize, top as isize), (1.0 - across) * (1.0 - down)),
            (texel(left as isize + 1, top as isize), across * (1.0 - down)),
            (texel(left as isize, top as isize + 1), (1.0 - across) * down),
            (texel(left as isize + 1, top as isize + 1), across * down),
        ];
        let mut mixed = [0.0; 4];
        for (color, weight) in corners {
            for channel in 0..4 {
                mixed[channel] += color[channel] * weight;
            }
        }
        mixed
    }
}

fn edge(from: Pos2, to: Pos2, x: f32, y: f32) -> f32 {
    (to.x - from.x) * (y - from.y) - (to.y - from.y) * (x - from.x)
}

fn owns_boundary(from: Pos2, to: Pos2) -> bool {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

fn triangle(picture: &mut Picture, textures: &Textures, id: TextureId, clip: Rect, corners: [Vertex; 3], scale: f32) {
    let mut corners = corners;
    let place = |vertex: &Vertex| pos2(vertex.pos.x * scale, vertex.pos.y * scale);
    let mut area = edge(place(&corners[0]), place(&corners[1]), place(&corners[2]).x, place(&corners[2]).y);
    if area == 0.0 {
        return;
    }
    if area < 0.0 {
        corners.swap(1, 2);
        area = -area;
    }
    let [a, b, c] = [place(&corners[0]), place(&corners[1]), place(&corners[2])];
    let left = a.x.min(b.x).min(c.x).max(clip.left()).max(0.0).floor() as usize;
    let top = a.y.min(b.y).min(c.y).max(clip.top()).max(0.0).floor() as usize;
    let right = (a.x.max(b.x).max(c.x).min(clip.right()).ceil().max(0.0) as usize).min(picture.width);
    let bottom = (a.y.max(b.y).max(c.y).min(clip.bottom()).ceil().max(0.0) as usize).min(picture.height);
    let colors = corners.map(|vertex| vertex.color.to_array().map(f32::from));
    let borders = [(b, c), (c, a), (a, b)];
    for y in top..bottom {
        for x in left..right {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            if px < clip.left() || px >= clip.right() || py < clip.top() || py >= clip.bottom() {
                continue;
            }
            let weights = borders.map(|(from, to)| edge(from, to, px, py));
            let inside = weights
                .iter()
                .zip(borders)
                .all(|(weight, (from, to))| *weight > 0.0 || (*weight == 0.0 && owns_boundary(from, to)));
            if !inside {
                continue;
            }
            let shares = weights.map(|weight| weight / area);
            let u = shares[0] * corners[0].uv.x + shares[1] * corners[1].uv.x + shares[2] * corners[2].uv.x;
            let v = shares[0] * corners[0].uv.y + shares[1] * corners[1].uv.y + shares[2] * corners[2].uv.y;
            let texel = textures.sample(id, u, v);
            let mut source = [0.0; 4];
            for channel in 0..4 {
                let tint =
                    shares[0] * colors[0][channel] + shares[1] * colors[1][channel] + shares[2] * colors[2][channel];
                source[channel] = tint * texel[channel] / 255.0;
            }
            picture.blend(x, y, source);
        }
    }
}

fn mesh(picture: &mut Picture, textures: &Textures, clip: Rect, shape: &Mesh, scale: f32) {
    for corner in shape.indices.as_chunks::<3>().0 {
        let picked = corner.map(|index| shape.vertices.get(index as usize).copied());
        if let [Some(first), Some(second), Some(third)] = picked {
            triangle(picture, textures, shape.texture_id, clip, [first, second, third], scale);
        }
    }
}

pub fn paint(picture: &mut Picture, textures: &Textures, primitives: &[ClippedPrimitive], scale: f32) {
    for clipped in primitives {
        if let Primitive::Mesh(shape) = &clipped.primitive {
            let clip = Rect::from_min_max(
                pos2(clipped.clip_rect.min.x * scale, clipped.clip_rect.min.y * scale),
                pos2(clipped.clip_rect.max.x * scale, clipped.clip_rect.max.y * scale),
            );
            mesh(picture, textures, clip, shape, scale);
        }
    }
}

pub struct Stage {
    pub ctx: Context,
    textures: Textures,
    width: f32,
    height: f32,
    scale: f32,
    time: f64,
    fill: Color32,
}

impl Stage {
    pub fn new(width: f32, height: f32, scale: f32) -> Stage {
        let ctx = Context::default();
        ctx.set_pixels_per_point(scale);
        Stage { ctx, textures: Textures::default(), width, height, scale, time: 0.0, fill: Color32::BLACK }
    }

    pub fn frame(&mut self, events: Vec<Event>, mut draw: impl FnMut(&mut Ui)) -> Picture {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(self.width, self.height))),
            time: Some(self.time),
            predicted_dt: 0.25,
            focused: true,
            events,
            ..RawInput::default()
        };
        self.time += 0.25;
        let mut output = self.ctx.run_ui(input, |ui| draw(ui));
        self.textures.update(&output.textures_delta);
        output.textures_delta.clear();
        self.fill = self.ctx.global_style().visuals.panel_fill;
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let mut picture =
            Picture::new((self.width * self.scale).round() as usize, (self.height * self.scale).round() as usize);
        let fill = self.fill.to_array();
        for pixel in picture.pixels.as_chunks_mut::<4>().0.iter_mut() {
            *pixel = fill;
        }
        paint(&mut picture, &self.textures, &primitives, output.pixels_per_point);
        picture
    }

    pub fn settle(&mut self, frames: usize, mut draw: impl FnMut(&mut Ui)) -> Picture {
        let mut picture = self.frame(Vec::new(), &mut draw);
        for _ in 1..frames {
            picture = self.frame(Vec::new(), &mut draw);
        }
        picture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{CornerRadius, Frame, Sense};

    fn square(color: Color32) -> impl FnMut(&mut Ui) {
        move |ui: &mut Ui| {
            Frame::NONE.fill(Color32::from_rgb(10, 20, 30)).show(ui, |ui| {
                let (area, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                ui.painter().rect_filled(area.shrink(10.0), CornerRadius::ZERO, color);
                ui.allocate_space(ui.available_size());
            });
        }
    }

    #[test]
    fn a_filled_rectangle_lands_on_the_expected_pixels() {
        let mut stage = Stage::new(80.0, 60.0, 1.0);
        let picture = stage.settle(2, square(Color32::from_rgb(200, 100, 50)));
        assert_eq!((picture.width, picture.height), (80, 60));
        assert_eq!(picture.pixel(20, 20), [200, 100, 50, 255]);
        assert_eq!(picture.pixel(5, 5), [10, 20, 30, 255]);
        assert_eq!(picture.pixel(70, 50), [10, 20, 30, 255]);
    }

    #[test]
    fn a_translucent_fill_mixes_with_the_background_once() {
        let mut stage = Stage::new(80.0, 60.0, 1.0);
        let picture = stage.settle(2, square(Color32::from_rgba_unmultiplied(255, 255, 255, 128)));
        let [red, green, blue, alpha] = picture.pixel(20, 20);
        assert_eq!(alpha, 255);
        for (mixed, base) in [(red, 10.0), (green, 20.0), (blue, 30.0)] {
            let wanted = 128.0 + base * (1.0 - 128.0 / 255.0);
            assert!((f32::from(mixed) - wanted).abs() <= 2.0, "{mixed} is not close to {wanted}");
        }
    }

    #[test]
    fn the_scale_multiplies_the_picture_and_text_leaves_ink() {
        let mut stage = Stage::new(120.0, 40.0, 2.0);
        let picture = stage.settle(2, |ui: &mut Ui| {
            Frame::NONE.fill(Color32::WHITE).show(ui, |ui| {
                ui.colored_label(Color32::BLACK, "Records");
                ui.allocate_space(ui.available_size());
            });
        });
        assert_eq!((picture.width, picture.height), (240, 80));
        let dark = picture.pixels.as_chunks::<4>().0.iter().filter(|pixel| pixel[0] < 96).count();
        assert!(dark > 40, "only {dark} dark pixels");
        assert_eq!(picture.pixel(230, 70), [255, 255, 255, 255]);
    }

    #[test]
    fn shared_edges_are_painted_once() {
        let mut picture = Picture::new(8, 8);
        let textures = Textures::default();
        let color = Color32::from_rgba_premultiplied(100, 0, 0, 100);
        let corner = |x: f32, y: f32| Vertex { pos: pos2(x, y), uv: pos2(0.0, 0.0), color };
        let shape = Mesh {
            vertices: vec![corner(0.5, 0.5), corner(6.5, 0.5), corner(6.5, 6.5), corner(0.5, 6.5)],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..Mesh::default()
        };
        mesh(&mut picture, &textures, Rect::from_min_max(pos2(0.0, 0.0), pos2(8.0, 8.0)), &shape, 1.0);
        for at in 0..6 {
            assert_eq!(picture.pixel(at, at), [100, 0, 0, 100], "diagonal pixel {at}");
        }
        assert_eq!(picture.pixel(3, 1), [100, 0, 0, 100]);
        assert_eq!(picture.pixel(7, 7), [0, 0, 0, 0]);
    }
}
