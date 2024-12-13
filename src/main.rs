use std::{error::Error, fs, time};

use sdl2::{event::Event, pixels::PixelFormatEnum};
use tomboy_emulator::{cart::Cart, cpu::Cpu};

fn main() -> Result<(), Box<dyn Error>> {
  let sdl = sdl2::init()?;
  let video = sdl.video()?;
  let ms_frame = time::Duration::from_secs_f64(1.0 / 60.0);

  const WIN_WIDTH: u32 = 20*8;
  const WIN_HEIGHT: u32 = 18*8;

  let mut canvas = video.window("TomboyEmu", WIN_WIDTH*4, WIN_HEIGHT*4)
    .position_centered().build()?.into_canvas()
    .accelerated().target_texture().present_vsync()
    .build()?;

  let mut events = sdl.event_pump()?;

  let mut emu = Cpu::new();
  let rom = fs::read("./tests/roms/dmg-acid2.gb")?;
  // let rom = fs::read("./roms/Tetris.gb")?;
  // let rom = fs::read("./bootroms/dmg_boot.bin")?;

  let cart = Cart::new(&rom);
  println!("{:?}", cart);

  let mut bus = emu.bus.borrow_mut();
  let (left, _) = bus.mem.split_at_mut(rom.len());
  left.copy_from_slice(&rom);
  drop(bus);

  let texture_creator = canvas.texture_creator();
  let mut texture = texture_creator
    .create_texture_target(PixelFormatEnum::RGBA32, WIN_WIDTH, WIN_HEIGHT)?;

  // let mut framebuf = FrameBuffer::new(WIN_WIDTH as usize, WIN_HEIGHT as usize);

  'running: loop {
    let ms_since_frame_start = time::Instant::now();

    while emu.ppu.vblank.take().is_none() {
      emu.step();
    }

    for event in events.poll_iter() {
      match event {
        Event::Quit { .. } => break 'running,
        Event::DropFile { filename, .. } => {
          emu = Cpu::new();

          let rom = fs::read(filename)?;
          let mut bus = emu.bus.borrow_mut();
          let (left, _) = bus.mem.split_at_mut(rom.len());
          left.copy_from_slice(&rom);
          drop(bus);
        }
        _ => {}
      }

    }
    
    // for i in 0..384 {
    //   let x = i % (WIN_WIDTH as usize/16);
    //   let y = i / (WIN_WIDTH as usize/16);
    //   let tile_start = 0x8000 + i*16;
    //   let tile = &emu.bus.mem[tile_start..tile_start+16];
    //   framebuf.set_tile(x*16, y*16, &tile);
    // }

    // for i in 0..20*18 {
    //   let x = i % 20;
    //   let y = i / 20;

    //   let tile_id = emu.read(0x9800 + y*20 + x);
    //   let tile_addr = emu.ppu.tile_addr(tile_id) as usize;
    //   let tile = &emu.bus.borrow().mem[tile_addr..tile_addr+16];
    //   framebuf.set_tile(x as usize*8, y as usize*8, &tile);
    // }
    
    canvas.clear();
    texture.update(None, &emu.ppu.lcd.buffer, emu.ppu.lcd.pitch())?;
    canvas.copy(&texture, None, None)?;
    canvas.present();

    let ms_elapsed = time::Instant::now() - ms_since_frame_start;
    if ms_frame > ms_elapsed {
      std::thread::sleep(ms_frame - ms_elapsed);
    }
  }

  Ok(())
}
