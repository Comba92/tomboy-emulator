pub trait Memory {
  fn read(&mut self, addr: u16) -> u8;
  fn write(&mut self, addr: u16, val: u8);
  fn tick(&mut self);
  fn halt_tick(&mut self);
  fn has_pending_interrupts(&self) -> bool;
}

pub struct Ram64kb {
  mem: [u8; 64 * 1024]
}

impl Default for Ram64kb {
  fn default() -> Self {
    Self { mem: [0; 64 * 1024] }
  }
}

impl Memory for Ram64kb {
  fn read(&mut self, addr: u16) -> u8 { self.mem[addr as usize] }
  fn write(&mut self, addr: u16, val: u8) { self.mem[addr as usize] = val; }
  fn tick(&mut self) {}
  fn halt_tick(&mut self) {}
  fn has_pending_interrupts(&self) -> bool { false }
}