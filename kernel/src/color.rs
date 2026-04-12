//! The color format definition.

/// The basic color storer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Creates a new Color with the specified RGB values and default alpha (255)
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a new Color with the specified RGBA values
    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);

        if hex.len() == 8 {
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Self::with_alpha(r, g, b, a)
        } else {
            Self::new(r, g, b)
        }
    }

    pub fn to_u32(&self, include_alpha: bool) -> u32 {
        let r = self.r as u32;
        let g = self.g as u32;
        let b = self.b as u32;
        if include_alpha {
            let a = self.a as u32;
            return (a << 24) | (r << 16) | (g << 8) | b;
        }
        (0 << 24) | (r << 16) | (g << 8) | b
    }

    pub fn from_u32(color: u32) -> Self {
        let r = (color >> 24) & 0xFF;
        let g = (color >> 16) & 0xFF;
        let b = (color >> 8) & 0xFF;
        Self::new(r as u8, g as u8, b as u8)
    }

    pub fn mix_alpha(&self, alpha: u8) -> Self {
        Self::with_alpha(self.r, self.g, self.b, alpha)
    }

    pub fn mix(&self, other: &Color, alpha: u8) -> Self {
        let r =
            ((self.r as u16 * alpha as u16 + other.r as u16 * (255 - alpha) as u16) / 255) as u8;
        let g =
            ((self.g as u16 * alpha as u16 + other.g as u16 * (255 - alpha) as u16) / 255) as u8;
        let b =
            ((self.b as u16 * alpha as u16 + other.b as u16 * (255 - alpha) as u16) / 255) as u8;
        let a = alpha;
        Self { r, g, b, a }
    }
    pub fn invert(&self) -> Color {
        Color::new(255 - self.r, 255 - self.g, 255 - self.b)
    }
}

#[macro_export]
macro_rules! color {
    // RGB Format (alpha=255)
    ($r:expr, $g:expr, $b:expr) => {
        Color::new($r as u8, $g as u8, $b as u8)
    };

    // RGBA format
    ($r:expr, $g:expr, $b:expr, $a:expr) => {
        Color::with_alpha($r as u8, $g as u8, $b as u8, $a as u8)
    };

    // Hex code
    (#$hex:expr) => {
        Color::from_hex($hex)
    };
}

// Basic color
pub const BLACK: Color = color!(0, 0, 0);
pub const WHITE: Color = color!(255, 255, 255);
pub const RED: Color = color!(255, 0, 0);
pub const GREEN: Color = color!(0, 255, 0);
pub const BLUE: Color = color!(0, 0, 255);

// Mixed colors
pub const YELLOW: Color = color!(255, 255, 0);
pub const CYAN: Color = color!(0, 255, 255); 
pub const MAGENTA: Color = color!(255, 0, 255);
pub const GRAY: Color = color!(128, 128, 128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_color_from_hex_rgb() {
        let c = Color::from_hex("#ff8040");
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 64);
        assert_eq!(c.a, 255);
    }

    #[test_case]
    fn test_color_from_hex_rgba() {
        let c = Color::from_hex("#ff804080");
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 64);
        assert_eq!(c.a, 128);
    }

    #[test_case]
    fn test_color_from_hex_without_hash() {
        let c = Color::from_hex("ff8040");
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 64);
    }

    #[test_case]
    fn test_color_from_hex_invalid() {
        let c = Color::from_hex("invalid");
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 255);
        assert_eq!(c.b, 255);
    }

    #[test_case]
    fn test_color_to_u32_with_alpha() {
        let c = Color::with_alpha(0x12, 0x34, 0x56, 0x78);
        let val = c.to_u32(true);
        assert_eq!(val, 0x78123456);
    }

    #[test_case]
    fn test_color_to_u32_without_alpha() {
        let c = Color::new(0x12, 0x34, 0x56);
        let val = c.to_u32(false);
        assert_eq!(val, 0x12345600);
    }

    #[test_case]
    fn test_color_from_u32() {
        let val = 0x12345600;
        let c = Color::from_u32(val);
        assert_eq!(c.r, 0x12);
        assert_eq!(c.g, 0x34);
        assert_eq!(c.b, 0x56);
    }

    #[test_case]
    fn test_color_mix_alpha() {
        let c = Color::new(255, 128, 64);
        let c = c.mix_alpha(128);
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 64);
        assert_eq!(c.a, 128);
    }

    #[test_case]
    fn test_color_mix_half() {
        let c1 = Color::new(255, 0, 0);
        let c2 = Color::new(0, 255, 0);
        let result = c1.mix(&c2, 128);
        assert_eq!(result.r, 128);
        assert_eq!(result.g, 127);
        assert_eq!(result.b, 0);
    }

    #[test_case]
    fn test_color_mix_full_first() {
        let c1 = Color::new(255, 0, 0);
        let c2 = Color::new(0, 255, 0);
        let result = c1.mix(&c2, 255);
        assert_eq!(result.r, 255);
        assert_eq!(result.g, 0);
        assert_eq!(result.b, 0);
    }

    #[test_case]
    fn test_color_mix_full_second() {
        let c1 = Color::new(255, 0, 0);
        let c2 = Color::new(0, 255, 0);
        let result = c1.mix(&c2, 0);
        assert_eq!(result.r, 0);
        assert_eq!(result.g, 255);
        assert_eq!(result.b, 0);
    }

    #[test_case]
    fn test_color_invert() {
        let c = Color::new(0, 128, 255);
        let inverted = c.invert();
        assert_eq!(inverted.r, 255);
        assert_eq!(inverted.g, 127);
        assert_eq!(inverted.b, 0);
    }

    #[test_case]
    fn test_color_invert_white() {
        let c = Color::new(255, 255, 255);
        let inverted = c.invert();
        assert_eq!(inverted, BLACK);
    }

    #[test_case]
    fn test_color_invert_black() {
        let c = Color::new(0, 0, 0);
        let inverted = c.invert();
        assert_eq!(inverted, WHITE);
    }
}
