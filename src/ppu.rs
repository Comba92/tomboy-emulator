use bitflags::bitflags;

bitflags! {
  #[derive(Default)]
  struct Ctrl: u8 {
    const bg_wind_on = 0b0000_0001;
    const obj_on     = 0b0000_0010;
    const obj_size   = 0b0000_0100;
    const bg_tilemap = 0b0000_1000;

    const bg_wind_addr = 0b0001_0000;
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

#[derive(Default)]
pub struct Ppu {
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

  tcycles: u8,
  scanlines: u8,
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

  pub fn step(&mut self) {
    self.tcycles += 1;
    if self.tcycles > 80 {
      self.mode = PpuMode::Mode3;
    }

    if self.tcycles > 204 {
      self.tcycles = 0;
      self.scanlines += 1;

      if (144..=153).contains(&self.scanlines) {
        self.mode = PpuMode::Mode1;
      } else if self.scanlines > 153 {
        self.scanlines = 0;
        self.mode = PpuMode::Mode2;
      }
    }
  }
}