use crate::config::{ConfigFile, parse_hex_color};
use crate::input::{GRID_SIZE, compute_cell_bounds};
use memmap2::MmapMut;

pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: MmapMut,
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> (Self, std::fs::File) {
        let stride = width * 4;
        let size = (stride * height) as usize;

        let tmp_path = format!("/tmp/chopsticks-shm-{}", std::process::id());
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .expect("Failed to create temp file");
        std::fs::remove_file(&tmp_path).expect("Failed to remove temp file");
        file.set_len(size as u64).expect("Failed to set file size");

        let data = unsafe { MmapMut::map_mut(&file).expect("Failed to mmap") };

        (
            PixelBuffer {
                width,
                height,
                stride,
                data,
            },
            file,
        )
    }

    pub fn fill(&mut self, color: u32) {
        let bytes = color.to_ne_bytes();
        for chunk in self.data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bytes);
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, color);
            }
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let offset = (y * self.stride + x * 4) as usize;
        u32::from_ne_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ])
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = (y * self.stride + x * 4) as usize;
        self.data[offset..offset + 4].copy_from_slice(&color.to_ne_bytes());
    }

    pub fn draw_hline(&mut self, y: u32, x1: u32, x2: u32, color: u32) {
        for x in x1..=x2.min(self.width - 1) {
            self.set_pixel(x, y, color);
        }
    }

    pub fn draw_vline(&mut self, x: u32, y1: u32, y2: u32, color: u32) {
        for y in y1..=y2.min(self.height - 1) {
            self.set_pixel(x, y, color);
        }
    }
}

pub fn draw_grid(buffer: &mut PixelBuffer, width: u32, height: u32, grid_color: u32) {
    for i in 0..=GRID_SIZE {
        let x = if i == GRID_SIZE {
            width - 1
        } else {
            (i * width) / GRID_SIZE
        };
        let y = if i == GRID_SIZE {
            height - 1
        } else {
            (i * height) / GRID_SIZE
        };
        if x < buffer.width {
            buffer.draw_vline(x, 0, buffer.height - 1, grid_color);
        }
        if y < buffer.height {
            buffer.draw_hline(y, 0, buffer.width - 1, grid_color);
        }
    }
}

struct CachedGlyph {
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    data: Vec<u8>,
}

pub struct FontCache {
    advances: std::collections::HashMap<char, f32>,
    glyphs: std::collections::HashMap<char, CachedGlyph>,
}

impl FontCache {
    pub fn new(font_data: &[u8], font_size: u32) -> Self {
        use swash::{
            FontRef,
            scale::{Render, ScaleContext, Source},
        };

        let font = FontRef::from_index(font_data, 0).expect("Failed to load font");
        let mut context = ScaleContext::new();
        let mut scaler = context
            .builder(font)
            .size(font_size as f32)
            .hint(true)
            .build();

        let metrics = font.metrics(&[]);
        let units_per_em = f32::from(metrics.units_per_em);
        let scale = font_size as f32 / units_per_em;
        let glyph_metrics = font.glyph_metrics(&[]);

        let mut advances = std::collections::HashMap::new();
        let mut glyphs = std::collections::HashMap::new();

        let chars_to_cache: Vec<char> = ('a'..='z').chain([',', '.', ' ']).collect();

        for ch in chars_to_cache {
            let id = font.charmap().map(ch);
            advances.insert(ch, glyph_metrics.advance_width(id) * scale);

            let mut render = Render::new(&[Source::Outline]);
            let render = render.format(swash::zeno::Format::Alpha);

            if let Some(image) = render.render(&mut scaler, id) {
                glyphs.insert(
                    ch,
                    CachedGlyph {
                        width: image.placement.width,
                        height: image.placement.height,
                        left: image.placement.left,
                        top: image.placement.top,
                        data: image.data,
                    },
                );
            } else {
                glyphs.insert(
                    ch,
                    CachedGlyph {
                        width: 0,
                        height: 0,
                        left: 0,
                        top: 0,
                        data: Vec::new(),
                    },
                );
            }
        }

        FontCache { advances, glyphs }
    }
}

fn draw_text_label(
    buffer: &mut PixelBuffer,
    text: &str,
    cx: u32,
    cy: u32,
    cache: &FontCache,
    text_color: u32,
) {
    let chars: Vec<char> = text.chars().collect();

    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;

    let mut x_offset = 0f32;
    for &ch in &chars {
        let glyph = cache
            .glyphs
            .get(&ch)
            .unwrap_or_else(|| cache.glyphs.get(&' ').unwrap());
        let advance = cache.advances.get(&ch).copied().unwrap_or(0.0);
        let gx = x_offset as i32 + glyph.left;
        let gy = -glyph.top;
        min_x = min_x.min(gx);
        max_x = max_x.max(gx + glyph.width.cast_signed());
        min_y = min_y.min(gy);
        max_y = max_y.max(gy + glyph.height.cast_signed());
        x_offset += advance;
    }

    let text_width = (max_x - min_x) as f32;
    // Use only the portion above the baseline for vertical centering,
    // so descenders (e.g., 'y', 'g') don't push the main body upward.
    let visual_height = (0 - min_y) as f32;
    // Snap to integer pixel boundaries for crisp rendering
    let start_x = (cx as f32 - text_width / 2.0).round() as i32 - min_x;
    let start_y = (cy as f32 - visual_height / 2.0).round() as i32 - min_y;

    // Extract text RGB
    let fg_r = ((text_color >> 16) & 0xFF) as u16;
    let fg_g = ((text_color >> 8) & 0xFF) as u16;
    let fg_b = (text_color & 0xFF) as u16;

    let mut x_offset = 0f32;
    for &ch in &chars {
        let glyph = cache
            .glyphs
            .get(&ch)
            .unwrap_or_else(|| cache.glyphs.get(&' ').unwrap());
        let advance = cache.advances.get(&ch).copied().unwrap_or(0.0);
        let glyph_x = start_x + x_offset as i32 + glyph.left;
        let glyph_y = start_y - glyph.top;

        for (i, &pixel) in glyph.data.iter().enumerate() {
            if pixel == 0 {
                continue;
            }

            let px = i32::try_from(i % glyph.width as usize).unwrap_or_default();
            let py = i32::try_from(i / glyph.width as usize).unwrap_or_default();
            let buf_x = glyph_x + px;
            let buf_y = glyph_y + py;

            if buf_x < 0
                || buf_y < 0
                || (buf_x as u32) >= buffer.width
                || (buf_y as u32) >= buffer.height
            {
                continue;
            }

            let bx = buf_x as u32;
            let by = buf_y as u32;

            // Alpha blend with background
            let alpha = u16::from(pixel);
            let inv_alpha = 255 - alpha;

            let bg = buffer.get_pixel(bx, by);
            let bg_r = ((bg >> 16) & 0xFF) as u16;
            let bg_g = ((bg >> 8) & 0xFF) as u16;
            let bg_b = (bg & 0xFF) as u16;

            let r = u32::from((fg_r * alpha + bg_r * inv_alpha) / 255);
            let g = u32::from((fg_g * alpha + bg_g * inv_alpha) / 255);
            let b = u32::from((fg_b * alpha + bg_b * inv_alpha) / 255);
            let color = 0xFF00_0000 | (r << 16) | (g << 8) | b;

            buffer.set_pixel(bx, by, color);
        }
        x_offset += advance;
    }
}

const PRECISION_KEYS: [char; 12] = ['y', 'u', 'i', 'o', 'h', 'j', 'k', 'l', 'n', 'm', ',', '.'];

pub fn draw_labels(
    buffer: &mut PixelBuffer,
    width: u32,
    height: u32,
    font_size: u32,
    text_color: u32,
    selected_row: Option<u32>,
    selected_cell: Option<(u32, u32)>,
) {
    let font_path = "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf";
    let font_data = std::fs::read(font_path).expect("Failed to read font file");

    let cache = FontCache::new(&font_data, font_size);

    if let Some((sel_row, sel_col)) = selected_cell {
        let (cell_x1, cell_y1, cell_x2, cell_y2) =
            compute_cell_bounds(sel_col, sel_row, width, height);
        let cell_w = cell_x2 - cell_x1;
        let cell_h = cell_y2 - cell_y1;
        let sub_w = cell_w / 4;
        let sub_h = cell_h / 3;

        for (idx, &ch) in PRECISION_KEYS.iter().enumerate() {
            let sub_col = (idx % 4) as u32;
            let sub_row = (idx / 4) as u32;
            let cx = cell_x1 + sub_col * sub_w + sub_w / 2;
            let cy = cell_y1 + sub_row * sub_h + sub_h / 2;
            draw_text_label(buffer, &ch.to_string(), cx, cy, &cache, text_color);
        }
        return;
    }

    if let Some(sel_row) = selected_row {
        for col in 0..GRID_SIZE {
            let label = format!("{}", (b'a' + col as u8) as char);
            let cx = ((2 * col + 1) * width) / (2 * GRID_SIZE);
            let cy = ((2 * sel_row + 1) * height) / (2 * GRID_SIZE);
            draw_text_label(buffer, &label, cx, cy, &cache, text_color);
        }
        return;
    }

    for row in 0..GRID_SIZE {
        for col in 0..GRID_SIZE {
            let label = format!(
                "{}{}",
                (b'a' + row as u8) as char,
                (b'a' + col as u8) as char
            );
            let cx = ((2 * col + 1) * width) / (2 * GRID_SIZE);
            let cy = ((2 * row + 1) * height) / (2 * GRID_SIZE);
            draw_text_label(buffer, &label, cx, cy, &cache, text_color);
        }
    }
}

pub fn draw_precision_grid(
    buffer: &mut PixelBuffer,
    width: u32,
    height: u32,
    selected_row: u32,
    selected_col: u32,
    grid_color: u32,
) {
    draw_grid(buffer, width, height, grid_color);

    let (cell_x1, cell_y1, cell_x2, cell_y2) =
        compute_cell_bounds(selected_col, selected_row, width, height);
    let cell_w = cell_x2 - cell_x1;
    let cell_h = cell_y2 - cell_y1;
    let sub_w = cell_w / 4;
    let sub_h = cell_h / 3;

    for i in 1..4 {
        let x = cell_x1 + i * sub_w;
        for y in cell_y1..=cell_y2 {
            buffer.set_pixel(x, y, grid_color);
        }
    }

    for i in 1..3 {
        let y = cell_y1 + i * sub_h;
        for x in cell_x1..=cell_x2 {
            buffer.set_pixel(x, y, grid_color);
        }
    }

    // Draw a small dot at the center of the cell to indicate space key target
    let center_x = cell_x1 + cell_w / 2;
    let center_y = cell_y1 + cell_h / 2;
    let dot_color = grid_color;
    for dy in -1..=1_i32 {
        for dx in -1..=1_i32 {
            let px = (center_x.cast_signed() + dx) as u32;
            let py = (center_y.cast_signed() + dy) as u32;
            buffer.set_pixel(px, py, dot_color);
        }
    }
}

pub fn render_frame(
    buffer: &mut PixelBuffer,
    width: u32,
    height: u32,
    selected_row: Option<u32>,
    selected_cell: Option<(u32, u32)>,
    config: &ConfigFile,
) {
    let grid_rgb = parse_hex_color(&config.grid_color);
    let grid_color = (u32::from(config.grid_opacity) << 24) | grid_rgb;
    let text_rgb = parse_hex_color(&config.text_color);
    let text_color = 0xFF00_0000 | text_rgb;

    let idle_bg = if config.idle_bg_opacity == 0 {
        0x0000_0000
    } else {
        (u32::from(config.idle_bg_opacity) << 24) | 0x0011_1133
    };
    let row_bg = if config.row_bg_opacity == 0 {
        0x0000_0000
    } else {
        (u32::from(config.row_bg_opacity) << 24) | 0x0011_1133
    };
    let cell_bg = if config.cell_bg_opacity == 0 {
        0x0000_0000
    } else {
        (u32::from(config.cell_bg_opacity) << 24) | 0x0033_3399
    };

    buffer.fill(idle_bg);

    if let Some((row, col)) = selected_cell {
        let (x1, y1, x2, y2) = compute_cell_bounds(col, row, width, height);
        buffer.fill_rect(x1, y1, x2 - x1, y2 - y1, cell_bg);
        draw_precision_grid(buffer, width, height, row, col, grid_color);

        let cell_font_size =
            ((height / GRID_SIZE).min(width / GRID_SIZE)) / config.font_size_divisor;
        let font_size = cell_font_size * 2 / 3;
        draw_labels(
            buffer,
            width,
            height,
            font_size,
            text_color,
            None,
            selected_cell,
        );
        return;
    }

    if let Some(row) = selected_row {
        let y1 = (row * height) / GRID_SIZE;
        let y2 = if row == GRID_SIZE - 1 {
            height - 1
        } else {
            ((row + 1) * height) / GRID_SIZE
        };
        buffer.fill_rect(0, y1, width, y2 - y1 + 1, row_bg);
        draw_grid(buffer, width, height, grid_color);

        let font_size = ((height / GRID_SIZE).min(width / GRID_SIZE)) / config.font_size_divisor;
        draw_labels(
            buffer,
            width,
            height,
            font_size,
            text_color,
            selected_row,
            None,
        );
        return;
    }

    draw_grid(buffer, width, height, grid_color);
    let font_size = ((height / GRID_SIZE).min(width / GRID_SIZE)) / config.font_size_divisor;
    draw_labels(buffer, width, height, font_size, text_color, None, None);
}
