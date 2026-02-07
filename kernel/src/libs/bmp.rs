/// BMP Image Parser
extern crate alloc;
use crate::graphics::color;
use alloc::vec::Vec;

#[derive(Debug)]
pub enum BmpError {
    InvalidSignature,
    UnsupportedFormat,
    InvalidData,
}

pub struct BmpImage {
    width: u32,
    height: u32,
    pixels: Vec<color::Color>,
}

impl BmpImage {
    pub fn from_bytes(data: &[u8]) -> Result<Self, BmpError> {
        // Check BMP file header
        if data.len() < 54 || data[0] != b'B' || data[1] != b'M' {
            return Err(BmpError::InvalidSignature);
        }

        // Parse BMP file header
        let width = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
        let height = u32::from_le_bytes([data[22], data[23], data[24], data[25]]);
        let bpp = u16::from_le_bytes([data[28], data[29]]);

        // Only support 24-bit and 32-bit BMP
        if bpp != 24 && bpp != 32 {
            return Err(BmpError::UnsupportedFormat);
        }

        let data_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;

        let mut pixels = Vec::with_capacity((width * height) as usize);
        let bytes_per_pixel = (bpp / 8) as usize;
        let row_padding = (4 - (width * bytes_per_pixel as u32) % 4) % 4;

        // Parse pixel data (BMP is stored bottom-up)
        for y in 0..height {
            let row_start = data_offset
                + ((height - y - 1) * (width * bytes_per_pixel as u32 + row_padding)) as usize;

            for x in 0..width {
                let pixel_start = row_start + (x * bytes_per_pixel as u32) as usize;
                if pixel_start + bytes_per_pixel > data.len() {
                    return Err(BmpError::InvalidData);
                }

                let b = data[pixel_start];
                let g = data[pixel_start + 1];
                let r = data[pixel_start + 2];
                let a = if bytes_per_pixel == 4 {
                    data[pixel_start + 3]
                } else {
                    255
                };

                pixels.push(color::Color::with_alpha(r, g, b, a));
            }
        }

        Ok(BmpImage {
            width,
            height,
            pixels,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<color::Color> {
        if x < self.width && y < self.height {
            let index = (y * self.width + x) as usize;
            Some(self.pixels[index])
        } else {
            None
        }
    }
}
