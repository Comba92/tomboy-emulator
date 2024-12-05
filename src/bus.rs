use crate::{ppu::Ppu, timer::Timer};
use bitflags::bitflags;

bitflags! {
  #[derive(PartialEq, Debug)]
  pub struct IFlags: u8 {
    const joypad = 0b0001_0000;
    const serial = 0b0000_1000;
    const timer  = 0b0000_0100;
    const lcd    = 0b0000_0010;
    const vblank = 0b0000_0001;
  }
}

pub struct Bus {
	pub mem: [u8; 0x10000],
  pub ppu: Ppu,
  pub timer: Timer,
  pub inte: IFlags,
  pub intf: IFlags,
}

enum BusTarget {
  Rom, VRam, ExRam, WRam, Oam, Unused, Ppu, IO, HRam, IF, IE,
}

fn map_addr(addr: u16) -> (BusTarget, u16) {
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
    0xFF0F => (IF, addr),
    0xFF00..=0xFF7F => (IO, addr),
    0xFF80..=0xFFFE => (HRam, addr - 0xFF80),
    0xFFFF => (IE, addr),
  }
}

impl Bus {
  pub fn new() -> Self {
    Self { 
      mem: [0; 0x10000], 
      ppu: Ppu::default(), 
      timer: Timer::default(),
      inte: IFlags::empty(), 
      intf: IFlags::empty(), 
    }
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    // let (dst, addr) = map_addr(addr);
    match addr {
      // BusTarget::Ppu => self.ppu.read_reg(addr),
      0xFF04..=0xFF07 => self.timer.read_reg(addr),
      0xFF0F => self.intf.bits(),
      0xFFFF => self.inte.bits(),
      _ => self.mem[addr as usize],
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    // let (dst, addr) = map_addr(addr);
    match addr {
      // BusTarget::Ppu => self.ppu.write_reg(addr, val),
      0xFF04..=0xFF07 => self.timer.write_reg(addr, val),
      0xFF0F => {
        println!("Wrote to IF");
        self.intf = IFlags::from_bits_truncate(val & 0b1_1111);
      }
      0xFFFF => self.inte = IFlags::from_bits_truncate(val & 0b1_1111),
      _ => self.mem[addr as usize] = val,
    }
  }
}