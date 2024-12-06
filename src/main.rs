use std::{error::Error, fs};

use sdl2::{event::Event, pixels::PixelFormatEnum};
use tomboy_emulator::{cpu::Cpu, frame::FrameBuffer};

fn main() -> Result<(), Box<dyn Error>> {
  let sdl = sdl2::init()?;
  let video = sdl.video()?;

  const WIN_WIDTH: u32 = 32*16;
  const WIN_HEIGHT: u32 = 12*16;

  let mut canvas = video.window("TomboyEmu", WIN_WIDTH*3, WIN_HEIGHT*3)
    .position_centered().build()?.into_canvas()
    .accelerated().target_texture().build()?;

  let mut events = sdl.event_pump()?;

  let mut emu = Cpu::new();
  let rom = fs::read("./roms/Super Mario Land (JUE) (V1.1) [!].gb")?;
  
  //let cart = Cart::new(&rom);
  //println!("{:?}", cart);

  let (left, _) = emu.bus.mem.split_at_mut(rom.len());
  left.copy_from_slice(&rom);

  let texture_creator = canvas.texture_creator();
  let mut texture = texture_creator
    .create_texture_target(PixelFormatEnum::RGBA32, WIN_WIDTH, WIN_HEIGHT)?;

  let mut framebuf = FrameBuffer::new(WIN_WIDTH as usize, WIN_HEIGHT as usize);

  'running: loop {
    while emu.bus.ppu.vblank_request.is_some() {
      emu.step();
    }

    for event in events.poll_iter() {
      match event {
        Event::Quit { .. } => break 'running,
        _ => {}
      }

      for i in 0..384 {
        let x = i % (WIN_WIDTH as usize/16);
        let y = i / (WIN_WIDTH as usize/16);
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

  println!("{:?}", emu);
  println!("IE {:?} IF {:?}", emu.bus.inte, emu.bus.intf);
  println!("Tileset {:?}", &emu.bus.mem[0x8000..0x9800]);
  println!("Tilemap {:?}", &emu.bus.mem[0x9800..0xA000]);

  Ok(())
}
