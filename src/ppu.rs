use std::collections::VecDeque;

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

const OAM: u16 = 0xFE00;
const VRAM0: u16 = 0x8000;
const VRAM1: u16 = 0x8800;
const VRAM2: u16 = 0x9000;
const MAP0: u16 = 0x9800;
const MAP1: u16 = 0x9C00; 

#[derive(Default)]
enum PpuMode {
  Hblank, // Mode0
  Vblank, // Mode1
  #[default]
  OamScan, // Mode2
  DrawingPixels, // Mode3
}

#[derive(Default)]
enum RenderState {
  #[default] Tile, DataLow, DataHigh, Sleep
}

#[derive(Default)]
struct Fetcher {
  state: RenderState,
  bg_fifo: VecDeque<u8>,
  should_do_step: bool,
  x: u8,
  pixel_x: u8,
  scroll_x: u8,
  
  tile_y: u8,
  tileset_id: u8,
  tileset_addr: u16,
  tile_lo: u8,
  tile_hi: u8,
}
impl Fetcher {
  pub fn reset(&mut self) {
    self.bg_fifo.clear();
    self.x = 0;
    self.pixel_x = 0;
    self.scroll_x = 0;
    self.should_do_step = false;
    self.state = RenderState::Tile;
  }
}

pub struct Ppu {
  pub lcd: FrameBuffer,
  fetcher: Fetcher,

  pub vram: [u8; 8*1024],
  pub oam: [u8; 160],

  mode: PpuMode,
  pub vblank: Option<()>,

  ctrl: Ctrl,
  stat: Stat,
  vram_enabled: bool,
  oam_enabled: bool,
  ly: u8,
  wnd_line: u8,
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
      fetcher: Fetcher::default(),
      vram: [0; 8*1024],
      oam: [0; 160],

      mode: Default::default(),
      vblank: None,

      ctrl: Ctrl::empty(),
      stat: Stat::empty(),

      vram_enabled: false,
      oam_enabled: false,
      ly: 0,
      wnd_line: 0,
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
          self.vram_enabled = false;
          self.fetcher.reset();
        }
      }
      DrawingPixels => {
        if self.fetcher.pixel_x >= 160 {
          self.mode = Hblank;
          self.oam_enabled = true;
          self.vram_enabled = true;

          self.send_lcd_int(Stat::mode0_int);
        } else {
          self.bg_step();
        }
      }
      Hblank => {
        if self.tcycles >= 456 {
          if self.ly >= 143 {
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
          self.oam_enabled = false;

          self.ly = 0;
          self.wnd_line = 0;
          self.send_lcd_int(Stat::mode2_int);
        }
      }
    };
  }

  fn ly_inc(&mut self) {
    if self.is_wnd_visible() {
      self.wnd_line += 1;
    }
    self.ly += 1;

    self.stat.set(Stat::lyc_eq_ly, self.ly == self.lyc);
    if self.stat.contains(Stat::lyc_eq_ly) {
      self.send_lcd_int(Stat::lyc_int);
    }
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

  fn vram_read(&self, addr: u16) -> u8 {
    self.vram[(addr - VRAM0) as usize]
  }

  fn vram_write(&mut self, addr: u16, val: u8) {
    self.vram[(addr - VRAM0) as usize] = val;
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

  pub fn is_vram_enabled(&self) -> bool {
    self.is_ppu_enabled() && self.vram_enabled
  }

  pub fn is_oam_enabled(&self) -> bool {
    self.is_ppu_enabled() && self.oam_enabled
  }

  fn is_inside_wnd(&self) -> bool {
    self.ly >= self.wy
    && self.fetcher.x >= self.wx/8
    && self.ctrl.contains(Ctrl::wnd_enabled) 
  }

  fn is_wnd_visible(&self) -> bool {
    self.ctrl.contains(Ctrl::wnd_enabled)
    && (0..=166).contains(&self.wx)
    && (0..=143).contains(&self.wy)
  }

  pub fn tileset_addr(&self, tileset_id: u8) -> u16 {
    match self.ctrl.contains(Ctrl::tileset_addr) {
      true  => VRAM0 + 16*tileset_id as u16,
      false => {
        let offset = tileset_id as i8;
        (VRAM2 as i32 + 16*offset as i32) as u16
      }
    }
  }

  fn bg_tilemap(&self) -> u16 {
    match self.ctrl.contains(Ctrl::bg_tilemap) {
      false => MAP0,
      true  => MAP1,
    }
  }

  fn wnd_tilemap(&self) -> u16 {
    match self.ctrl.contains(Ctrl::wnd_tilemap) {
      false => MAP0,
      true  => MAP1,
    }
  }

  fn bg_palette(&self, colord_id: u8)  -> u8 {
    let bg_palette = self.bgp;
    (bg_palette >> (colord_id*2)) & 0b11
  }

  fn obj_palette(&self, obp: bool, colord_id: u8)  -> u8 {
    let obj_palette = match obp {
      false => self.obp0,
      true => self.obp1,
    };

    (obj_palette >> (colord_id*2)) & 0b11
  }

  fn oam_scan(&mut self) {

  }

  fn bg_step(&mut self) {
    if self.fetcher.should_do_step {
      match self.fetcher.state {
        RenderState::Tile => {
          let (x, y, tilemap) = 
          if self.is_inside_wnd() {
            let tilemap = self.wnd_tilemap();
            let x = (self.fetcher.x*8).wrapping_add(7).wrapping_sub(self.wx)/8;
            let y = self.wnd_line;

            (x, y, tilemap)
          } else {
            let tilemap = self.bg_tilemap();
            let y = self.ly.wrapping_add(self.scy);
            let x = (self.fetcher.x + self.scx/8) & 31;

            (x, y, tilemap)
          };

          self.fetcher.x += 1;
          
          let tilemap_id = tilemap + 32 * (y/8) as u16 + x as u16;

          self.fetcher.tile_y = y;
          self.fetcher.tileset_id = self.vram_read(tilemap_id);
          self.fetcher.state = RenderState::DataLow;
        }
        RenderState::DataLow => {
          let tile_start = self.tileset_addr(self.fetcher.tileset_id);
          self.fetcher.tileset_addr = tile_start + 2*(self.fetcher.tile_y % 8) as u16;

          self.fetcher.tile_lo = self.vram_read(self.fetcher.tileset_addr);

          self.fetcher.state = RenderState::DataHigh;
        }
        RenderState::DataHigh => {
          self.fetcher.tile_hi = self.vram_read(self.fetcher.tileset_addr+1);

          for bit in 0..8 {
            let lo = (self.fetcher.tile_lo >> bit) & 1;
            let hi = (self.fetcher.tile_hi >> bit) & 1;
            let pixel = (hi << 1) | lo;
            self.fetcher.bg_fifo.push_front(pixel);
          }

          self.fetcher.state = RenderState::Sleep;
        }
        RenderState::Sleep => {
          self.fetcher.state = RenderState::Tile;
        }
      }
    }
    
    self.fetcher.should_do_step = !self.fetcher.should_do_step;
    self.push_pixel();
  }

  fn push_pixel(&mut self) {
    if self.fetcher.bg_fifo.is_empty() { return; }

    let pixel = self.fetcher.bg_fifo.pop_front().unwrap();
    if self.fetcher.scroll_x < self.scx % 8 {
      self.fetcher.scroll_x += 1;
      return;
    }

    self.lcd.set_pixel(self.fetcher.pixel_x as usize, self.ly as usize, pixel);
    self.fetcher.pixel_x += 1;
  }
}