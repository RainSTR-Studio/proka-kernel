extern crate alloc;
use crate::color;
use crate::graphics::{color::Color, Pixel, Renderer};
use crate::FRAMEBUFFER_REQUEST;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::fmt::{self, Write};
use lazy_static::lazy_static;

pub const DEFAULT_FONT_SIZE: f32 = 10.0;
pub const TAB_SPACES: usize = 4;
pub const GLYPH_CACHE_SIZE: usize = 95; // ASCII printable characters
pub const MAX_ANSI_PARAMS: usize = 8;

// The default font writer
lazy_static! {
    pub static ref DEFAULT_FONT: FontRef<'static> = {
        let font_data = include_bytes!("../../../fonts/maple-mono.ttf");
        FontRef::try_from_slice(font_data).expect("Failed to load font")
    };
}

/// Represents a character with its foreground and background colors in the console buffer.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ConsoleChar {
    ch: char,
    fg: Color,
    bg: Color,
}

#[derive(Clone)]
struct GlyphBitmap {
    bitmap: Vec<u8>,
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
}

// LRU Cache Item
struct CacheItem {
    bitmap: GlyphBitmap,
    last_used: u64,
}

struct GlyphCache {
    cache: BTreeMap<char, CacheItem>,
    counter: u64,
    size: usize,
}

impl GlyphCache {
    fn new(size: usize) -> Self {
        Self {
            cache: BTreeMap::new(),
            counter: 0,
            size,
        }
    }

    fn get(&mut self, ch: char) -> Option<GlyphBitmap> {
        if let Some(item) = self.cache.get_mut(&ch) {
            self.counter += 1;
            item.last_used = self.counter;
            return Some(item.bitmap.clone());
        }
        None
    }

    fn put(&mut self, ch: char, bitmap: GlyphBitmap) {
        self.counter += 1;
        if self.cache.len() >= self.size {
            if let Some((&k, _)) = self.cache.iter().min_by_key(|(_, v)| v.last_used) {
                let key_to_remove = k;
                self.cache.remove(&key_to_remove);
            }
        }
        self.cache.insert(
            ch,
            CacheItem {
                bitmap,
                last_used: self.counter,
            },
        );
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.counter = 0;
    }
}

// ANSI parse state
#[derive(Debug, PartialEq, Eq)]
enum AnsiParseState {
    Normal,
    Escape,
    Csi,
}

pub struct TtfConsole<'a> {
    pub renderer: Renderer<'a>,
    font: FontRef<'static>,
    scale: PxScale,
    font_size: f32,

    buffer: Vec<Option<ConsoleChar>>,
    scroll_offset_y: usize,

    width_chars: u32,
    height_chars: u32,

    cursor_x: u32,
    cursor_y: u32,
    prev_cursor_x: u32,
    prev_cursor_y: u32,
    current_color: Color,
    current_bg_color: Color,
    default_color: Color,
    default_bg_color: Color,

    font_width: u32,
    font_height: u32,
    font_baseline: f32,

    cursor_needs_redraw: bool,
    glyph_cache: GlyphCache,
    hidden_cursor: bool,

    ansi_parse_state: AnsiParseState,
    ansi_params: [u32; MAX_ANSI_PARAMS],
    ansi_param_count: usize,
    ansi_has_digit: bool,
}

impl<'a> TtfConsole<'a> {
    pub fn new(renderer: Renderer<'a>, font: FontRef<'static>) -> Self {
        let mut console = Self {
            renderer,
            font,
            scale: PxScale::from(0.0),
            font_size: DEFAULT_FONT_SIZE,
            cursor_x: 0,
            cursor_y: 0,
            buffer: Vec::new(),
            scroll_offset_y: 0,
            width_chars: 0,
            height_chars: 0,
            current_color: crate::graphics::color::WHITE,
            current_bg_color: crate::graphics::color::BLACK,
            default_color: crate::graphics::color::WHITE,
            default_bg_color: crate::graphics::color::BLACK,
            font_width: 0,
            font_height: 0,
            font_baseline: 0.0,
            cursor_needs_redraw: true,
            glyph_cache: GlyphCache::new(GLYPH_CACHE_SIZE),
            hidden_cursor: true,
            ansi_parse_state: AnsiParseState::Normal,
            ansi_params: [0; MAX_ANSI_PARAMS],
            ansi_param_count: 0,
            ansi_has_digit: false,
            prev_cursor_x: 0,
            prev_cursor_y: 0,
        };
        console.init_font_metrics(DEFAULT_FONT_SIZE);
        console.buffer = vec![None; (console.width_chars * console.height_chars) as usize];
        console
    }

    pub fn init() -> Self {
        let renderer = Renderer::new(
            FRAMEBUFFER_REQUEST
                .get_response()
                .expect("Framebuffer request failed")
                .framebuffers()
                .next()
                .expect("No framebuffer found"),
        );
        Self::new(renderer, DEFAULT_FONT.clone())
    }

    fn init_font_metrics(&mut self, font_size_pt: f32) {
        self.font_size = font_size_pt;
        self.scale = self
            .font
            .pt_to_px_scale(font_size_pt)
            .unwrap_or(PxScale::from(16.0));
        let scaled_font = self.font.as_scaled(self.scale);

        let ascent = scaled_font.ascent();
        let descent = scaled_font.descent();
        let line_gap = scaled_font.line_gap();

        let font_line_height = ascent - descent + line_gap;
        self.font_baseline = ascent;

        let g_id = self.font.glyph_id('M');
        let g = g_id.with_scale(self.scale);
        let bound = self.font.glyph_bounds(&g);

        self.font_width = libm::ceilf(bound.width()) as u32;
        self.font_height = libm::ceilf(font_line_height) as u32;

        self.width_chars = self
            .renderer
            .width()
            .checked_div(self.font_width as u64)
            .unwrap_or(1) as u32;
        self.height_chars = self
            .renderer
            .height()
            .checked_div(self.font_height as u64)
            .unwrap_or(1) as u32;

        self.cursor_x = 0;
        self.cursor_y = 0;
        self.prev_cursor_x = 0;
        self.prev_cursor_y = 0;
        self.scroll_offset_y = 0;
    }

    pub fn set_font(&mut self, new_font_data: &'static [u8], new_font_size: Option<f32>) {
        match FontRef::try_from_slice(new_font_data) {
            Ok(new_font) => {
                self.font = new_font;
                let size_to_use = new_font_size.unwrap_or(self.font_size);
                self.init_font_metrics(size_to_use);
                self.cursor_needs_redraw = true;
                self.glyph_cache.clear();
                self.redraw();
            }
            Err(_) => {
                return;
            }
        }
    }

    pub fn get_renderer(&mut self) -> &mut Renderer<'a> {
        &mut self.renderer
    }

    pub fn cursor_hidden(&mut self) {
        self.hidden_cursor = true;
    }

    pub fn cursor_visible(&mut self) {
        self.hidden_cursor = false;
    }

    pub fn clear(&mut self) {
        self.buffer.fill(None);
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.scroll_offset_y = 0;
        self.cursor_needs_redraw = true;
        self.redraw();
    }

    #[allow(dead_code)]
    fn clear_screen_pixels(&mut self) {
        let raw_clear_color = self.renderer.get_clear_color();
        self.renderer.set_clear_color(self.current_bg_color);
        self.renderer.clear();
        self.renderer.set_clear_color(raw_clear_color);
    }

    pub fn scroll(&mut self, lines: i32) {
        if lines == 0 {
            return;
        }
        let total_rows = self.buffer.len() / self.width_chars as usize;
        let old_offset = self.scroll_offset_y;
        let new_offset = (self.scroll_offset_y as i32 + lines)
            .max(0)
            .min((total_rows as i32 - self.height_chars as i32).max(0))
            as usize;

        if old_offset == new_offset {
            return;
        }

        let actual_lines = new_offset as i32 - old_offset as i32;
        self.scroll_offset_y = new_offset;
        self.cursor_needs_redraw = true;

        if actual_lines.abs() < self.height_chars as i32 {
            let pixel_offset = -(actual_lines * self.font_height as i32);
            self.renderer.scroll_y(pixel_offset as i64);

            if actual_lines > 0 {
                for y in (self.height_chars as i32 - actual_lines) as u32..self.height_chars {
                    self.draw_line_to_screen(y);
                }
            } else {
                for y in 0..(-actual_lines) as u32 {
                    self.draw_line_to_screen(y);
                }
            }
            self.draw_cursor();
            self.renderer.present();
        } else {
            self.redraw();
        }
    }

    fn draw_line_to_screen(&mut self, screen_y: u32) {
        if screen_y >= self.height_chars {
            return;
        }
        let buf_y = (screen_y + self.scroll_offset_y as u32) as usize;
        let total_rows = self.buffer.len() / self.width_chars as usize;
        if buf_y >= total_rows {
            let width = self.renderer.width();
            let bg_color = self.current_bg_color;
            self.renderer.fill_rect(
                Pixel::new(0, (screen_y * self.font_height) as u64),
                width,
                self.font_height as u64,
                bg_color,
            );
            return;
        }

        let start_idx = buf_y * self.width_chars as usize;
        let end_idx = start_idx + self.width_chars as usize;
        let row_data: Vec<Option<ConsoleChar>> = self.buffer[start_idx..end_idx].to_vec();
        for (x, cell) in row_data.into_iter().enumerate() {
            if let Some(char_info) = cell {
                self.draw_char_to_screen_at_px(
                    char_info.ch,
                    x as u32 * self.font_width,
                    screen_y * self.font_height,
                    char_info.fg,
                    char_info.bg,
                );
            } else {
                let font_width = self.font_width;
                let font_height = self.font_height;
                let bg_color = self.current_bg_color;
                self.renderer.fill_rect(
                    Pixel::new(
                        (x as u32 * font_width) as u64,
                        (screen_y * font_height) as u64,
                    ),
                    font_width as u64,
                    font_height as u64,
                    bg_color,
                );
            }
        }
    }

    fn ensure_buffer_capacity(&mut self) {
        let target_buf_y = (self.cursor_y + self.scroll_offset_y as u32) as usize;
        let total_rows = self.buffer.len() / self.width_chars as usize;
        if target_buf_y >= total_rows {
            let new_total_rows = target_buf_y + 1;
            self.buffer
                .resize(new_total_rows * self.width_chars as usize, None);
        }

        if self.cursor_y >= self.height_chars {
            let lines_to_scroll = self.cursor_y - self.height_chars + 1;
            let old_scroll_offset_y = self.scroll_offset_y;
            self.scroll_offset_y = (self.scroll_offset_y as u32 + lines_to_scroll) as usize;
            self.cursor_y = self.height_chars - 1;

            if old_scroll_offset_y != self.scroll_offset_y {
                let pixel_offset = -(lines_to_scroll as i32 * self.font_height as i32);
                self.renderer.scroll_y(pixel_offset as i64);

                for y in (self.height_chars - lines_to_scroll)..self.height_chars {
                    self.draw_line_to_screen(y);
                }
            }
        }
    }

    /// 将字符写入当前光标位置（不移动光标）
    fn put_char_impl(&mut self, ch: char) {
        self.ensure_buffer_capacity();

        let current_buf_y = (self.cursor_y + self.scroll_offset_y as u32) as usize;
        let buf_x = self.cursor_x as usize;
        let idx = current_buf_y * self.width_chars as usize + buf_x;

        let new_char_info = Some(ConsoleChar {
            ch,
            fg: self.current_color,
            bg: self.current_bg_color,
        });

        let mut needs_redraw = true;
        if let Some(current_cell) = self.buffer.get(idx) {
            if *current_cell == new_char_info {
                needs_redraw = false;
            }
        }

        if let Some(cell) = self.buffer.get_mut(idx) {
            *cell = new_char_info;
        }

        if needs_redraw && self.cursor_y < self.height_chars {
            self.draw_char_to_screen_at_px(
                ch,
                self.cursor_x * self.font_width,
                self.cursor_y * self.font_height,
                self.current_color,
                self.current_bg_color,
            );
        }

        self.cursor_needs_redraw = true;
    }

    #[inline(always)]
    fn draw_char_to_screen_at_px(
        &mut self,
        ch: char,
        x_px: u32,
        y_px: u32,
        fg_color: Color,
        bg_color: Color,
    ) {
        let bitmap = if let Some(bm) = self.glyph_cache.get(ch) {
            bm
        } else {
            let glyph = match self
                .font
                .outline_glyph(self.font.glyph_id(ch).with_scale(self.scale))
            {
                Some(g) => g,
                None => return,
            };
            let px_bounds = glyph.px_bounds();
            let width = px_bounds.width() as u32;
            let height = px_bounds.height() as u32;
            let mut data = vec![0u8; (width * height) as usize];

            glyph.draw(|x, y, c| {
                let alpha = (c * 255.0) as u8;
                if alpha > 0 {
                    let idx = (y * width + x) as usize;
                    if idx < data.len() {
                        data[idx] = alpha;
                    }
                }
            });

            let new_bitmap = GlyphBitmap {
                bitmap: data,
                width,
                height,
                offset_x: px_bounds.min.x as i32,
                offset_y: px_bounds.min.y as i32,
            };
            self.glyph_cache.put(ch, new_bitmap.clone());
            new_bitmap
        };

        self.renderer.fill_rect(
            Pixel::new(x_px as u64, y_px as u64),
            self.font_width as u64,
            self.font_height as u64,
            bg_color,
        );

        let baseline_y = y_px as f32 + self.font_baseline;
        let start_x = (x_px as i32 + bitmap.offset_x) as u64;
        let start_y = (baseline_y + bitmap.offset_y as f32) as u64;
        for row in 0..bitmap.height {
            for col in 0..bitmap.width {
                let alpha = bitmap.bitmap[(row * bitmap.width + col) as usize];
                unsafe {
                    self.renderer.set_pixel_raw_unchecked(
                        start_x + col as u64,
                        start_y + row as u64,
                        &fg_color.mix_alpha(alpha),
                    );
                }
            }
        }
    }

    pub fn draw_cursor(&mut self) {
        if !self.cursor_needs_redraw || self.hidden_cursor {
            return;
        }
        if self.prev_cursor_y < self.height_chars {
            let prev_x = self.prev_cursor_x;
            let prev_y = self.prev_cursor_y;
            let buf_y = (prev_y + self.scroll_offset_y as u32) as usize;
            let idx = buf_y * self.width_chars as usize + prev_x as usize;

            if let Some(Some(char_info)) = self.buffer.get(idx) {
                self.draw_char_to_screen_at_px(
                    char_info.ch,
                    prev_x * self.font_width,
                    prev_y * self.font_height,
                    char_info.fg,
                    char_info.bg,
                );
            } else {
                self.renderer.fill_rect(
                    Pixel::new(
                        (prev_x * self.font_width) as u64,
                        (prev_y * self.font_height) as u64,
                    ),
                    self.font_width as u64,
                    self.font_height as u64,
                    self.current_bg_color,
                );
            }
        }
        if self.cursor_y < self.height_chars {
            let cursor_x_px = self.cursor_x * self.font_width;
            let cursor_y_px = self.cursor_y * self.font_height;

            let inverse_color = self.current_bg_color.invert();
            self.renderer.fill_rect(
                Pixel::new(cursor_x_px as u64, cursor_y_px as u64),
                self.font_width as u64,
                self.font_height as u64,
                inverse_color,
            );
        }
        self.prev_cursor_x = self.cursor_x;
        self.prev_cursor_y = self.cursor_y;
        self.cursor_needs_redraw = false;
    }

    pub fn redraw(&mut self) {
        self.clear_screen_pixels();

        let start_display_row = self.scroll_offset_y;
        let end_display_row = (self.scroll_offset_y + self.height_chars as usize)
            .min(self.buffer.len() / self.width_chars as usize);

        let mut chars_to_draw = Vec::new();

        for y_offset in 0..(end_display_row - start_display_row) {
            let buf_y = start_display_row + y_offset;
            let row_start = buf_y * self.width_chars as usize;
            let row_end = row_start + self.width_chars as usize;

            for (x, cell) in self.buffer[row_start..row_end].iter().enumerate() {
                if let Some(char_info) = cell {
                    chars_to_draw.push((
                        char_info.ch,
                        x as u32 * self.font_width,
                        y_offset as u32 * self.font_height,
                        char_info.fg,
                        char_info.bg,
                    ));
                }
            }
        }

        for (ch, x, y, fg, bg) in chars_to_draw {
            self.draw_char_to_screen_at_px(ch, x, y, fg, bg);
        }

        self.draw_cursor();
        self.renderer.present();
    }

    pub fn write_string(&mut self, string: &str) {
        let mut rest = string;
        while !rest.is_empty() {
            if self.ansi_parse_state == AnsiParseState::Normal {
                if let Some(esc_pos) = rest.find('\x1b') {
                    let (normal, tail) = rest.split_at(esc_pos);
                    for c in normal.chars() {
                        self.handle_normal_char(c);
                    }
                    self.ansi_parse_state = AnsiParseState::Escape;
                    rest = &tail[1..];
                } else {
                    for c in rest.chars() {
                        self.handle_normal_char(c);
                    }
                    break;
                }
            } else {
                let mut chars = rest.chars();
                if let Some(c) = chars.next() {
                    self.handle_ansi_char(c);
                    rest = chars.as_str();
                } else {
                    break;
                }
            }
        }

        self.cursor_needs_redraw = true;

        self.draw_cursor();
        self.renderer.present();
    }

    fn handle_normal_char(&mut self, c: char) {
        match c {
            '\n' => {
                self.cursor_x = 0;
                self.cursor_y += 1;
                self.ensure_buffer_capacity();
            }
            '\r' => {
                self.cursor_x = 0;
            }
            '\t' => {
                let mut spaces_to_add = TAB_SPACES as u32 - (self.cursor_x % TAB_SPACES as u32);
                if spaces_to_add == 0 {
                    spaces_to_add = TAB_SPACES as u32;
                }
                for _ in 0..spaces_to_add {
                    self.put_char_impl(' ');
                    self.cursor_x += 1;
                    if self.cursor_x >= self.width_chars {
                        self.cursor_x = 0;
                        self.cursor_y += 1;
                        self.ensure_buffer_capacity();
                        break;
                    }
                }
            }
            _ => {
                self.put_char_impl(c);
                self.cursor_x += 1;
                if self.cursor_x >= self.width_chars {
                    self.cursor_x = 0;
                    self.cursor_y += 1;
                    self.ensure_buffer_capacity();
                }
            }
        }
    }

    fn handle_ansi_char(&mut self, c: char) {
        match self.ansi_parse_state {
            AnsiParseState::Escape => {
                if c == '[' {
                    self.ansi_parse_state = AnsiParseState::Csi;
                    self.ansi_params.fill(0);
                    self.ansi_param_count = 0;
                    self.ansi_has_digit = false;
                } else {
                    self.ansi_parse_state = AnsiParseState::Normal;
                    self.handle_normal_char(c);
                }
            }
            AnsiParseState::Csi => {
                if c.is_ascii_digit() {
                    let val = c as u32 - '0' as u32;
                    self.ansi_params[self.ansi_param_count] =
                        self.ansi_params[self.ansi_param_count] * 10 + val;
                    self.ansi_has_digit = true;
                } else if c == ';' {
                    if self.ansi_param_count + 1 < MAX_ANSI_PARAMS {
                        self.ansi_param_count += 1;
                        self.ansi_has_digit = false;
                    }
                } else if c == 'm' {
                    let count = if self.ansi_has_digit || self.ansi_param_count > 0 {
                        self.ansi_param_count + 1
                    } else {
                        0
                    };

                    let mut params = [0u32; MAX_ANSI_PARAMS];
                    params[..count].copy_from_slice(&self.ansi_params[..count]);

                    self.apply_ansi_codes(&params[..count]);
                    self.ansi_parse_state = AnsiParseState::Normal;
                } else {
                    self.ansi_parse_state = AnsiParseState::Normal;
                }
            }
            _ => self.ansi_parse_state = AnsiParseState::Normal,
        }
    }

    fn ansi_code_to_color(code: u32) -> Option<Color> {
        match code {
            30 | 40 => Some(crate::graphics::color::BLACK),
            31 | 41 => Some(crate::graphics::color::RED),
            32 | 42 => Some(crate::graphics::color::GREEN),
            33 | 43 => Some(crate::graphics::color::YELLOW),
            34 | 44 => Some(crate::graphics::color::BLUE),
            35 | 45 => Some(crate::graphics::color::MAGENTA),
            36 | 46 => Some(crate::graphics::color::CYAN),
            37 | 47 => Some(crate::graphics::color::WHITE),

            90 => Some(color!(128, 128, 128)),
            91 => Some(color!(255, 100, 100)),
            92 => Some(color!(100, 255, 100)),
            93 => Some(color!(255, 255, 100)),
            94 => Some(color!(100, 100, 255)),
            95 => Some(color!(255, 100, 255)),
            96 => Some(color!(100, 255, 255)),
            97 => Some(color!(255, 255, 255)),

            100 => Some(color!(64, 64, 64)),
            101 => Some(color!(150, 0, 0)),
            102 => Some(color!(0, 150, 0)),
            103 => Some(color!(150, 150, 0)),
            104 => Some(color!(0, 0, 150)),
            105 => Some(color!(150, 0, 150)),
            106 => Some(color!(0, 150, 150)),
            107 => Some(color!(150, 150, 150)),
            _ => None,
        }
    }

    fn apply_ansi_codes(&mut self, codes: &[u32]) {
        if codes.is_empty() {
            self.set_fg_color(self.default_color);
            self.set_bg_color(self.default_bg_color);
            return;
        }

        for &code in codes {
            match code {
                0 => {
                    self.set_fg_color(self.default_color);
                    self.set_bg_color(self.default_bg_color);
                }
                30..=37 => {
                    if let Some(color) = Self::ansi_code_to_color(code) {
                        self.set_fg_color(color);
                    }
                }
                39 => {
                    self.set_fg_color(self.default_color);
                }
                40..=47 => {
                    if let Some(color) = Self::ansi_code_to_color(code) {
                        self.set_bg_color(color);
                    }
                }
                49 => {
                    self.set_bg_color(self.default_bg_color);
                }
                90..=97 => {
                    if let Some(color) = Self::ansi_code_to_color(code) {
                        self.set_fg_color(color);
                    }
                }
                100..=107 => {
                    if let Some(color) = Self::ansi_code_to_color(code) {
                        self.set_bg_color(color);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn set_fg_color(&mut self, color: Color) {
        if self.current_color != color {
            self.current_color = color;
            self.cursor_needs_redraw = true;
        }
    }

    pub fn set_bg_color(&mut self, color: Color) {
        if self.current_bg_color != color {
            self.current_bg_color = color;
            self.renderer.set_clear_color(color);
            self.cursor_needs_redraw = true;
            self.redraw();
        }
    }
}

impl Write for TtfConsole<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// Implement the unified [`Console`] trait
impl<'a> crate::output::console::Console for TtfConsole<'a> {
    fn clear(&mut self) {
        self.buffer.fill(None);
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.scroll_offset_y = 0;
        self.cursor_needs_redraw = true;
        self.redraw();
    }

    fn set_fg_color(&mut self, color: Color) {
        if self.current_color != color {
            self.current_color = color;
            self.cursor_needs_redraw = true;
        }
    }

    fn set_bg_color(&mut self, color: Color) {
        if self.current_bg_color != color {
            self.current_bg_color = color;
            self.renderer.set_clear_color(color);
            self.cursor_needs_redraw = true;
            self.redraw();
        }
    }

    fn get_fg_color(&self) -> Color {
        self.current_color
    }

    fn get_bg_color(&self) -> Color {
        self.current_bg_color
    }

    fn put_char(&mut self, ch: char) {
        // 使用现有的 handle_normal_char 方法处理字符
        // 但对于普通字符，直接调用 put_char 并更新位置
        match ch {
            '\n' => {
                self.cursor_x = 0;
                self.cursor_y += 1;
                self.ensure_buffer_capacity();
            }
            '\r' => {
                self.cursor_x = 0;
            }
            '\t' => {
                let mut spaces_to_add = TAB_SPACES as u32 - (self.cursor_x % TAB_SPACES as u32);
                if spaces_to_add == 0 {
                    spaces_to_add = TAB_SPACES as u32;
                }
                for _ in 0..spaces_to_add {
                    self.put_char_impl(' ');
                    self.cursor_x += 1;
                    if self.cursor_x >= self.width_chars {
                        self.cursor_x = 0;
                        self.cursor_y += 1;
                        self.ensure_buffer_capacity();
                        break;
                    }
                }
            }
            _ => {
                self.put_char_impl(ch);
                self.cursor_x += 1;
                if self.cursor_x >= self.width_chars {
                    self.cursor_x = 0;
                    self.cursor_y += 1;
                    self.ensure_buffer_capacity();
                }
            }
        }
        self.cursor_needs_redraw = true;
        self.draw_cursor();
        self.renderer.present();
    }

    fn cursor_up(&mut self, lines: u32) {
        self.cursor_y = self.cursor_y.saturating_sub(lines);
        self.cursor_needs_redraw = true;
    }

    fn cursor_down(&mut self, lines: u32) {
        self.cursor_y = (self.cursor_y + lines).min(self.height_chars - 1);
        self.ensure_buffer_capacity();
        self.cursor_needs_redraw = true;
    }

    fn cursor_left(&mut self, cols: u32) {
        self.cursor_x = self.cursor_x.saturating_sub(cols);
        self.cursor_needs_redraw = true;
    }

    fn cursor_right(&mut self, cols: u32) {
        self.cursor_x = (self.cursor_x + cols).min(self.width_chars - 1);
        self.cursor_needs_redraw = true;
    }

    fn set_cursor_pos(&mut self, x: u32, y: u32) {
        self.cursor_x = x.min(self.width_chars - 1);
        self.cursor_y = y;
        self.ensure_buffer_capacity();
        self.cursor_needs_redraw = true;
    }

    fn get_cursor_pos(&self) -> (u32, u32) {
        (self.cursor_x, self.cursor_y)
    }

    fn cursor_hide(&mut self) {
        self.hidden_cursor = true;
        self.cursor_needs_redraw = true;
    }

    fn cursor_show(&mut self) {
        self.hidden_cursor = false;
        self.cursor_needs_redraw = true;
    }
}
