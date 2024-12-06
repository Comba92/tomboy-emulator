use std::{error::Error, fs};

use sdl2::{event::Event, pixels::PixelFormatEnum};
use tomboy_emulator::cpu::Cpu;

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

fn main() -> Result<(), Box<dyn Error>> {
  let sdl = sdl2::init()?;
  let video = sdl.video()?;
  let mut canvas = video.window("TomboyEmu", 48*16*3, 8*16*3)
    .position_centered().build()?.into_canvas()
    .accelerated().target_texture().build()?;

  let mut events = sdl.event_pump()?;

  let mut emu = Cpu::new();
  let rom = fs::read("./tests/roms/01-special.gb")?;
  //let cart = Cart::new(&rom);
  //println!("{:?}", cart);

  let (left, _) = emu.bus.mem.split_at_mut(rom.len());
  left.copy_from_slice(&rom);

  let texture_creator = canvas.texture_creator();
  let mut texture = texture_creator
    .create_texture_target(PixelFormatEnum::RGBA32, 48*16, 8*16)?;

  let mut framebuf = FrameBuffer::new(48*16, 8*16);

  'running: loop {
    emu.step();

    for event in events.poll_iter() {
      match event {
        Event::Quit { .. } => break 'running,
        _ => {}
      }

      for i in 0..384 {
        let x = i % 48;
        let y = i / 48;
        let tile_start = 0x8000 + i*16;
        let tile = &emu.bus.mem[tile_start..tile_start+16];
        framebuf.set_tile(x*16, y*16, &tile);
      }

      canvas.clear();
      texture.update(None, &framebuf.buffer, framebuf.pitch())?;
      canvas.copy(&texture, None, None)?;
      canvas.present();
    }
  }

  println!("Tileset {:?}", &emu.bus.mem[0x8000..0x9800]);
  println!("Tilemap {:?}", &emu.bus.mem[0x9800..0xA000]);

  Ok(())
}
