const PALETTE: [(u8, u8, u8); 4] = [
  (15,56,15),
  (48,98,48),
  (139,172,15),
  (155,188,15),
];

const PIXEL_BYTES: usize = 4;
pub struct FrameBuffer {
    pub buffer: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

impl FrameBuffer {
  pub fn new(width: usize, height: usize) -> Self {
      let buffer = vec![0; width * height * PIXEL_BYTES];
      Self { buffer, width, height }
  }

  pub fn gameboy_lcd() -> Self {
    Self::new(32*8, 32*8)
  }

  pub fn pitch(&self) -> usize {
      self.width * PIXEL_BYTES
  }

  pub fn set_pixel(&mut self, x: usize, y: usize, color_id: u8) {
    let color = &PALETTE[color_id as usize];
    let idx = (y*self.width + x) * PIXEL_BYTES;
    self.buffer[idx + 0] = color.0;
    self.buffer[idx + 1] = color.1;
    self.buffer[idx + 2] = color.2;
    self.buffer[idx + 3] = 255;
}

pub fn set_tile(&mut self, x: usize, y: usize, tile: &[u8]) {
    for row in 0..8 {
      let plane0 = tile[row*2];
      let plane1 = tile[row*2 + 1];

      for bit in 0..8 {
          let bit0 = (plane0 >> bit) & 1;
          let bit1 = ((plane1 >> bit) & 1) << 1;
          let color_idx = bit1 | bit0;
          self.set_pixel(x + 7-bit, y + row, color_idx);
      }
    }
  }
}