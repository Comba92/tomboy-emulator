use crate::{bus::{self, InterruptFlags}, frame::FrameBuffer};
use bitflags::bitflags;

bitflags! {
  #[derive(Default, Clone, Copy)]
  pub struct Ctrl: u8 {
    const bg_wnd_enabled = 0b0000_0001;
    const obj_enabled    = 0b0000_0010;
    const obj_size   = 0b0000_0100;
    const bg_tilemap = 0b0000_1000;

    const tileset_addr = 0b0001_0000;
    const wnd_enabled  = 0b0010_0000;
    const wnd_tilemap  = 0b0100_0000;
    const lcd_enabled  = 0b1000_0000;
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
enum PpuMode {
  Hblank, // Mode0
  Vblank, // Mode1
  #[default]
  OamScan, // Mode2
  DrawingPixels, // Mode3
}

pub struct Ppu {
  pub lcd: FrameBuffer,

  pub vram: [u8; 8*1024],
  pub oam: [u8; 160],

  mode: PpuMode,
  pub vblank: Option<()>,

  ctrl: Ctrl,
  stat: Stat,
  ppu_enabled: bool,
  vram_enabled: bool,
  oam_enabled: bool,
  ly: u8,
  lyc: u8,
  scy: u8,
  scx: u8,
  wy: u8,
  wx: u8,
  bgp: u8,
  obp0: u8,
  obp1: u8,

  tcycles: u16,
  intf: InterruptFlags,
}

impl Ppu {
  pub fn new(intf: InterruptFlags) -> Self {
    Self {
      lcd: FrameBuffer::gameboy_lcd(),
      vram: [0; 8*1024],
      oam: [0; 160],

      mode: Default::default(),
      vblank: None,

      ctrl: Ctrl::empty(),
      stat: Stat::empty(),
      ppu_enabled: false,
      vram_enabled: false,
      oam_enabled: false,
      ly: 0,
      lyc: 0,
      scy: 0,
      scx: 0,
      wy: 0,
      wx: 0,
      bgp: 0,
      obp0: 0,
      obp1: 0,

      tcycles: Default::default(), 
      intf,
    }
  }

  pub fn tick(&mut self) {
    use PpuMode::*;
    
    self.tcycles += 1;
    if self.tcycles > 456 {
      self.tcycles = 0;
      self.ly_inc();
    }

    match self.mode {
      OamScan => {
        if self.tcycles >= 80 {
          self.mode = DrawingPixels;
        }
      }
      DrawingPixels => {
        // if self.render.scaline_x >= 160 {
        if self.tcycles > 80 + 172 {
          self.mode = Hblank;

          self.send_lcd_int(Stat::mode0_int);
        } else {
          // self.bg_drawing_step();
        }
      }
      Hblank => {
        if self.tcycles >= 456 {
          if self.ly > 143 {
            self.mode = Vblank;
            self.send_vblank_int();
            self.send_lcd_int(Stat::mode1_int);
          } else {
            self.mode = OamScan;
          };
        }
      }
      Vblank => {
        if self.ly >= 154 {
          self.mode = OamScan;
          self.ly_reset();
          self.send_lcd_int(Stat::mode2_int);
        }
      }
    };
  }

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
      0xFF47 => self.bgp,
      0xFF48 => self.obp0,
      0xFF49 => self.obp1,
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
      0xFF47 => self.bgp = val,
      0xFF48 => self.obp0 = val,
      0xFF49 => self.obp1 = val,
      _ => eprintln!("Ppu register write {addr:04X} not implemented"),
    }
  }

  fn send_vblank_int(&mut self) {
    bus::send_interrupt(&self.intf, bus::IFlags::vblank);
    self.vblank = Some(());
  }

  fn send_lcd_int(&mut self, flag: Stat) {
    if self.stat.contains(flag) {
      bus::send_interrupt(&self.intf, bus::IFlags::lcd);
    }
  }

  pub fn is_ppu_enabled(&self) -> bool {
    self.ctrl.contains(Ctrl::lcd_enabled)
  }
}