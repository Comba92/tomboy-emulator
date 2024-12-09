use bitflags::bitflags;
use crate::{bus::{self, SharedBus}, frame::FrameBuffer};

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
const DMA: u16 = 0xFF46;
const WY: u16 = 0xFF4A;
const WX: u16 = 0xFF4B;
const BGP: u16 = 0xFF47;
const OAM: u16 = 0xFE00;
const VRAM0: u16 = 0x8000;
const VRAM1: u16 = 0x8800;
const VRAM2: u16 = 0x9000;


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
        // eprintln!("Ppu register read {addr:04X} not implemented");
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
      // _ => eprintln!("Ppu register write {addr:04X} not implemented"),
      _ => {}
    }
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
  bg_scanline: [u8; 160],
  wind_scanline: [u8; 160],
  spr_scanline: [u8; 160],

  mode: PpuMode,
  pub vblank: Option<()>,

  tcycles: usize,
  bus: SharedBus,
}

impl Ppu {
  pub fn new(bus: SharedBus) -> Self {
    Self {
      lcd: FrameBuffer::gameboy_lcd(), 
      bg_scanline:  [0; 160],
      wind_scanline: [0; 160],
      spr_scanline: [0; 160],

      mode: Default::default(),
      vblank: None,

      tcycles: Default::default(), 
      bus,
    }
  }

  fn send_vblank_int(&mut self) {
    bus::send_interrupt(&self.bus.borrow().intf, bus::IFlags::vblank);
    self.vblank = Some(());
  }

  fn send_lcd_int(&mut self, flag: Stat) {
    if self.stat().contains(flag) {
      bus::send_interrupt(&self.bus.borrow().intf, bus::IFlags::lcd);
    } 
  }
  
  pub fn tick(&mut self) {
    use PpuMode::*;
    
    self.tcycles += 1;
    if self.tcycles > 456 {
      self.tcycles = 0;
      self.ly_inc();
    }

    let mut stat = self.stat();
    stat.set(Stat::lyc_eq_ly, self.ly() == self.read(LYC));
    self.set_stat(stat);

    if stat.contains(Stat::lyc_eq_ly) {
      self.send_lcd_int(Stat::lyc_int);
    }

    match self.mode {
      OamScan => {
        if self.tcycles >= 80 {
          self.scan_sprites();
          self.mode = DrawingPixels;
        }
      }
      DrawingPixels => {
        if self.tcycles >= 80 + 172 {
          self.render_scanline();
          self.scan_sprites();

          self.mode = Hblank;
          self.send_lcd_int(Stat::mode0_int);
        }
      }
      Hblank => {
        if self.tcycles >= 456 {
          if self.ly() > 143 {
            self.mode = Vblank;
            self.send_vblank_int();
            self.send_lcd_int(Stat::mode1_int);
          } else {
            self.mode = OamScan
          };
        }
      }
      Vblank => {
        if self.ly() >= 154 {
          self.ly_reset();
          self.mode = OamScan;
          self.send_lcd_int(Stat::mode2_int);
        }
      }
    };
  }


  pub fn render_tile(&mut self, x: usize, y: usize, tile_addr: usize) {
    let tile = &self.bus.borrow().mem[tile_addr..tile_addr+16];

    for row in 0..8 {
      let plane0 = tile[row*2];
      let plane1 = tile[row*2 + 1];

      for bit in 0..8 {
          let bit0 = (plane0 >> bit) & 1;
          let bit1 = ((plane1 >> bit) & 1) << 1;
          let color_idx = bit1 | bit0;
          let color = self.palette_color(color_idx);

          self.lcd.set_pixel(x + 7-bit, y + row, color);
      }
    }
  }

  pub fn render_tile_row(&mut self, x: usize, y: usize, tileset_addr: usize, row: usize) {
    let tile_addr = tileset_addr + row*2;
    let tile_row = &self.bus.borrow().mem[tile_addr..tile_addr+2];

    let plane0 = tile_row[0];
    let plane1 = tile_row[1];

    for bit in 0..8 {
        let bit0 = (plane0 >> bit) & 1;
        let bit1 = ((plane1 >> bit) & 1) << 1;
        let color_idx = bit1 | bit0;
        let color = self.palette_color(color_idx);

        self.lcd.set_pixel(x + 7-bit, y, color);
    }
  }

  fn scan_sprites(&mut self) {
    let scanline = self.ly();

    for i in (0..256).step_by(4) {
      let mut y = self.read(OAM + i);
      let mut x = self.read(OAM + i+1);

      if y < 16 || y >= 160 || x < 8 || x >= 168 { continue; }
      y -= 16;
      x -= 8;

      let row = scanline.abs_diff(y) as usize;
      if row >= 8 { continue; }

      let tile_id = self.read(OAM + i+2) as u16;
      let tileset_addr = self.read(VRAM0 + tile_id*16) as usize;
      self.render_tile_row(x as usize, y as usize + row, tileset_addr, row as usize);
    }
  }

  fn render_scanline(&mut self) {
    let scanline = self.ly() as u16;

    let scx = self.read(SCX) as u16;
    let scy = self.read(SCY) as u16;
    let tilemap = self.bg_tilemap();

    for pixel in (0..256).step_by(8) {
      let tilemap_addr = tilemap 
        + ((scy + scanline) % 256)/8 * 32
        + ((scx + pixel) % 256)/8;

      let tile_id = self.read(tilemap_addr);
      let tileset_addr = self.tile_addr(tile_id) as usize;
      self.render_tile_row(pixel as usize, scanline as usize, tileset_addr, scanline as usize%8);
    }
  }

  fn render_bg(&mut self) {
    let scx = self.read(SCX) as u16/8;
    let scy = self.read(SCY) as u16/8;
    let tilemap = self.bg_tilemap();

    for y in 0..144/8 {
      for x in 0..160/8 {
        let tilemap_addr = tilemap
          + ((scy + y) % 32) * 32
          + ((scx + x) % 32);

        let tile_id = self.read(tilemap_addr);
        let tileset_addr = self.tile_addr(tile_id) as usize;

        self.render_tile(8*x as usize, 8*y as usize, tileset_addr);
      }
    }
  }

  fn render_spr(&mut self) {
    for i in (0xFE00..=0xFE9F).step_by(4) {
      let y = self.read(i) as usize;
      let x = self.read(i+1) as usize;
      let tile_id = self.read(i+2);
      let attributes = self.read(i+3);
      
      if x < 8 || y < 16 || x >= 168 || y >= 160 { continue; }

      // let priority = (attributes >> 7) & 1 == 0;
      // let y_flip = (attributes >> 6) & 1;
      // let x_flip = (attributes >> 5) & 1;
      
      let tileset_addr = VRAM0 as usize + 16*tile_id as usize;
      self.render_tile(x+8, y+16, tileset_addr);
    }
  }

  pub fn read(&self, addr: u16) -> u8 {
    self.bus.borrow().read(addr)
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    self.bus.borrow_mut().write(addr, val);
  }

  fn bg_tilemap(&self) -> u16 {
    match self.ctrl().contains(Ctrl::bg_tilemap) {
      false => 0x9800,
      true => 0x9C00,
    }
  }

  fn wind_tilemap(&self) -> u16 {
    match self.ctrl().contains(Ctrl::wind_tilemap) {
      false => 0x9800,
      true  => 0x9C00,
    }
  }

  fn ctrl(&self) -> Ctrl {
    self.bus.borrow().ppu_regs.ctrl
  }

  fn stat(&self) -> Stat {
    self.bus.borrow().ppu_regs.stat
  }

  fn set_stat(&mut self, stat: Stat) {
    self.bus.borrow_mut().ppu_regs.stat = stat;
  }

  fn ly(&self) -> u8 {
    self.bus.borrow().ppu_regs.ly
  }

  fn ly_inc(&mut self) {
    self.bus.borrow_mut().ppu_regs.ly += 1;
  }

  fn ly_reset(&mut self) {
    self.bus.borrow_mut().ppu_regs.ly = 0;
  }

  pub fn tile_addr(&self, tile_id: u8) -> u16 {
    match self.ctrl().contains(Ctrl::tiles_addr) {
      true  => VRAM0 + 16*tile_id as u16,
      false => {
        let offset = tile_id as i8;
        (VRAM2 as i32 + 16*offset as i32) as u16
      }
    }
  }

  fn palette_color(&self, colord_id: u8)  -> u8 {
    let bg_palette = self.bus.borrow().ppu_regs.bg_palette;
    (bg_palette >> (colord_id*2)) & 0b11
  }
}