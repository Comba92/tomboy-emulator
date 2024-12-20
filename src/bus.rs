use std::{cell::{Cell, RefCell}, num::Wrapping, rc::Rc};

use crate::{cart::Cart, joypad::Joypad, mapper::{self, Mapper}, ppu, timer::Timer};
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
  rom: Vec<u8>,
  ram: [u8; 8*1024],
  exram: Vec<u8>,
  hram: [u8; 0x7F],
  pub vram: [u8; 8*1024],
  oam: [u8; 160],

  pub ppu_regs: ppu::Registers,
  
  mapper: Box<dyn Mapper>,
  cart: Cart,
  pub timer: Timer,
  pub joypad: Joypad,

  pub inte: IFlags,
  pub intf: InterruptFlags,
}


enum BusTarget {
  Rom, VRam, ExRam, WRam, Oam, Unusable, 
  Joypad, Ppu, Timer, NoImpl, HRam, IF, IE,
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
    0xFEA0..=0xFEFF => (Unusable, addr),
    0xFF00 => (Joypad, addr),
    0xFF04..=0xFF07 => (Timer, addr),
    0xFF0F => (IF, addr),
    0xFF40..=0xFF4B => (Ppu, addr),
    0xFF01..=0xFF7F => (NoImpl, addr),
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
  pub fn new(rom: &[u8]) -> SharedBus {
    let intf = Rc::new(Cell::new(IFlags::empty()));
    let cart = Cart::new(rom).unwrap();

    let bus = Self {
      rom: Vec::from(rom),
      ram:   [0; 8*1024],
      exram: Vec::from([0].repeat(cart.ram_size)),
      hram: [0; 0x7F],
      vram: [0; 8*1024],
      oam:  [0; 160],

      ppu_regs: ppu::Registers::default(),

      mapper: mapper::get_mapper(cart.mapper_code),
      cart,
      timer: Timer::new(intf.clone()),
      joypad: Joypad::new(intf.clone()),
      inte: IFlags::empty(), 
      intf,
    };

    Rc::new(RefCell::new(bus))
  }

  // TODO: return 0xff is ppu is enabled
  pub fn read(&self, addr: u16) -> u8 {
    let (target, addr) = map_addr(addr);
    use BusTarget::*;
    match &target {
      Rom => self.mapper.read_rom(&self.rom, addr),
      VRam => self.vram[addr as usize],
      ExRam => self.mapper.read_ram(&self.exram, addr),
      WRam => self.ram[addr as usize],
      Oam => self.oam[addr as usize],
      Unusable => 0,
      Joypad => self.joypad.read(),
      Ppu => self.ppu_regs.read(addr),
      Timer => self.timer.read_reg(addr),
      IF => self.intf.get().bits(),
      HRam => self.hram[addr as usize],
      IE => self.inte.bits(),
      NoImpl => 0,
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    let (target, addr) = map_addr(addr);
    use BusTarget::*;
    match &target {
      Rom => self.mapper.write_rom(&mut self.rom, addr, val),
      VRam => self.vram[addr as usize] = val,
      ExRam => self.mapper.write_ram(&mut self.exram, addr, val),
      WRam => self.ram[addr as usize] = val,
      Oam => self.oam[addr as usize] = val,
      Unusable => {}
      Joypad => self.joypad.write(val),
      Ppu => self.ppu_regs.write(addr, val),
      Timer => self.timer.write_reg(addr, val),
      IF => self.intf.set(IFlags::from_bits_truncate(val)),
      HRam => self.hram[addr as usize] = val,
      IE => self.inte = IFlags::from_bits_truncate(val),
      NoImpl => {},
    }
  }

  pub fn intf(&self) -> IFlags {
    self.intf.get()
  }

  pub fn set_intf(&self, val: IFlags) {
    self.intf.set(val);
  }
}