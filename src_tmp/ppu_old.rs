use std::collections::VecDeque;

use bitflags::bitflags;
use crate::{bus::{self, SharedBus}, frame::FrameBuffer};

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
pub struct Registers {
  ctrl: Ctrl,
  ppu_enabled: bool,
  vram_enabled: bool,
  oam_enabled: bool,

  ly: u8,
  lyc: u8,
  stat: Stat,

  scy: u8,
  scx: u8,
  wy: u8,
  wx: u8,

  bgp: u8,
  obp0: u8,
  obp1: u8,
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
const OBP0: u16 = 0xFF48;
const OBP1: u16 = 0xFF49;
const OAM: u16 = 0xFE00;
const VRAM0: u16 = 0x8000;
const VRAM1: u16 = 0x8800;
const VRAM2: u16 = 0x9000;
const MAP0: u16 = 0x9800;
const MAP1: u16 = 0x9C00; 

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
      0xFF47 => self.bgp,
      0xFF48 => self.obp0,
      0xFF49 => self.obp1,
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
      0xFF47 => self.bgp = val,
      0xFF48 => self.obp0 = val,
      0xFF49 => self.obp1 = val,
      // _ => eprintln!("Ppu register write {addr:04X} not implemented"),
      _ => {}
    }
  }

  pub fn is_ppu_enabled(&self) -> bool {
    self.ctrl.contains(Ctrl::lcd_enabled)
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

#[derive(Default)]
struct RenderData {
  step: RenderStep,
  dot: bool,
  scaline_x: u8,
  tilemap_x: u8,
  tilemap_y: u8,
  tile_id: u8,
  tileset_addr: u16,
  tile_data_low: u8,
  tile_data_high: u8,
}

struct OamObject {
  y: u8,
  x: u8,
  tile_id: u8,
  priority: bool,
  x_flip: bool,
  y_flip: bool,
  dmg_palette: bool,
}

#[derive(Default)]
enum RenderStep {
  #[default] Tile, DataLow, DataHigh, Sleep
}

pub struct Ppu {
  pub lcd: FrameBuffer,
  vram: [u8; 8*1024],
  oam: [u8; 160],

  render: RenderData,
  bg_fifo: VecDeque<u8>,
  wnd_line: u8,

  mode: PpuMode,
  pub vblank: Option<()>,

  tcycles: u16,
  bus: SharedBus,
}

impl Ppu {
  pub fn new() -> Self {
    Self {
      lcd: FrameBuffer::gameboy_lcd(),
      render: RenderData::default(),
      bg_fifo: VecDeque::new(),

      mode: Default::default(),
      vblank: None,

      tcycles: Default::default(), 
      wnd_line: Default::default(),
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

    match self.mode {
      OamScan => {
        if self.tcycles >= 80 {
          self.mode = DrawingPixels;

          self.bus.borrow_mut().ppu_regs.vram_enabled = false;
          self.bg_fifo.clear();
          self.render = RenderData::default();
        }
      }
      DrawingPixels => {
        // if self.render.scaline_x >= 160 {
        if self.tcycles > 80 + 172 {
          self.render_bg_scanline();
          self.render_objs_scanline();
          self.render_wnd_scanline();

          self.mode = Hblank;
          self.bus.borrow_mut().ppu_regs.oam_enabled  = true;
          self.bus.borrow_mut().ppu_regs.vram_enabled = true;

          self.send_lcd_int(Stat::mode0_int);
        } else {
          // self.bg_drawing_step();
        }
      }
      Hblank => {
        if self.tcycles >= 456 {
          if self.ly() > 143 {
            self.mode = Vblank;
            self.send_vblank_int();
            self.send_lcd_int(Stat::mode1_int);
          } else {
            self.mode = OamScan;
          };
        }
      }
      Vblank => {
        if self.ly() >= 154 {
          self.mode = OamScan;
          self.ly_reset();
          self.bus.borrow_mut().ppu_regs.oam_enabled = false;
          self.send_lcd_int(Stat::mode2_int);
        }
      }
    };
  }

  fn bg_drawing_step(&mut self) {
    if !self.ctrl().contains(Ctrl::bg_wnd_enabled) {
      self.lcd.set_pixel(self.render.scaline_x as usize, self.ly() as usize, self.bg_palette(0));
      self.render.scaline_x += 1;
      return;
    }

    self.render.dot = !self.render.dot;

    if !self.render.dot {
      match &self.render.step {
        RenderStep::Tile => {
          self.render.tilemap_y = self.read(SCY) + self.ly();
          let wx = self.read(WX);
          let wy = self.read(WY);

          let (x, y, tilemap) = if self.ly() >= wy 
          && self.render.scaline_x >= wx
          && self.ctrl().contains(Ctrl::wnd_enabled) {
            // it is a window tile
            if self.wnd_line == 0 { self.bg_fifo.clear(); }
            self.render.tilemap_y = self.wnd_line;
            (wx as u16, self.wnd_line as u16, self.wnd_tilemap())
          } else {
            // it is a background tile
            let x = 
            (self.read(SCX) + self.render.tilemap_x) as u16 % 256;
            let y = 
              self.render.tilemap_y as u16 % 256;
            (x, y, self.bg_tilemap())
          };

          let tilemap_addr  = tilemap + 32 * (y/8) + x/8;
          self.render.tile_id = self.read(tilemap_addr);

          self.render.step = RenderStep::DataLow;
        }
        RenderStep::DataLow => {
          self.render.tileset_addr = 
            self.tileset_addr(self.render.tile_id)
            + 2 * (self.render.tilemap_y as u16 % 8);
            
          self.render.tile_data_low = self.read(self.render.tileset_addr);

          self.render.step = RenderStep::DataHigh;
        }
        RenderStep::DataHigh => {
          self.render.tile_data_high = 
            self.read(self.render.tileset_addr.wrapping_add(1));

          for bit in (0..8).rev() {
            let lo = (self.render.tile_data_low  >> bit) & 1;
            let hi = (self.render.tile_data_high >> bit) & 1;
            let pixel = (hi << 1) | lo;
            self.bg_fifo.push_back(pixel);
          }

          self.render.step = RenderStep::Sleep;
        }
        RenderStep::Sleep => {
          self.render.step = RenderStep::Tile;
        }
      };
    }

    if self.bg_fifo.len() > 8 {
      let pixel = self.bg_fifo.pop_front().unwrap();
      self.lcd.set_pixel(self.render.scaline_x as usize, self.ly() as usize, pixel);
      self.render.scaline_x += 1;
    }

    self.render.tilemap_x += 1;
  }

  pub fn render_tile(&mut self, x: usize, y: usize, tile_addr: usize) {
    let tile_addr = tile_addr - VRAM0 as usize;
    let tile = &self.bus.borrow().vram[tile_addr..tile_addr+16];

    for row in 0..8 {
      let plane0 = tile[row*2];
      let plane1 = tile[row*2 + 1];

      for bit in 0..8 {
          let bit0 = (plane0 >> bit) & 1;
          let bit1 = ((plane1 >> bit) & 1) << 1;
          let color_idx = bit1 | bit0;
          let color = self.bg_palette(color_idx);

          self.lcd.set_pixel(x + 7-bit, y + row, color);
      }
    }
  }

  pub fn render_tile_row(&mut self, x: usize, y: usize, tileset_addr: usize, row: usize, obj_attr: Option<u8>) {    
    let (priority, palette, x_offset, y_offset) = if let Some(attr) = &obj_attr {
      let priority = attr >> 7 == 0;
      let y_flip = (attr >> 6) & 1 == 1;
      let x_flip = (attr >> 5) & 1 == 1;
      let palette = (attr >> 4) & 1 == 1;
      let x_offset = if x_flip { 0 } else { 7 } as usize;
      let y_offset = if y_flip { 7 } else { 0 } as usize;
      (priority, palette, x_offset, y_offset)
    } else {
      (true, false, 7, 0)
    };

    let tile_row_addr = tileset_addr + y_offset.abs_diff(row)*2;

    let plane0 = self.read(tile_row_addr as u16);
    let plane1 = self.read(tile_row_addr as u16+1);

    for bit in 0..8 {
        let bit0 = (plane0 >> bit) & 1;
        let bit1 = ((plane1 >> bit) & 1) << 1;
        let color_idx = bit1 | bit0;
        
        if obj_attr.is_some() && (!priority || color_idx == 0) { continue; }

        let color = if obj_attr.is_some() {
          self.obj_palette(palette, color_idx)
        } else {
          self.bg_palette(color_idx)
        };

        self.lcd.set_pixel(x + x_offset.abs_diff(bit), y, color);
    }
  }

  fn render_objs_scanline(&mut self) {
    if !self.ctrl().contains(Ctrl::lcd_enabled) 
    || !self.ctrl().contains(Ctrl::obj_enabled) { return; }

    let scanline = self.ly();

    let mut visible = 0;
    for i in (0..256).step_by(4) {
      let mut y = self.read(OAM + i+0);
      let mut x = self.read(OAM + i+1);

      if y < 16 || y >= 160 || x < 8 || x >= 168 { continue; }
      y -= 16;
      x -= 8;

      let row = scanline.abs_diff(y) as usize;
      if row >= (if self.ctrl().contains(Ctrl::obj_size) {16} else {8}) { continue; }

      let attributes = self.read(OAM + i+3);
      let y_flip = (attributes >> 6) & 1 == 1;

      let mut tile_id = self.read(OAM + i+2) as u16;
      if self.ctrl().contains(Ctrl::obj_size) {
        match y_flip {
          false => tile_id = if row >= 8 { tile_id | 0x01 } else { tile_id & 0xFE },
          true => tile_id = if  row >= 8 { tile_id & 0xFE } else { tile_id | 0x01 },
        }
      }
      
      let tileset_addr = VRAM0 as usize + 16*tile_id as usize;

      self.render_tile_row(
        x as usize, y as usize + row, tileset_addr, row as usize, Some(attributes)
      );

      visible += 1;
      if visible >= 10 {break;}
    }
  }

  fn render_bg_scanline(&mut self) {
    let scanline = self.ly() as u16;

    let scx = self.read(SCX) as u16;
    let scy = self.read(SCY) as u16;
    let tilemap = self.bg_tilemap();

    for pixel in (8..160).step_by(8) {
      let tilemap_addr = tilemap 
        + ((scy + scanline) % 256)/8 * 32
        + ((scx + pixel) % 256)/8;

      let tile_id = self.read(tilemap_addr);
      let tileset_addr = self.tileset_addr(tile_id) as usize;

      if !self.ctrl().contains(Ctrl::lcd_enabled) || !self.ctrl().contains(Ctrl::bg_wnd_enabled) {
        self.render_tile_row((pixel - scx%8) as usize, scanline as usize, 0, (scy + scanline) as usize%8, None);
      } else {
        self.render_tile_row((pixel - scx%8) as usize, scanline as usize, tileset_addr, (scy + scanline) as usize%8, None);
      }
    }
  }

  fn render_wnd_scanline(&mut self) {
    if !self.ctrl().contains(Ctrl::lcd_enabled) 
    || !self.ctrl().contains(Ctrl::bg_wnd_enabled) 
    || !self.ctrl().contains(Ctrl::wnd_enabled) {return;}

    let mut wx = self.read(WX) as u16;
    let wy = self.read(WY) as u16;
    let scanline = self.ly() as u16;

    if scanline < wy || wx < 7 || wx >= 166 || wy >= 143 { return; }
    wx -= 7;
    let tilemap = self.wnd_tilemap();
    let row = self.wnd_line as u16;
    self.wnd_line += 1;

    for pixel in (wx..160).step_by(8) {
      let tilemap_addr = tilemap
        + row/8 * 32
        + (pixel + 7 - wx)/8;

      let tile_id = self.read(tilemap_addr);
      let tileset_addr = self.tileset_addr(tile_id) as usize;
      self.render_tile_row(pixel as usize, scanline as usize, tileset_addr, scanline as usize%8, None);
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
        let tileset_addr = self.tileset_addr(tile_id) as usize;

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
    // if self.ly() >= self.read(WY) {
    //   self.wnd_line += 1;
    // }
    self.bus.borrow_mut().ppu_regs.ly += 1;

    let ly = self.ly();
    let mut stat = self.stat();
    stat.set(Stat::lyc_eq_ly, ly == self.read(LYC));
    self.set_stat(stat);

    if stat.contains(Stat::lyc_eq_ly) {
      self.send_lcd_int(Stat::lyc_int);
    }
  }

  fn ly_reset(&mut self) {
    self.bus.borrow_mut().ppu_regs.ly = 0;
    self.wnd_line = 0;
  }

  fn tile_data_addr(&self, offset: u8) -> u16 {
    match self.ctrl().contains(Ctrl::tileset_addr) {
      true => VRAM0 + offset as u16,
      false => (VRAM2.wrapping_add_signed((offset as i8) as i16)) as u16
    }
  }

  pub fn tileset_addr(&self, tile_id: u8) -> u16 {
    match self.ctrl().contains(Ctrl::tileset_addr) {
      true  => VRAM0 + 16*tile_id as u16,
      false => {
        let offset = tile_id as i8;
        (VRAM2 as i32 + 16*offset as i32) as u16
      }
    }
  }

  fn bg_tilemap(&self) -> u16 {
    match self.ctrl().contains(Ctrl::bg_tilemap) {
      false => MAP0,
      true  => MAP1,
    }
  }

  fn wnd_tilemap(&self) -> u16 {
    match self.ctrl().contains(Ctrl::wnd_tilemap) {
      false => MAP0,
      true  => MAP1,
    }
  }

  fn bg_palette(&self, colord_id: u8)  -> u8 {
    let bg_palette = self.bus.borrow().ppu_regs.bgp;
    (bg_palette >> (colord_id*2)) & 0b11
  }

  fn obj_palette(&self, obp: bool, colord_id: u8)  -> u8 {
    let obj_palette = match obp {
      false => self.bus.borrow().ppu_regs.obp0,
      true => self.bus.borrow().ppu_regs.obp1,
    };

    (obj_palette >> (colord_id*2)) & 0b11
  }
}