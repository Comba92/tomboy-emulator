use crate::{apu::Apu, bus::Bus, cart::CartHeader, cpu::Cpu, frame::FrameBuffer, joypad::Joypad, mbc::Cart, ppu::Ppu};

pub struct Gameboy {
  cpu: Cpu
}

impl Gameboy {
  pub fn boot_from_bytes(rom: &[u8]) -> Result<Self, String> {
    let cart = Cart::new(rom)?;
    Ok(Self {cpu: Cpu::new(cart)})
  }

  pub fn step(&mut self) {
    self.get_cpu().step();
  }

  pub fn step_until_vblank(&mut self) {
    loop {
      if self.get_ppu().frame_ready.take().is_some() { break; }
      self.step();
    }
  }

  pub fn reset(&mut self) {}
}

impl Gameboy {
  pub fn get_bus(&mut self) -> &mut Bus {
    &mut self.cpu.bus
  }

  pub fn get_cpu(&mut self) -> &mut Cpu {
    &mut self.cpu
  }

  pub fn get_ppu(&mut self) -> &mut Ppu {
    &mut self.cpu.bus.ppu
  }

  pub fn get_apu(&mut self) -> &mut Apu {
    &mut self.cpu.bus.apu
  }

  pub fn get_cart(&self) -> CartHeader {
    self.cpu.bus.cart.header.clone()
  }

  pub fn get_resolution(&mut self) -> (usize, usize) { (32*8, 30*8) }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.lcd
  }

  pub fn get_samples(&mut self) -> Vec<f32> {
    Default::default()
  }

  pub fn get_joypad(&mut self) -> &mut Joypad {
    &mut self.cpu.bus.joypad
  }
}