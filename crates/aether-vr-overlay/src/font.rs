//! Bitmap font renderer for debug overlay text.
//!
//! Provides a minimal 5x7 monospace font that renders to RGBA pixel buffers.
//! Extracted from `aether-vr-emulator` for cross-platform reuse.

pub const GLYPH_WIDTH: usize = 5;
pub const GLYPH_HEIGHT: usize = 7;
pub const GLYPH_SPACING: usize = 6;
pub const LINE_HEIGHT: usize = 16;

/// Get a 7-row bitmap for a character (5 bits per row, MSB on left).
pub fn char_bitmap(ch: char) -> [u8; 7] {
    match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'A' | 'a' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' | 'b' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' | 'c' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' | 'd' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' | 'e' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' | 'f' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' | 'g' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' | 'h' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' | 'i' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' | 'j' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' | 'k' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' | 'l' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' | 'm' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' | 'n' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' | 'o' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' | 'p' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' | 'q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' | 'r' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' | 's' => [
            0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
        ],
        'T' | 't' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' | 'u' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' | 'v' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' | 'w' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' | 'x' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' | 'y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' | 'z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        ' ' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100,
        ],
        ':' => [
            0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000,
        ],
        '#' => [
            0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '=' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        '%' => [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
        _ => [
            0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
        ],
    }
}

/// Parameters for drawing text onto an RGBA buffer.
pub struct DrawParams<'a> {
    pub buffer: &'a mut [u8],
    pub width: usize,
    pub height: usize,
    pub color: [u8; 4],
    pub scale: usize,
}

/// Draw a single character onto an RGBA buffer at the given position.
pub fn draw_char_rgba(params: &mut DrawParams<'_>, x: i32, y: i32, ch: char) {
    let bitmap = char_bitmap(ch);
    for (row, &bits) in bitmap.iter().enumerate() {
        for col in 0..GLYPH_WIDTH {
            if bits & (1 << (4 - col)) != 0 {
                for sy in 0..params.scale {
                    for sx in 0..params.scale {
                        let px = x + (col * params.scale + sx) as i32;
                        let py = y + (row * params.scale + sy) as i32;
                        if px >= 0
                            && px < params.width as i32
                            && py >= 0
                            && py < params.height as i32
                        {
                            let idx = (py as usize * params.width + px as usize) * 4;
                            params.buffer[idx] = params.color[0];
                            params.buffer[idx + 1] = params.color[1];
                            params.buffer[idx + 2] = params.color[2];
                            params.buffer[idx + 3] = params.color[3];
                        }
                    }
                }
            }
        }
    }
}

/// Draw a text string onto an RGBA buffer at the given position.
pub fn draw_text_rgba(params: &mut DrawParams<'_>, text: &str, x: i32, y: i32) {
    let spacing = GLYPH_SPACING * params.scale;
    let mut cx = x;
    for ch in text.chars() {
        draw_char_rgba(params, cx, y, ch);
        cx += spacing as i32;
    }
}

/// Measure the pixel dimensions of a text string at a given scale.
pub fn measure_text(text: &str, scale: usize) -> (usize, usize) {
    if text.is_empty() {
        return (0, 0);
    }
    let width = text.len() * GLYPH_SPACING * scale - scale; // subtract trailing gap
    let height = GLYPH_HEIGHT * scale;
    (width, height)
}

/// Draw a single character onto a u32 (0xRRGGBB) framebuffer. Used by the emulator.
pub fn draw_char_u32(
    pixels: &mut [u32],
    fb_width: usize,
    fb_height: usize,
    x: i32,
    y: i32,
    ch: char,
    color: u32,
) {
    let bitmap = char_bitmap(ch);
    for (row, &bits) in bitmap.iter().enumerate() {
        for col in 0..GLYPH_WIDTH {
            if bits & (1 << (4 - col)) != 0 {
                let px = x + col as i32;
                let py = y + row as i32;
                if px >= 0 && px < fb_width as i32 && py >= 0 && py < fb_height as i32 {
                    pixels[py as usize * fb_width + px as usize] = color;
                }
            }
        }
    }
}

/// Draw text onto a u32 (0xRRGGBB) framebuffer. Used by the emulator.
pub fn draw_text_u32(
    pixels: &mut [u32],
    fb_width: usize,
    fb_height: usize,
    text: &str,
    x: i32,
    y: i32,
    color: u32,
) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char_u32(pixels, fb_width, fb_height, cx, y, ch, color);
        cx += GLYPH_SPACING as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_bitmap_space_is_empty() {
        assert!(char_bitmap(' ').iter().all(|&r| r == 0));
    }

    #[test]
    fn char_bitmap_all_digits_nonempty() {
        for d in '0'..='9' {
            assert!(char_bitmap(d).iter().any(|&r| r != 0), "digit {d} empty");
        }
    }

    #[test]
    fn char_bitmap_all_letters_nonempty() {
        for ch in 'A'..='Z' {
            assert!(char_bitmap(ch).iter().any(|&r| r != 0), "letter {ch} empty");
        }
    }

    #[test]
    fn char_bitmap_lowercase_same_as_uppercase() {
        for ch in 'A'..='Z' {
            let lower = char::from(ch as u8 + 32);
            assert_eq!(char_bitmap(ch), char_bitmap(lower));
        }
    }

    #[test]
    fn char_bitmap_unknown_returns_box() {
        let bm = char_bitmap('\u{FFFF}');
        assert!(bm.iter().any(|&r| r != 0));
    }

    #[test]
    fn char_bitmap_punctuation_nonempty() {
        for ch in ['.', ':', '-', '(', ')', ',', '#', '/', '+', '=', '%'] {
            assert!(char_bitmap(ch).iter().any(|&r| r != 0), "punct {ch} empty");
        }
    }

    #[test]
    fn draw_char_rgba_renders_pixels() {
        let mut buf = vec![0u8; 20 * 20 * 4];
        let mut p = DrawParams {
            buffer: &mut buf,
            width: 20,
            height: 20,
            color: [255, 0, 0, 255],
            scale: 1,
        };
        draw_char_rgba(&mut p, 2, 2, 'A');
        assert!(buf.iter().any(|&b| b == 255));
    }

    #[test]
    fn draw_char_rgba_with_scale() {
        let mut buf1 = vec![0u8; 40 * 40 * 4];
        let mut buf2 = vec![0u8; 40 * 40 * 4];
        {
            let mut p = DrawParams {
                buffer: &mut buf1,
                width: 40,
                height: 40,
                color: [255, 255, 255, 255],
                scale: 1,
            };
            draw_char_rgba(&mut p, 0, 0, 'X');
        }
        {
            let mut p = DrawParams {
                buffer: &mut buf2,
                width: 40,
                height: 40,
                color: [255, 255, 255, 255],
                scale: 2,
            };
            draw_char_rgba(&mut p, 0, 0, 'X');
        }
        let count1 = buf1.chunks(4).filter(|p| p[0] == 255).count();
        let count2 = buf2.chunks(4).filter(|p| p[0] == 255).count();
        assert!(count2 > count1 * 3);
    }

    #[test]
    fn draw_char_rgba_out_of_bounds_no_panic() {
        let mut buf = vec![0u8; 10 * 10 * 4];
        {
            let mut p = DrawParams {
                buffer: &mut buf,
                width: 10,
                height: 10,
                color: [255, 0, 0, 255],
                scale: 1,
            };
            draw_char_rgba(&mut p, -5, -5, 'A');
            draw_char_rgba(&mut p, 100, 100, 'A');
        }
    }

    #[test]
    fn draw_text_rgba_renders_multiple_chars() {
        let mut buf = vec![0u8; 100 * 20 * 4];
        let mut p = DrawParams {
            buffer: &mut buf,
            width: 100,
            height: 20,
            color: [0, 255, 0, 255],
            scale: 1,
        };
        draw_text_rgba(&mut p, "Hi", 0, 0);
        drop(p);
        let green_count = buf.chunks(4).filter(|p| p[1] == 255).count();
        assert!(green_count > 5);
    }

    #[test]
    fn draw_text_rgba_empty_no_change() {
        let mut buf = vec![0u8; 40 * 20 * 4];
        let before = buf.clone();
        let mut p = DrawParams {
            buffer: &mut buf,
            width: 40,
            height: 20,
            color: [255, 255, 255, 255],
            scale: 1,
        };
        draw_text_rgba(&mut p, "", 0, 0);
        drop(p);
        assert_eq!(buf, before);
    }

    #[test]
    fn measure_text_empty() {
        assert_eq!(measure_text("", 1), (0, 0));
    }

    #[test]
    fn measure_text_single_char() {
        let (w, h) = measure_text("A", 1);
        assert_eq!(w, 5); // 6 - 1 trailing gap
        assert_eq!(h, 7);
    }

    #[test]
    fn measure_text_multiple_chars() {
        let (w, _) = measure_text("AB", 1);
        assert_eq!(w, 11); // 6 + 6 - 1
    }

    #[test]
    fn measure_text_with_scale() {
        let (w, h) = measure_text("A", 2);
        assert_eq!(w, 10); // (6*2 - 2)
        assert_eq!(h, 14); // 7*2
    }

    #[test]
    fn draw_char_u32_renders() {
        let mut pixels = vec![0u32; 20 * 20];
        draw_char_u32(&mut pixels, 20, 20, 2, 2, 'A', 0x00ff00);
        assert!(pixels.iter().any(|&p| p == 0x00ff00));
    }

    #[test]
    fn draw_text_u32_renders() {
        let mut pixels = vec![0u32; 100 * 20];
        draw_text_u32(&mut pixels, 100, 20, "Hi", 0, 0, 0xff0000);
        assert!(pixels.iter().any(|&p| p == 0xff0000));
    }
}
