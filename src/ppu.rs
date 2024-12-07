use bitflags::bitflags;

use crate::frame::FrameBuffer;

bitflags! {
  #[derive(Default)]
  struct Ctrl: u8 {
    const bg_wind_on = 0b0000_0001;
    const obj_on     = 0b0000_0010;
    const obj_size   = 0b0000_0100;
    const bg_tilemap = 0b0000_1000;

    const tiles_addr   = 0b0001_0000;
    const window_on    = 0b0010_0000;
    const wind_tilemap = 0b0100_0000;
    const ppu_on       = 0b1000_0000;
  }

  #[derive(Default)]
  struct Stat: u8 {
    const ppu_mode  = 0b0000_0011;
    const lyc_eq_ly = 0b0000_0100;
    const mode0_int = 0b0000_1000;
    const mode1_int = 0b0001_0000;
    const mode2_int = 0b0010_0000;
    const lyc_int   = 0b0100_0000;
  }
}

pub struct Ppu {
  lcd: FrameBuffer,
  bg_scanline: [u8; 160],
  wnd_scanline: [u8; 160],
  spr_scanline: [u8; 160],

  mode: PpuMode,
  ctrl: Ctrl,
  ly: u8,
  lyc: u8,
  stat: Stat,
  scy: u8,
  scx: u8,
  wy: u8,
  wx: u8,
  bg_palette: u8,

  pub vblank_request: Option<()>,
  pub stat_request: Option<()>,

  tcycles: u8,
  scanlines: u8,
}

impl Default for Ppu {
  fn default() -> Self {
    Self { 
      lcd: FrameBuffer::gameboy_lcd(), 
      bg_scanline: [0; 160],
      wnd_scanline: [0; 160],
      spr_scanline: [0; 160],

      mode: Default::default(),
      ctrl: Default::default(), 
      ly: Default::default(),
      lyc: Default::default(),
      stat: Default::default(), 
      scy: Default::default(), 
      scx: Default::default(), 
      wy: Default::default(), 
      wx: Default::default(), 
      bg_palette: Default::default(), 
      tcycles: Default::default(), 
      scanlines: Default::default(), 
      stat_request: None, 
      vblank_request: None
    }
  }
}

#[derive(Default)]
enum PpuMode {
  #[default]
  Mode2, // OAM Scan
  Mode3, // Drawing pixels
  Mode0, // Hblank
  Mode1, // Vblank
}

impl Ppu {
  pub fn step(&mut self) {
    self.tcycles += 1;
    if self.tcycles == 80 {
      self.mode = PpuMode::Mode3;
    } else if self.tcycles == 172 {
      self.mode = PpuMode::Mode0;
    }

    if self.tcycles > 204 {
      self.tcycles = 0;
      self.scanlines += 1;
      
      if self.ly == self.lyc {
        self.stat_request = Some(());
      }

      if self.scanlines < 144 {
        self.ly += 1;
      }

      if self.scanlines == 144 {
        self.mode = PpuMode::Mode1;
        self.vblank_request = Some(());
      } else if self.scanlines > 153 {
        self.scanlines = 0;
        self.mode = PpuMode::Mode2;
      }
    }
  }


  pub fn tile_addr(&self, tile_id: u8) -> u16 {
    match self.ctrl.contains(Ctrl::tiles_addr) {
      true  => 0x8000 + 16*tile_id as u16,
      false => {
        let offset = tile_id as i8;
        (0x9000 + 16*offset as i32) as u16
      }
    }
  }

  pub fn read_reg(&mut self, addr: u16) -> u8 {
    match addr {
      0xFF40 => self.ctrl.bits(),
      0xFF41 => self.stat.bits(),
      0xFF42 => self.scy,
      0xFF43 => self.scx,
      0xFF44 => 0x90, // self.ly,
      0xFF45 => self.lyc,
      0xFF4A => self.wy,
      0xFF4B => self.wx,
      0xFF47 => self.bg_palette,
      _ => todo!("Ppu register read {addr:04X} not implemented"),
    }
  }

  pub fn write_reg(&mut self, addr: u16, val: u8) {
    match addr {
      0xFF40 => self.ctrl = Ctrl::from_bits_retain(val),
      0xFF41 => self.stat = Stat::from_bits_retain(val),
      0xFF42 => self.scy = val,
      0xFF43 => self.scx = val,
      0xFF45 => self.lyc = val,
      0xFF4A => self.wy = val,
      0xFF4B => self.wx = val,
      0xFF47 => self.bg_palette = val,
      _ => todo!("Ppu register write {addr:04X} not implemented"),
    }
  }
}