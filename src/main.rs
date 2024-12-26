use std::{error::Error, fs, time};

use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum};
use tomboy_emulator::{cart::Cart, cpu::Cpu, joypad};

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

  let rom = fs::read("./tests/roms/01-special.gb")?;
  // let rom = fs::read("./roms/Tetris.gb")?;
  // let rom = fs::read("./bootroms/dmg_boot.bin")?;
  
  let mut emu = Cpu::new(&rom);

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
          let rom = fs::read(filename)?;
          emu = Cpu::new(&rom);
        }
        Event::KeyDown { keycode, .. } => {
          if let Some(keycode) = keycode {
            match keycode {
              Keycode::Up => { emu.bus.borrow_mut().joypad.dpad_pressed(joypad::Flags::select_up); }
              Keycode::Down => { emu.bus.borrow_mut().joypad.dpad_pressed(joypad::Flags::start_down); }
              Keycode::Left => { emu.bus.borrow_mut().joypad.dpad_pressed(joypad::Flags::b_left ); }
              Keycode::Right => { emu.bus.borrow_mut().joypad.dpad_pressed(joypad::Flags::a_right ); }
              Keycode::Z => { emu.bus.borrow_mut().joypad.button_pressed(joypad::Flags::a_right ); }
              Keycode::X => { emu.bus.borrow_mut().joypad.button_pressed(joypad::Flags::b_left); }
              Keycode::M => { emu.bus.borrow_mut().joypad.button_pressed(joypad::Flags::start_down); }
              Keycode::N => { emu.bus.borrow_mut().joypad.button_pressed(joypad::Flags::select_up); }
              _ => {}
            }
          }
        }
        Event::KeyUp { keycode, .. } => {
          if let Some(keycode) = keycode {
            match keycode {
              Keycode::Up => { emu.bus.borrow_mut().joypad.dpad_released(joypad::Flags::select_up); }
              Keycode::Down => { emu.bus.borrow_mut().joypad.dpad_released(joypad::Flags::start_down); }
              Keycode::Left => { emu.bus.borrow_mut().joypad.dpad_released(joypad::Flags::b_left ); }
              Keycode::Right => { emu.bus.borrow_mut().joypad.dpad_released(joypad::Flags::a_right ); }
              Keycode::Z => { emu.bus.borrow_mut().joypad.button_released(joypad::Flags::a_right ); }
              Keycode::X => { emu.bus.borrow_mut().joypad.button_released(joypad::Flags::b_left); }
              Keycode::M => { emu.bus.borrow_mut().joypad.button_released(joypad::Flags::start_down); }
              Keycode::N => { emu.bus.borrow_mut().joypad.button_released(joypad::Flags::select_up); }
              _ => {}
            }
          }
        }
        _ => {}
      }

    }
    
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
