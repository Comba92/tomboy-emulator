use crate::ppu::Ppu;

pub struct Bus {
	pub mem: [u8; 0x10000],
  ppu: Ppu
}

enum BusTarget {
  Rom, VRam, ExRam, WRam, Oam, Unused, Ppu, IO, HRam, IE,
}

impl Bus {
  pub fn new() -> Self {
    Self { mem: [0; 0x10000], ppu: Ppu::default() }
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    let (dst, addr) = self.map(addr);
    match dst {
      BusTarget::Ppu => self.ppu.read_reg(addr),
      _ => self.mem[addr as usize],
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    let (dst, addr) = self.map(addr);
    match dst {
      BusTarget::Ppu => self.ppu.write_reg(addr, val),
      _ => self.mem[addr as usize] = val,
    }
  }

  fn map(&self, addr: u16) -> (BusTarget, u16) {
    use BusTarget::*;
    match addr {
      0x0000..=0x7FFF => (Rom, addr),
      0x8000..=0x9FFF => (VRam, addr - 0x8000),
      0xA000..=0xBFFF => (ExRam, addr - 0xA000),
      0xC000..=0xDFFF => (WRam, addr - 0xC000),
      0xE000..=0xFDFF => (WRam, (addr & 0xDFFF) - 0xC000),
      0xFE00..=0xFE9F => (Oam, addr - 0xFE00),
      0xFEA0..=0xFEFF => (Unused, addr),
      0xFF40..=0xFF4B => (Ppu, addr),
      0xFF00..=0xFF7F => (IO, addr),
      0xFF80..=0xFFFE => (HRam, addr - 0xFF80),
      0xFFFF => (IE, addr),
    }
  }
}