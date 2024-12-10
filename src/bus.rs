use std::{cell::{Cell, RefCell}, rc::Rc};

use crate::{ppu, timer::Timer};
use bitflags::bitflags;

bitflags! {
  #[derive(PartialEq, Clone, Copy, Debug)]
  pub struct IFlags: u8 {
    const joypad = 0b0001_0000;
    const serial = 0b0000_1000;
    const timer  = 0b0000_0100;
    const lcd    = 0b0000_0010;
    const vblank = 0b0000_0001;
  }
}

pub type SharedBus = Rc<RefCell<Bus>>;
pub type InterruptFlags = Rc<Cell<IFlags>>;
pub struct Bus {
	pub mem: [u8; 0x10000],
  pub ppu_regs: ppu::Registers,
  pub timer: Timer,
  pub inte: IFlags,
  pub intf: InterruptFlags,
}

enum BusTarget {
  Rom, VRam, ExRam, WRam, Oam, Unused, Ppu, Timer, IO, HRam, IF, IE,
}

#[allow(unused)]
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
    0xFF04..=0xFF07 => (Timer, addr),
    0xFF0F => (IF, addr),
    0xFF00..=0xFF7F => (IO, addr),
    0xFF80..=0xFFFE => (HRam, addr - 0xFF80),
    0xFFFF => (IE, addr),
  }
}

pub fn send_interrupt(intf: &Cell<IFlags>, int: IFlags) {
  let mut flags = intf.get();
  flags.insert(int);
  intf.set(flags);
}

impl Bus {
  pub fn new() -> SharedBus {
    let intf = Rc::new(Cell::new(IFlags::empty()));

    let bus = Self { 
      mem: [0; 0x10000],
      ppu_regs: ppu::Registers::default(),
      timer: Timer::new(intf.clone()),
      inte: IFlags::empty(), 
      intf,
    };

    Rc::new(RefCell::new(bus))
  }

  pub fn read(&self, addr: u16) -> u8 {
    // match addr {
    //   0xE000..=0xFDFF => self.mem[addr as usize & 0xDFFF],
    //   0xFF04..=0xFF07 => self.timer.read_reg(addr),
    //   0xFF40..=0xFF4B => self.ppu_regs.read(addr),
    //   0xFF0F => self.intf.get().bits(),
    //   0xFFFF => self.inte.bits(),
    //   _ => self.mem[addr as usize],
    // }

    self.mem[addr as usize]
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    // match addr {
    //   0x0000..=0x7FFF => eprintln!("Illegal write to ROM"),
    //   0xE000..=0xFDFF => self.mem[addr as usize & 0xDFFF] = val,
    //   0xFF04..=0xFF07 => self.timer.write_reg(addr, val),
    //   0xFF40..=0xFF4B => self.ppu_regs.write(addr, val),
    //   0xFF0F => self.intf.set(IFlags::from_bits_truncate(val & 0b1_1111)),
    //   0xFFFF => self.inte = IFlags::from_bits_truncate(val & 0b1_1111),
    //   _ => self.mem[addr as usize] = val,
    // }

    self.mem[addr as usize] = val;
  }

  pub fn intf(&self) -> IFlags {
    self.intf.get()
  }

  pub fn set_intf(&self, val: IFlags) {
    self.intf.set(val);
  }
}