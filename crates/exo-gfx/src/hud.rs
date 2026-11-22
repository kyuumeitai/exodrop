use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec4};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],   // Pixel coordinates (0,0 = top-left)
    pub color: [f32; 4], // Linear RGBA
}

#[derive(Default, Clone)]
pub struct HudDrawList {
    pub vertices: Vec<HudVertex>,
}

impl HudDrawList {
    pub fn new() -> Self {
        Self {
            vertices: Vec::with_capacity(4096),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
    }

    #[inline]
    pub fn tri(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Vec4) {
        let col = color.to_array();
        self.vertices.push(HudVertex { pos: a.to_array(), color: col });
        self.vertices.push(HudVertex { pos: b.to_array(), color: col });
        self.vertices.push(HudVertex { pos: c.to_array(), color: col });
    }

    #[inline]
    pub fn quad(&mut self, a: Vec2, b: Vec2, c: Vec2, d: Vec2, color: Vec4) {
        self.tri(a, b, c, color);
        self.tri(a, c, d, color);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Vec4) {
        let a = Vec2::new(x, y);
        let b = Vec2::new(x + w, y);
        let c = Vec2::new(x + w, y + h);
        let d = Vec2::new(x, y + h);
        self.quad(a, b, c, d, color);
    }

    /// Sci-fi slanted parallelogram bar (gives modern 2026 tactical HUD flair!).
    pub fn slanted_rect(&mut self, x: f32, y: f32, w: f32, h: f32, skew: f32, color: Vec4) {
        let a = Vec2::new(x + skew, y);
        let b = Vec2::new(x + w + skew, y);
        let c = Vec2::new(x + w, y + h);
        let d = Vec2::new(x, y + h);
        self.quad(a, b, c, d, color);
    }

    pub fn rect_outline(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, color: Vec4) {
        self.rect(x, y, w, t, color);
        self.rect(x, y + h - t, w, t, color);
        self.rect(x, y, t, h, color);
        self.rect(x + w - t, y, t, h, color);
    }

    pub fn line(&mut self, p0: Vec2, p1: Vec2, thickness: f32, color: Vec4) {
        let d = p1 - p0;
        let len = d.length();
        if len < 0.001 {
            return;
        }
        let n = Vec2::new(-d.y, d.x) / len * (thickness * 0.5);
        self.quad(p0 - n, p1 - n, p1 + n, p0 + n, color);
    }

    pub fn circle_filled(&mut self, center: Vec2, radius: f32, segs: usize, color: Vec4) {
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let p0 = center + Vec2::new(a0.cos(), a0.sin()) * radius;
            let p1 = center + Vec2::new(a1.cos(), a1.sin()) * radius;
            self.tri(center, p0, p1, color);
        }
    }

    pub fn circle_outline(
        &mut self,
        center: Vec2,
        radius: f32,
        thickness: f32,
        segs: usize,
        color: Vec4,
    ) {
        let r_in = (radius - thickness * 0.5).max(0.0);
        let r_out = radius + thickness * 0.5;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let d0 = Vec2::new(a0.cos(), a0.sin());
            let d1 = Vec2::new(a1.cos(), a1.sin());
            self.quad(
                center + d0 * r_in,
                center + d0 * r_out,
                center + d1 * r_out,
                center + d1 * r_in,
                color,
            );
        }
    }

    /// Tactical diamond lock-on bracket.
    pub fn diamond(&mut self, c: Vec2, r: f32, thickness: f32, color: Vec4) {
        let top = c + Vec2::new(0.0, -r);
        let right = c + Vec2::new(r, 0.0);
        let bot = c + Vec2::new(0.0, r);
        let left = c + Vec2::new(-r, 0.0);
        self.line(top, right, thickness, color);
        self.line(right, bot, thickness, color);
        self.line(bot, left, thickness, color);
        self.line(left, top, thickness, color);
    }

    /// Corner targeting brackets around a box.
    pub fn corner_brackets(&mut self, c: Vec2, half: Vec2, arm: f32, t: f32, color: Vec4) {
        let l = c.x - half.x;
        let r = c.x + half.x;
        let top = c.y - half.y;
        let bot = c.y + half.y;
        // Top-left
        self.rect(l, top, arm, t, color);
        self.rect(l, top, t, arm, color);
        // Top-right
        self.rect(r - arm, top, arm, t, color);
        self.rect(r - t, top, t, arm, color);
        // Bottom-left
        self.rect(l, bot - t, arm, t, color);
        self.rect(l, bot - arm, t, arm, color);
        // Bottom-right
        self.rect(r - arm, bot - t, arm, t, color);
        self.rect(r - t, bot - arm, t, arm, color);
    }

    /// Crisp built-in 5x7 geometric sci-fi bitmap font renderer.
    pub fn text(&mut self, text: &str, x: f32, y: f32, scale: f32, color: Vec4) {
        let mut cursor_x = x;
        let step = 6.0 * scale;
        for ch in text.chars() {
            let uc = ch.to_ascii_uppercase();
            if uc == '\n' {
                continue;
            }
            if let Some(rows) = glyph_5x7(uc) {
                for (ry, &bits) in rows.iter().enumerate() {
                    let mut rx = 0usize;
                    while rx < 5 {
                        if (bits & (1 << (4 - rx))) != 0 {
                            let start = rx;
                            while rx < 5 && (bits & (1 << (4 - rx))) != 0 {
                                rx += 1;
                            }
                            self.rect(
                                cursor_x + start as f32 * scale,
                                y + ry as f32 * scale,
                                (rx - start) as f32 * scale,
                                scale,
                                color,
                            );
                        } else {
                            rx += 1;
                        }
                    }
                }
            }
            cursor_x += step;
        }
    }

    pub fn text_width(text: &str, scale: f32) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        (text.chars().count() as f32 * 6.0 - 1.0) * scale
    }

    pub fn text_centered(&mut self, text: &str, cx: f32, y: f32, scale: f32, color: Vec4) {
        let w = Self::text_width(text, scale);
        self.text(text, cx - w * 0.5, y, scale, color);
    }

    pub fn text_right(&mut self, text: &str, rx: f32, y: f32, scale: f32, color: Vec4) {
        let w = Self::text_width(text, scale);
        self.text(text, rx - w, y, scale, color);
    }

    /// Text with subtle dark drop-shadow for readability over bright sky/lava.
    pub fn text_shadow(&mut self, text: &str, x: f32, y: f32, scale: f32, color: Vec4) {
        let shadow = Vec4::new(0.01, 0.02, 0.04, color.w * 0.85);
        self.text(text, x + scale * 0.7, y + scale * 0.7, scale, shadow);
        self.text(text, x, y, scale, color);
    }

    pub fn text_centered_shadow(&mut self, text: &str, cx: f32, y: f32, scale: f32, color: Vec4) {
        let w = Self::text_width(text, scale);
        self.text_shadow(text, cx - w * 0.5, y, scale, color);
    }
}

fn glyph_5x7(c: char) -> Option<[u8; 7]> {
    Some(match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '%' => [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
        ']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        '|' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        '&' => [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101],
        '=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
        ' ' => return None,
        _ => return None,
    })
}
