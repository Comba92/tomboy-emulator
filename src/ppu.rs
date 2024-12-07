use bitflags::bitflags;
use crate::{bus::{IFlags, SharedBus}, frame::FrameBuffer};

bitflags! {
  #[derive(Default, Clone, Copy)]
  pub struct Ctrl: u8 {
    const bg_wind_on = 0b0000_0001;
    const obj_on     = 0b0000_0010;
    const obj_size   = 0b0000_0100;
    const bg_tilemap = 0b0000_1000;

    const tiles_addr   = 0b0001_0000;
    const window_on    = 0b0010_0000;
    const wind_tilemap = 0b0100_0000;
    const ppu_on       = 0b1000_0000;
  }

  #[derive(Default, Clone, Copy)]
  pub struct Stat: u8 {
    const ppu_mode  = 0b0000_0011;
    const lyc_eq_ly = 0b0000_0100;
    const mode0_int = 0b0000_1000;
    const mode1_int = 0b0001_0000;
    const mode2_int = 0b0010_0000;
    const lyc_int   = 0b0100_0000;
  }
}

#[derive(Default)]
pub struct Registers {
  ctrl: Ctrl,

  ly: u8,
  lyc: u8,
  stat: Stat,

  scy: u8,
  scx: u8,
  wy: u8,
  wx: u8,
  bg_palette: u8,
}

const CTRL: u16 = 0xFF40;
const STAT: u16 = 0xFF41;
const SCY: u16 = 0xFF42;
const SCX: u16 = 0xFF43;
const LY: u16 = 0xFF44;
const LYC: u16 = 0xFF45;
const WY: u16 = 0xFF4A;
const WX: u16 = 0xFF4B;
const PALETTE: u16 = 0xFF47;

impl Registers {
  pub fn read(&self, addr: u16) -> u8 {
    match addr {
      0xFF40 => self.ctrl.bits(),
      0xFF41 => self.stat.bits(),
      0xFF42 => self.scy,
      0xFF43 => self.scx,
      0xFF44 => self.ly,
      0xFF45 => self.lyc,
      0xFF4A => self.wy,
      0xFF4B => self.wx,
      0xFF47 => self.bg_palette,
      _ => {
        eprintln!("Ppu register read {addr:04X} not implemented");
        0
      }
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0xFF40 => self.ctrl = Ctrl::from_bits_retain(val),
      0xFF41 => self.stat = Stat::from_bits_retain(val),
      0xFF42 => self.scy = val,
      0xFF43 => self.scx = val,
      0xFF44 => self.ly = val,
      0xFF45 => self.lyc = val,
      0xFF4A => self.wy = val,
      0xFF4B => self.wx = val,
      0xFF47 => self.bg_palette = val,
      _ => eprintln!("Ppu register write {addr:04X} not implemented"),
    }
  }
}


#[derive(Default)]
enum PpuMode {
  #[default]
  Hblank, // Mode0
  Vblank, // Mode1
  OamScan, // Mode2
  DrawingPixels, // Mode3
}

pub struct Ppu {
  pub lcd: FrameBuffer,
  bg_scanline: [u8; 160],
  wnd_scanline: [u8; 160],
  spr_scanline: [u8; 160],

  mode: PpuMode,
  pub vblank: Option<()>,

  tcycles: usize,
  scanlines: usize,
  bus: SharedBus,
}

impl Ppu {
  pub fn new(bus: SharedBus) -> Self {
    Self {
      lcd: FrameBuffer::gameboy_lcd(), 
      bg_scanline:  [0; 160],
      wnd_scanline: [0; 160],
      spr_scanline: [0; 160],

      mode: Default::default(),
      vblank: None,

      tcycles: Default::default(), 
      scanlines: Default::default(), 
      bus,
    }
  }

  pub fn tick(&mut self) {
    self.tcycles += 1;
    if self.tcycles > 456 {
      self.tcycles = 0;
      self.scanlines += 1;
      self.set_ly(self.read(LY) + 1);

      if self.scanlines == 144 {
        self.bus.borrow_mut().intf.insert(IFlags::vblank);
        self.vblank = Some(());
      }
      if self.scanlines > 154 {
        self.scanlines = 0;
        self.set_ly(0);
      }
    }
  }

  fn render_bg(&mut self) {
    for y in 0..144/8 {
      for x in 0..160/8 {
        let tilemap_addr = self.bg_tilemap() 
          + ((self.read(SCY) as u16/8 + y) % 256) * (144/8)
          + ((self.read(SCX) as u16/8 + x) % 256);

        let tile_id = self.read(tilemap_addr);
        let tileset_addr = self.tile_addr(tile_id) as usize;
        let tile = &self.bus.borrow().mem[tileset_addr..tileset_addr+16];

        self.lcd.set_tile(8*x as usize, 8*y as usize, tile);
      }
    }
  }

  pub fn read(&self, addr: u16) -> u8 {
    self.bus.borrow().read(addr)
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    self.bus.borrow_mut().write(addr, val);
  }

  fn ctrl(&self) -> Ctrl {
    self.bus.borrow().ppu_regs.ctrl
  }

  fn bg_tilemap(&self) -> u16 {
    match self.ctrl().contains(Ctrl::bg_tilemap) {
      false => 0x9800,
      true => 0x9C00,
    }
  }

  fn stat(&self) -> Stat {
    self.bus.borrow().ppu_regs.stat
  }

  fn set_stat(&mut self, val: Stat) {
    self.bus.borrow_mut().ppu_regs.stat = val;
  }

  fn set_ly(&mut self, val: u8) {
    self.bus.borrow_mut().ppu_regs.ly = val;
  }

  pub fn tile_addr(&self, tile_id: u8) -> u16 {
    match self.ctrl().contains(Ctrl::tiles_addr) {
      true  => 0x8000 + 16*tile_id as u16,
      false => {
        let offset = tile_id as i8;
        (0x9000 + 16*offset as i32) as u16
      }
    }
  }
}