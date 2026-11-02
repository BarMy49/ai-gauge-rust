use font8x8::{UnicodeFonts, BASIC_FONTS};

pub const SIZES: [usize; 6] = [16, 20, 24, 32, 48, 64];

pub fn text(p: Option<f64>) -> String {
    match p.filter(|p| p.is_finite()) {
        Some(p) if p > 999. => "!".into(),
        Some(p) => format!("{:.0}", p.max(0.)),
        None => "?".into(),
    }
}

// Three-column numerals keep all three digits distinct even in a 16px tray slot.
// Wider values use this condensed face rather than dropping columns when scaled.
fn glyph(c: char, condensed: bool) -> Vec<Vec<bool>> {
    if condensed {
        let rows = match c {
            '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
            '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
            '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
            '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
            '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
            '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
            '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
            '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
            '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
            '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
            _ => unreachable!("only three-digit numbers use the condensed font"),
        };
        return rows
            .iter()
            .map(|row| (0..3).map(|x| row & (1 << (2 - x)) != 0).collect())
            .collect();
    }
    let rows = if c == '0' {
        // An open counter reads more clearly than the font's diagonally slashed zero.
        [0x3e, 0x63, 0x63, 0x63, 0x63, 0x63, 0x3e, 0]
    } else {
        BASIC_FONTS.get(c).expect("supported icon character")
    };
    let top = rows.iter().position(|row| *row != 0).unwrap();
    let bottom = rows.iter().rposition(|row| *row != 0).unwrap();
    let left = (0..8)
        .find(|x| rows.iter().any(|r| r & (1 << x) != 0))
        .unwrap();
    let right = (0..8)
        .rfind(|x| rows.iter().any(|r| r & (1 << x) != 0))
        .unwrap();
    rows[top..=bottom]
        .iter()
        .map(|row| (left..=right).map(|x| row & (1 << x) != 0).collect())
        .collect()
}

pub fn rgba(p: Option<f64>, size: usize) -> Vec<u8> {
    let text = text(p);
    let glyphs: Vec<_> = text.chars().map(|c| glyph(c, text.len() == 3)).collect();
    let height = glyphs.iter().map(Vec::len).max().unwrap();
    let width = glyphs.iter().map(|g| g[0].len()).sum::<usize>() + glyphs.len() - 1;
    let mut source = vec![false; width * height];
    let mut left = 0;
    for glyph in &glyphs {
        for (y, row) in glyph.iter().enumerate() {
            source[y * width + left..y * width + left + row.len()].copy_from_slice(row);
        }
        left += glyph[0].len() + 1;
    }

    let outline = (size / 32).max(1);
    let draw_height = size - 2 * outline;
    let draw_width = (width * draw_height / height)
        .min(size - 2 * outline)
        .max(1);
    let offset_x = (size - draw_width) / 2;
    let mut mask = vec![false; size * size];
    for y in 0..draw_height {
        for x in 0..draw_width {
            mask[(y + outline) * size + x + offset_x] =
                source[(y * height / draw_height) * width + x * width / draw_width];
        }
    }

    let mut out = vec![0; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            if mask[y * size + x] {
                for oy in y.saturating_sub(outline)..=(y + outline).min(size - 1) {
                    for ox in x.saturating_sub(outline)..=(x + outline).min(size - 1) {
                        let i = (oy * size + ox) * 4;
                        out[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
                    }
                }
            }
        }
    }
    for (pixel, foreground) in out.chunks_exact_mut(4).zip(mask) {
        if foreground {
            pixel.copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    out
}

#[cfg(any(windows, test))]
pub fn ico(p: Option<f64>) -> Vec<u8> {
    let mut out = vec![];
    for n in [0u16, 1, SIZES.len() as u16] {
        out.extend(n.to_le_bytes());
    }
    let mut images = vec![];
    let mut offset = 6 + 16 * SIZES.len();
    for size in SIZES {
        let pixels = rgba(p, size);
        // Both bitmap planes have rows padded to DWORD boundaries.
        let mask_stride = size.div_ceil(32) * 4;
        let image_size = 40 + size * size * 4 + mask_stride * size;
        out.extend([size as u8, size as u8, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((image_size as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += image_size;

        images.extend(40u32.to_le_bytes());
        images.extend((size as i32).to_le_bytes());
        images.extend((size as i32 * 2).to_le_bytes());
        images.extend(1u16.to_le_bytes());
        images.extend(32u16.to_le_bytes());
        for n in [0u32, (size * size * 4) as u32, 0, 0, 0, 0] {
            images.extend(n.to_le_bytes());
        }
        for y in (0..size).rev() {
            for x in 0..size {
                let i = (y * size + x) * 4;
                images.extend([pixels[i + 2], pixels[i + 1], pixels[i], pixels[i + 3]]);
            }
        }
        for y in (0..size).rev() {
            let mut row = vec![0; mask_stride];
            for x in 0..size {
                if pixels[(y * size + x) * 4 + 3] == 0 {
                    row[x / 8] |= 0x80 >> (x % 8);
                }
            }
            images.extend(row);
        }
    }
    out.extend(images);
    out
}

#[cfg(target_os = "linux")]
pub fn argb(p: Option<f64>, size: usize) -> Vec<u8> {
    rgba(p, size)
        .chunks_exact(4)
        .flat_map(|x| [x[3], x[0], x[1], x[2]])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_height_digits_at_every_tray_size() {
        for size in SIZES {
            for p in [
                Some(0.),
                Some(7.),
                Some(37.),
                Some(100.),
                Some(999.),
                None,
                Some(1000.),
            ] {
                let pixels = rgba(p, size);
                assert_eq!(pixels.len(), size * size * 4);
                let white: Vec<_> = pixels
                    .chunks_exact(4)
                    .enumerate()
                    .filter(|(_, p)| *p == [255, 255, 255, 255])
                    .map(|(i, _)| (i % size, i / size))
                    .collect();
                let top = white.iter().map(|(_, y)| *y).min().unwrap();
                let bottom = white.iter().map(|(_, y)| *y).max().unwrap();
                assert!(bottom - top + 1 >= size * 7 / 8);
                assert!(white
                    .iter()
                    .all(|(x, y)| *x > 0 && *x < size - 1 && *y > 0 && *y < size - 1));
                assert!(pixels.chunks_exact(4).any(|p| p == [0, 0, 0, 255]));
                assert!(pixels.chunks_exact(4).any(|p| p[3] == 0));
            }
        }
    }

    #[test]
    fn icon_values_and_unknown_are_distinct() {
        assert_eq!(text(Some(37.4)), "37");
        assert_eq!(text(Some(37.6)), "38");
        assert_eq!(text(Some(-1.)), "0");
        assert_eq!(text(Some(999.)), "999");
        assert_eq!(text(Some(999.1)), "!");
        for p in [None, Some(f64::NAN), Some(f64::INFINITY)] {
            assert_eq!(text(p), "?");
        }
        for size in SIZES {
            assert_ne!(rgba(None, size), rgba(Some(0.), size));
            assert_ne!(rgba(Some(0.), size), rgba(Some(100.), size));
        }
    }

    #[test]
    fn ico_contains_native_sizes_and_aligned_transparency_masks() {
        let bytes = ico(Some(100.));
        assert_eq!(&bytes[..6], &[0, 0, 1, 0, SIZES.len() as u8, 0]);
        let u32_at = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let mut expected_offset = 6 + SIZES.len() * 16;
        for (n, size) in SIZES.into_iter().enumerate() {
            let entry = 6 + 16 * n;
            assert_eq!(bytes[entry] as usize, size);
            assert_eq!(bytes[entry + 1] as usize, size);
            assert_eq!(u32_at(entry + 12), expected_offset);
            let offset = expected_offset;
            assert_eq!(u32_at(offset), 40);
            assert_eq!(u32_at(offset + 4), size);
            assert_eq!(u32_at(offset + 8), size * 2);
            let stride = size.div_ceil(32) * 4;
            let length = 40 + size * size * 4 + stride * size;
            assert_eq!(u32_at(entry + 8), length);
            let mask_start = offset + 40 + size * size * 4;
            for y in 0..size {
                for x in 0..size {
                    let alpha = bytes[offset + 40 + (y * size + x) * 4 + 3];
                    let transparent =
                        bytes[mask_start + y * stride + x / 8] & (0x80 >> (x % 8)) != 0;
                    assert_eq!(transparent, alpha == 0);
                }
            }
            expected_offset += length;
        }
        assert_eq!(bytes.len(), expected_offset);
    }
}
