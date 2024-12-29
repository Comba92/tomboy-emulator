use std::{cell::Cell, rc::Rc};

use crate::{apu::Apu, joypad::Joypad, mbc::Cart, ppu::Ppu, timer::Timer};
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

#[derive(Default)]
struct Dma {
	start: u16,
	offset: u16,
}
impl Dma {
	pub fn init(&mut self, val: u8) {
		self.start = (val as u16) << 8;
		self.offset = 160;
	}

  pub fn current(&self) -> u16 {
    self.start.wrapping_add(self.offset())
  }

  pub fn offset(&self) -> u16 {
    160-self.offset
  }

  pub fn advance(&mut self) {
    self.offset -= 1;
  }

  pub fn is_transferring(&self) -> bool {
    self.offset > 0
  }
}

pub type InterruptFlags = Rc<Cell<IFlags>>;
pub struct Bus {
  ram: [u8; 8*1024],
  hram: [u8; 0x7F],
  dma: Dma,
  
  pub cart: Cart,
  pub ppu: Ppu,
  pub timer: Timer,
  pub joypad: Joypad,
  pub apu: Apu,

  pub inte: IFlags,
  pub intf: InterruptFlags,
}

enum BusTarget {
  Rom, VRam, OamDma, ExRam, WRam, Oam, Unusable, 
  Joypad, Ppu, Apu, Timer, NoImpl, HRam, IF, IE,
}

#[allow(unused)]
fn map_addr(addr: u16) -> (BusTarget, u16) {
  use BusTarget::*;
  match addr {
    0x0000..=0x7FFF => (Rom, addr),
    0x8000..=0x9FFF => (VRam, addr - 0x8000),
    0xA000..=0xBFFF => (ExRam, addr),
    0xC000..=0xDFFF => (WRam, addr - 0xC000),
    0xE000..=0xFDFF => (WRam, (addr & 0xDFFF) - 0xC000),
    0xFE00..=0xFE9F => (Oam, addr - 0xFE00),
    0xFEA0..=0xFEFF => (Unusable, addr),
    0xFF00 => (Joypad, addr),
    0xFF04..=0xFF07 => (Timer, addr),
    0xFF0F => (IF, addr),
    0xFF10..=0xFF3F => (Apu, addr),
    0xFF46 => (OamDma, addr),
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
  pub fn new(rom: &[u8]) -> Bus {
    let intf = Rc::new(Cell::new(IFlags::empty()));
    let cart = Cart::new(rom).unwrap();

    Self {
      ram: [0; 8*1024],
      hram: [0; 0x7F],
      dma: Dma::default(),

      cart,
      ppu: Ppu::new(intf.clone()),
      apu: Apu::default(),
      timer: Timer::new(intf.clone()),
      joypad: Joypad::new(intf.clone()),
      inte: IFlags::empty(), 
      intf,
    }
  }

  pub fn tick(&mut self) {
    for _ in 0..4 { self.ppu.tick(); }
    self.timer.tick();
  }

  pub fn handle_dma(&mut self) {
    if self.dma.is_transferring() {
      let addr = self.dma.current();
      let val = self.read(addr);
      self.write(0xFE00 + self.dma.offset(), val);
      
      self.dma.advance();
      self.tick();
    }
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    let (target, addr) = map_addr(addr);
    use BusTarget::*;
    match &target {
      Rom => self.cart.rom_read(addr),
      VRam => self.ppu.vram[addr as usize],
      ExRam => self.cart.ram_read(addr),
      WRam => self.ram[addr as usize],
      Oam => self.ppu.oam[addr as usize],
      Unusable => 0,
      Joypad => self.joypad.read(),
      Apu => self.apu.read(addr),
      Ppu => self.ppu.read(addr),
      Timer => self.timer.read(addr),
      IF => self.intf.get().bits(),
      HRam => self.hram[addr as usize],
      IE => self.inte.bits(),
      _ => 0,
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    let (target, addr) = map_addr(addr);
    use BusTarget::*;
    match &target {
      Rom => self.cart.rom_write(addr, val),
      VRam => self.ppu.vram[addr as usize] = val,
      ExRam => self.cart.ram_write(addr, val),
      WRam => self.ram[addr as usize] = val,
      Oam => self.ppu.oam[addr as usize] = val,
      Unusable => {}
      Joypad => self.joypad.write(val),
      Apu => self.apu.write(addr, val),
      Ppu => self.ppu.write(addr, val),
      OamDma => {
        self.dma.init(val);
        self.tick();
      }
      Timer => self.timer.write(addr, val),
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