use std::{error::Error, fs, time};

use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum};
use tomboy_emulator::{cpu::Cpu, joypad};

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

  let rom = fs::read("./tests/roms/dmg-acid2.gb")?;
  let mut emu = Cpu::new(&rom);

  let texture_creator = canvas.texture_creator();
  let mut texture = texture_creator
    .create_texture_target(PixelFormatEnum::RGBA32, WIN_WIDTH, WIN_HEIGHT)?;


  'running: loop {
    let ms_since_frame_start = time::Instant::now();

    while emu.bus.ppu.vblank.take().is_none() {
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
              Keycode::Up => { emu.bus.joypad.dpad_pressed(joypad::Flags::select_up); }
              Keycode::Down => { emu.bus.joypad.dpad_pressed(joypad::Flags::start_down); }
              Keycode::Left => { emu.bus.joypad.dpad_pressed(joypad::Flags::b_left ); }
              Keycode::Right => { emu.bus.joypad.dpad_pressed(joypad::Flags::a_right ); }
              Keycode::Z => { emu.bus.joypad.button_pressed(joypad::Flags::a_right ); }
              Keycode::X => { emu.bus.joypad.button_pressed(joypad::Flags::b_left); }
              Keycode::M => { emu.bus.joypad.button_pressed(joypad::Flags::start_down); }
              Keycode::N => { emu.bus.joypad.button_pressed(joypad::Flags::select_up); }
              _ => {}
            }
          }
        }
        Event::KeyUp { keycode, .. } => {
          if let Some(keycode) = keycode {
            match keycode {
              Keycode::Up => { emu.bus.joypad.dpad_released(joypad::Flags::select_up); }
              Keycode::Down => { emu.bus.joypad.dpad_released(joypad::Flags::start_down); }
              Keycode::Left => { emu.bus.joypad.dpad_released(joypad::Flags::b_left ); }
              Keycode::Right => { emu.bus.joypad.dpad_released(joypad::Flags::a_right ); }
              Keycode::Z => { emu.bus.joypad.button_released(joypad::Flags::a_right ); }
              Keycode::X => { emu.bus.joypad.button_released(joypad::Flags::b_left); }
              Keycode::M => { emu.bus.joypad.button_released(joypad::Flags::start_down); }
              Keycode::N => { emu.bus.joypad.button_released(joypad::Flags::select_up); }
              _ => {}
            }
          }
        }
        _ => {}
      }

    }
    
    canvas.clear();
    texture.update(None, &emu.bus.ppu.lcd.buffer, emu.bus.ppu.lcd.pitch())?;
    canvas.copy(&texture, None, None)?;
    canvas.present();

    let ms_elapsed = time::Instant::now() - ms_since_frame_start;
    if ms_frame > ms_elapsed {
      std::thread::sleep(ms_frame - ms_elapsed);
    }
  }

  Ok(())
}
