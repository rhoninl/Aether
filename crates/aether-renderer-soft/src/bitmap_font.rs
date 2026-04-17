//! Baked 5x7 bitmap font for ASCII 32..126.
//!
//! Each glyph is 7 rows of 5 bits; bit 4 is leftmost pixel, bit 0 is rightmost.
//! Unsupported codepoints render as blanks. Pixel format: ARGB u32.

pub const GLYPH_WIDTH: u32 = 5;
pub const GLYPH_HEIGHT: u32 = 7;
pub const GLYPH_SPACING_X: u32 = 1;
const FIRST_CHAR: u8 = 32;
const LAST_CHAR: u8 = 126;
const GLYPH_COUNT: usize = (LAST_CHAR - FIRST_CHAR + 1) as usize;

/// 7 rows, 5 bits each. Bit layout per row: b4 b3 b2 b1 b0 (left to right).
/// Legibility over beauty. Only letters/digits/common punctuation are drawn;
/// the rest are blank 5x7 placeholders that still consume a glyph slot.
const GLYPHS: [[u8; GLYPH_HEIGHT as usize]; GLYPH_COUNT] = [
    // 32 ' '
    [0, 0, 0, 0, 0, 0, 0],
    // 33 '!'
    [0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100, 0],
    // 34 '"'
    [0b01010, 0b01010, 0, 0, 0, 0, 0],
    // 35 '#'
    [0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0, 0],
    // 36 '$'
    [
        0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100,
    ],
    // 37 '%'
    [0b11001, 0b11010, 0b00100, 0b01011, 0b10011, 0, 0],
    // 38 '&'
    [0b01100, 0b10010, 0b01100, 0b10101, 0b10010, 0b01101, 0],
    // 39 '\''
    [0b00100, 0b00100, 0, 0, 0, 0, 0],
    // 40 '('
    [
        0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
    ],
    // 41 ')'
    [
        0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
    ],
    // 42 '*'
    [0, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0],
    // 43 '+'
    [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
    // 44 ','
    [0, 0, 0, 0, 0, 0b00100, 0b01000],
    // 45 '-'
    [0, 0, 0, 0b11111, 0, 0, 0],
    // 46 '.'
    [0, 0, 0, 0, 0, 0b00100, 0],
    // 47 '/'
    [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0, 0],
    // 48 '0'
    [
        0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
    ],
    // 49 '1'
    [
        0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
    ],
    // 50 '2'
    [
        0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
    ],
    // 51 '3'
    [
        0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
    ],
    // 52 '4'
    [
        0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
    ],
    // 53 '5'
    [
        0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
    ],
    // 54 '6'
    [
        0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
    ],
    // 55 '7'
    [
        0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
    ],
    // 56 '8'
    [
        0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
    ],
    // 57 '9'
    [
        0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
    ],
    // 58 ':'
    [0, 0b00100, 0, 0, 0, 0b00100, 0],
    // 59 ';'
    [0, 0b00100, 0, 0, 0, 0b00100, 0b01000],
    // 60 '<'
    [
        0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
    ],
    // 61 '='
    [0, 0, 0b11111, 0, 0b11111, 0, 0],
    // 62 '>'
    [
        0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
    ],
    // 63 '?'
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
    // 64 '@'
    [
        0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110,
    ],
    // 65 'A'
    [
        0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
    ],
    // 66 'B'
    [
        0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
    ],
    // 67 'C'
    [
        0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
    ],
    // 68 'D'
    [
        0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
    ],
    // 69 'E'
    [
        0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
    ],
    // 70 'F'
    [
        0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
    ],
    // 71 'G'
    [
        0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
    ],
    // 72 'H'
    [
        0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
    ],
    // 73 'I'
    [
        0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
    ],
    // 74 'J'
    [
        0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
    ],
    // 75 'K'
    [
        0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
    ],
    // 76 'L'
    [
        0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
    ],
    // 77 'M'
    [
        0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
    ],
    // 78 'N'
    [
        0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001,
    ],
    // 79 'O'
    [
        0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
    ],
    // 80 'P'
    [
        0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
    ],
    // 81 'Q'
    [
        0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
    ],
    // 82 'R'
    [
        0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
    ],
    // 83 'S'
    [
        0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
    ],
    // 84 'T'
    [
        0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
    ],
    // 85 'U'
    [
        0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
    ],
    // 86 'V'
    [
        0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
    ],
    // 87 'W'
    [
        0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
    ],
    // 88 'X'
    [
        0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
    ],
    // 89 'Y'
    [
        0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100,
    ],
    // 90 'Z'
    [
        0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
    ],
    // 91 '['
    [
        0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
    ],
    // 92 '\\'
    [0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0, 0],
    // 93 ']'
    [
        0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
    ],
    // 94 '^'
    [0b00100, 0b01010, 0b10001, 0, 0, 0, 0],
    // 95 '_'
    [0, 0, 0, 0, 0, 0, 0b11111],
    // 96 '`'
    [0b01000, 0b00100, 0, 0, 0, 0, 0],
    // 97 'a'
    [0, 0, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
    // 98 'b'
    [
        0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110,
    ],
    // 99 'c'
    [0, 0, 0b01110, 0b10001, 0b10000, 0b10001, 0b01110],
    // 100 'd'
    [
        0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111,
    ],
    // 101 'e'
    [0, 0, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
    // 102 'f'
    [
        0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000,
    ],
    // 103 'g'
    [0, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    // 104 'h'
    [
        0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001,
    ],
    // 105 'i'
    [0b00100, 0, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 106 'j'
    [0b00010, 0, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100],
    // 107 'k'
    [
        0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010,
    ],
    // 108 'l'
    [
        0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
    ],
    // 109 'm'
    [0, 0, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001],
    // 110 'n'
    [0, 0, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
    // 111 'o'
    [0, 0, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
    // 112 'p'
    [0, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000],
    // 113 'q'
    [0, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001],
    // 114 'r'
    [0, 0, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
    // 115 's'
    [0, 0, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
    // 116 't'
    [
        0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110,
    ],
    // 117 'u'
    [0, 0, 0b10001, 0b10001, 0b10001, 0b10001, 0b01111],
    // 118 'v'
    [0, 0, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    // 119 'w'
    [0, 0, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
    // 120 'x'
    [0, 0, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
    // 121 'y'
    [0, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    // 122 'z'
    [0, 0, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
    // 123 '{'
    [
        0b00010, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00010,
    ],
    // 124 '|'
    [
        0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
    ],
    // 125 '}'
    [
        0b01000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01000,
    ],
    // 126 '~'
    [0b01001, 0b10101, 0b10010, 0, 0, 0, 0],
];

fn glyph_for(ch: char) -> Option<&'static [u8; GLYPH_HEIGHT as usize]> {
    let code = ch as u32;
    if code < FIRST_CHAR as u32 || code > LAST_CHAR as u32 {
        return None;
    }
    Some(&GLYPHS[(code - FIRST_CHAR as u32) as usize])
}

pub fn draw_text(fb: &mut [u32], fb_w: u32, fb_h: u32, x: i32, y: i32, text: &str, color: u32) {
    draw_text_scaled(fb, fb_w, fb_h, x, y, text, color, 1);
}

pub fn draw_text_scaled(
    fb: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    x: i32,
    y: i32,
    text: &str,
    color: u32,
    scale: u32,
) {
    if fb.len() != (fb_w * fb_h) as usize || scale == 0 || fb_w == 0 || fb_h == 0 {
        return;
    }
    let advance = (GLYPH_WIDTH + GLYPH_SPACING_X) * scale;
    let mut cursor_x = x;
    for ch in text.chars() {
        if let Some(glyph) = glyph_for(ch) {
            draw_glyph(fb, fb_w, fb_h, cursor_x, y, glyph, color, scale);
        }
        cursor_x += advance as i32;
    }
}

fn draw_glyph(
    fb: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    x: i32,
    y: i32,
    glyph: &[u8; GLYPH_HEIGHT as usize],
    color: u32,
    scale: u32,
) {
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..GLYPH_WIDTH {
            let bit = (bits >> (GLYPH_WIDTH - 1 - col)) & 1;
            if bit == 0 {
                continue;
            }
            let base_x = x + (col * scale) as i32;
            let base_y = y + (row as u32 * scale) as i32;
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = base_x + dx as i32;
                    let py = base_y + dy as i32;
                    if px < 0 || py < 0 || (px as u32) >= fb_w || (py as u32) >= fb_h {
                        continue;
                    }
                    let idx = (py as u32 * fb_w + px as u32) as usize;
                    fb[idx] = color;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: u32 = 0xff000000;
    const FG: u32 = 0xffffffff;

    fn empty(w: u32, h: u32) -> Vec<u32> {
        vec![BG; (w * h) as usize]
    }

    fn lit_count(fb: &[u32]) -> usize {
        fb.iter().filter(|px| **px != BG).count()
    }

    #[test]
    fn empty_string_is_noop() {
        let mut fb = empty(32, 16);
        draw_text(&mut fb, 32, 16, 0, 0, "", FG);
        assert_eq!(lit_count(&fb), 0);
    }

    #[test]
    fn draw_capital_i_lights_known_pixels() {
        let mut fb = empty(16, 16);
        draw_text(&mut fb, 16, 16, 0, 0, "I", FG);
        // 'I' is: 01110 / 00100 / 00100 / 00100 / 00100 / 00100 / 01110
        // 3 + 1 + 1 + 1 + 1 + 1 + 3 = 11 pixels
        assert_eq!(lit_count(&fb), 11);
        // Center column (x=2) should be lit at all 7 rows.
        for y in 0..7 {
            let idx = (y * 16 + 2) as usize;
            assert_eq!(fb[idx], FG, "col 2 row {} should be lit", y);
        }
    }

    #[test]
    fn scale_two_doubles_linear_extents() {
        let mut fb1 = empty(32, 32);
        let mut fb2 = empty(32, 32);
        draw_text(&mut fb1, 32, 32, 0, 0, "I", FG);
        draw_text_scaled(&mut fb2, 32, 32, 0, 0, "I", FG, 2);
        // Each lit pixel becomes a 2x2 block, so count quadruples.
        assert_eq!(lit_count(&fb2), lit_count(&fb1) * 4);
    }

    #[test]
    fn unsupported_char_renders_blank() {
        let mut fb = empty(16, 16);
        draw_text(&mut fb, 16, 16, 0, 0, "\u{1F600}", FG);
        assert_eq!(lit_count(&fb), 0);
    }

    #[test]
    fn off_screen_text_does_not_panic() {
        let mut fb = empty(16, 16);
        draw_text(&mut fb, 16, 16, -100, -100, "HI", FG);
        assert_eq!(lit_count(&fb), 0);
        draw_text(&mut fb, 16, 16, 100, 100, "HI", FG);
        assert_eq!(lit_count(&fb), 0);
    }

    #[test]
    fn multiple_chars_advance_cursor() {
        let mut fb1 = empty(64, 16);
        let mut fb2 = empty(64, 16);
        draw_text(&mut fb1, 64, 16, 0, 0, "I", FG);
        draw_text(&mut fb2, 64, 16, 0, 0, "II", FG);
        assert_eq!(lit_count(&fb2), lit_count(&fb1) * 2);
    }
}
