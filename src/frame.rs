const PALETTE: [(u8, u8, u8); 4] = [
  (155,188,15),
  (139,172,15),
  (48,98,48),
  (15,56,15),
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
}