use std::collections::VecDeque;

use crate::{bus::{self, IFlags, InterruptFlags}, frame::FrameBuffer, nth_bit};
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

  #[derive(Default, Clone, Copy, PartialEq)]
  pub struct Stat: u8 {
    const lyc_eq_ly = 0b0000_0100;
    const mode0_int = 0b0000_1000;
    const mode1_int = 0b0001_0000;
    const mode2_int = 0b0010_0000;
    const lyc_int   = 0b0100_0000;
    const unused    = 0b1000_0000;
  }
}

const _OAM: u16 = 0xFE00;
const VRAM0: u16 = 0x8000;
const _VRAM1: u16 = 0x8800;
const VRAM2: u16 = 0x9000;
const MAP0: u16 = 0x9800;
const MAP1: u16 = 0x9C00; 

#[derive(Default, Clone, Copy, PartialEq)]
enum PpuMode {
  Hblank, // Mode0
  Vblank, // Mode1
  #[default]
  OamScan, // Mode2
  DrawingPixels, // Mode3
}

#[derive(Default)]
enum FetcherState {
  #[default] Tile, DataLow, DataHigh, Push
}

struct Fetcher {
  state: FetcherState,
  obj_visible: Vec<OamObject>,
  bg_fifo: VecDeque<u8>,
  obj_scanline: [Option<ObjFifoEntry>; 160],
  should_do_step: bool,
  x: u8,
  wnd_hit: bool,
  pixel_x: u8,
  bg_scroll_x: u8,
  wnd_scroll_x: u8,
  
  tile_y: u8,
  tileset_id: u8,
  tileset_addr: u16,
  tile_lo: u8,
  tile_hi: u8,
}

impl Default for Fetcher {
  fn default() -> Self {
    Self { state: Default::default(), obj_visible: Default::default(), bg_fifo: Default::default(), obj_scanline: [const {None}; 160], should_do_step: Default::default(), x: Default::default(), wnd_hit: Default::default(), pixel_x: Default::default(), bg_scroll_x: Default::default(), wnd_scroll_x: Default::default(), tile_y: Default::default(), tileset_id: Default::default(), tileset_addr: Default::default(), tile_lo: Default::default(), tile_hi: Default::default() }
  }
}

impl Fetcher {
  pub fn reset(&mut self) {
    self.bg_fifo.clear();
    self.x = 0;
    self.wnd_hit = false;
    self.pixel_x = 0;
    self.bg_scroll_x = 0;
    self.wnd_scroll_x = 0;
    self.should_do_step = false;
    self.state = FetcherState::Tile;
  }
}

#[derive(Default, Clone)]
struct ObjFifoEntry {
  color: u8,
  palette: bool,
  priority: bool,
}
struct OamObject {
  i: u8,
  y: u8,
  x: u8,
  tile_id: u8,
  priority: bool,
  x_flip: bool,
  y_flip: bool,
  dmg_palette: bool,
}
impl OamObject {
  pub fn new(bytes: &[u8], i: u8) -> Self {
    let y = bytes[0];
    let x = bytes[1];
    let tile_id = bytes[2];
    let attr = bytes[3];
    let priority = !nth_bit(attr, 7);
    let y_flip = nth_bit(attr, 6);
    let x_flip = nth_bit(attr, 5);
    let dmg_palette = nth_bit(attr, 4);

    Self {
      i, y, x, tile_id, priority, y_flip, x_flip, dmg_palette
    }
  }
}

pub struct Ppu {
  pub lcd: FrameBuffer,
  fetcher: Fetcher,

  pub vram: [u8; 8*1024],
  pub oam: [u8; 160],

  mode: PpuMode,
  pub frame_ready: Option<()>,

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

  tcycles: usize,
  intf: InterruptFlags,
  stat_int_flag: bool,
}

impl Ppu {
  pub fn new(intf: InterruptFlags) -> Self {
    Self {
      lcd: FrameBuffer::gameboy_lcd(),
      fetcher: Fetcher::default(),
      vram: [0; 8*1024],
      oam: [0; 160],

      mode: Default::default(),
      frame_ready: None,

      ctrl: Ctrl::lcd_enabled,
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
      stat_int_flag: false,
    }
  }

  pub fn tick(&mut self) {
    if !self.is_lcd_enabled() {
      self.tcycles += 1;
      if self.tcycles >= 70224 {
        self.frame_ready = Some(());
      }
    }

    let old_stat = self.stat;

    self.tcycles += 1;
    if self.tcycles > 456 {
      self.tcycles = 0;
      self.ly_inc();
    }
    
    use PpuMode::*;
    match self.mode {
      OamScan => {
        if self.tcycles >= 80 {
          // we do this in one go
          self.oam_scan();
          self.fill_obj_scanline();

          self.mode = DrawingPixels;
          self.vram_enabled = false;
        }
      }
      DrawingPixels => {
        if self.fetcher.pixel_x >= 160 {
          self.oam_enabled = true;
          self.vram_enabled = true;
          self.fetcher.reset();
          
          self.mode = Hblank;
          // self.send_lcd_int(Stat::mode0_int);
          self.send_stat_int();
        } else {
          self.fetcher_step();
        }
      }
      Hblank => {
        if self.tcycles >= 456 {
          if self.ly >= 143 {
            
            self.mode = Vblank;
            self.send_vblank_int();
            // self.send_lcd_int(Stat::mode1_int);
            self.send_stat_int();
          } else {
            self.mode = OamScan;
            // self.send_lcd_int(Stat::mode2_int);
            self.send_stat_int();
          };
        }
      }
      Vblank => {
        if self.ly >= 154 {
          self.mode = OamScan;
          // self.send_lcd_int(Stat::mode2_int);
          self.send_stat_int();
          
          self.oam_enabled = false;

          self.ly = 0;
          self.wnd_line = 0;
        }
      }
    };

    self.stat.set(Stat::lyc_eq_ly, self.lyc == self.ly);
    self.send_stat_int();
    // self.stat.set(Stat::lyc_eq_ly, self.ly == self.lyc);
    // let lyc = self.ly == self.lyc;
    // self.stat.insert(Stat::lyc_eq_ly);
    // if old_stat != self.stat {
    //   // self.send_lyc_int();
    //   self.send_stat_int();
    // }
  }

  pub fn read(&self, addr: u16) -> u8 {
    match addr {
      0xFF40 => self.ctrl.bits(),
      0xFF41 => {
        let mut res = self.stat.bits();
        res |= self.mode as u8;
        res |= Stat::unused.bits();
        res
      },
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
      0xFF40 => {
        let old_ctrl = self.ctrl.clone();
        self.ctrl = Ctrl::from_bits_retain(val);

        // lcd enabling/disabling logic
        if old_ctrl.contains(Ctrl::lcd_enabled) != self.ctrl.contains(Ctrl::lcd_enabled) {
          // it is turned on
          if self.ctrl.contains(Ctrl::lcd_enabled) {
            self.tcycles = 80;
            self.ly = 0;
            self.wnd_line = 0;
            self.mode = PpuMode::DrawingPixels;
            self.stat.set(Stat::lyc_eq_ly, self.ly == self.lyc);
            self.send_stat_int();
            // self.send_stat_int();
          // it is turned off
          } else {
            self.tcycles = 0;
            self.ly = 0;
            self.wnd_line = 0;
            self.mode = PpuMode::Hblank;
            self.fetcher.reset();
            self.lcd.reset();

            self.vram_enabled = true;
            self.oam_enabled = true;
          }
        }
      }
      0xFF41 => {
        let mut res = Stat::from_bits_retain(val & 0b0111_1000);
        res.set(Stat::lyc_eq_ly, self.stat.contains(Stat::lyc_eq_ly));
        self.stat = res;
      }
      0xFF42 => self.scy = val,
      0xFF43 => self.scx = val,
      0xFF45 => {
        self.lyc = val;
        // self.send_lyc_int();
        // self.send_stat_int();
        self.stat.set(Stat::lyc_eq_ly, self.lyc == self.ly);
        self.send_stat_int();
      }
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

  fn send_vblank_int(&mut self) {
    if self.is_lcd_enabled() {
      bus::send_interrupt(&self.intf, bus::IFlags::vblank);
    }

    self.frame_ready = Some(());
  }

  fn send_lcd_int(&mut self, flag: Stat) {
    if self.stat.contains(flag) && self.is_lcd_enabled() {
      bus::send_interrupt(&self.intf, bus::IFlags::lcd);
    }
  }

  fn send_stat_int(&mut self) {
    let int = self.is_lcd_enabled() && (
      (self.stat.contains(Stat::lyc_int) && self.stat.contains(Stat::lyc_eq_ly))
      || (self.stat.contains(Stat::mode0_int) && self.mode == PpuMode::Hblank)
      || (self.stat.contains(Stat::mode1_int) && self.mode == PpuMode::Vblank)
      || (self.stat.contains(Stat::mode2_int) && self.mode == PpuMode::OamScan)
    );

    if int && !self.stat_int_flag {
      bus::send_interrupt(&self.intf, IFlags::lcd);
    }

    self.stat_int_flag = int;
  }

  fn send_lyc_int(&mut self) {
    self.stat.set(Stat::lyc_eq_ly, self.ly == self.lyc);

    if self.stat.contains(Stat::lyc_eq_ly) {
      self.send_lcd_int(Stat::lyc_int);
    }
  }

  pub fn is_lcd_enabled(&self) -> bool {
    self.ctrl.contains(Ctrl::lcd_enabled)
  }

  fn ly_inc(&mut self) {
    // wnd_line is only incremented when window is VISIBLE and HIT
    if self.ly >= self.wy
    && self.wy < 143
    && self.wx < 166
    {
      self.wnd_line += 1;
    }
    self.ly += 1;

    self.stat.set(Stat::lyc_eq_ly, self.lyc == self.ly);
    self.send_stat_int();
    // self.send_lyc_int();
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

  fn obj_size(&self) -> u8 {
    match self.ctrl.contains(Ctrl::obj_size) {
      false => 8,
      true  => 16,
    }
  }

  fn oam_scan(&mut self) {
    self.fetcher.obj_visible.clear();

    for i in (0..160).step_by(4) {
      let y = self.oam[i];

      if self.ly.wrapping_add(16) >= y
      && self.ly.wrapping_add(16) < y.wrapping_add(self.obj_size())
      {
        let obj = OamObject::new(&self.oam[i..i+4], i as u8/4);
        self.fetcher.obj_visible.push(obj);
      }

      if self.fetcher.obj_visible.len() >= 10 { break; }
    }

    // we sort them in reverse (lower to higher), so that we always set for last to the scanline the higher priority object
    self.fetcher.obj_visible.sort_by(|a, b| {
      if a.x == b.x { b.i.cmp(&a.i) } else { b.x.cmp(&a.x) } 
    });
  }

  fn fill_obj_scanline(&mut self) {
    if !self.is_lcd_enabled() { return; }
    if !self.ctrl.contains(Ctrl::obj_enabled) { return; }
    self.fetcher.obj_scanline.fill(None);

    for obj in &self.fetcher.obj_visible {
      if obj.x == 0 || obj.x >= 168 { continue; }

      let y = obj.y.saturating_sub(16);
      let row = self.ly.abs_diff(y);
      
      // Sprite 8x16 tile handling
      let tile_id = if self.ctrl.contains(Ctrl::obj_size) {        
        obj.tile_id & 0xFE
      } else { obj.tile_id };

      // Y flipping (simply reverse the y offset)
      let y_offset = if obj.y_flip {
        row.abs_diff(self.obj_size()-1)
      } else { row };

      let tileset_addr = VRAM0 
        + 16*tile_id as u16
        + 2*y_offset as u16;

      let mut tile_lo = self.vram_read(tileset_addr);
      let mut tile_hi = self.vram_read(tileset_addr+1);

      // X flipping (reverse the bits, knowing that they are reversed without flipping)
      if !obj.x_flip {
        tile_lo = tile_lo.reverse_bits();
        tile_hi = tile_hi.reverse_bits();
      }

      // push each pixel
      for i in 0..8 {
        if obj.x + i < 8 || obj.x + i >= 168 { continue; }

        let x = obj.x + i - 8;

        let pixel_lo = (tile_lo >> i) & 1;
        let pixel_hi = (tile_hi >> i) & 1;
        let color = (pixel_hi << 1) | pixel_lo;
        if color == 0 { continue; }

        let data = ObjFifoEntry { 
          color,
          palette: obj.dmg_palette,
          priority: obj.priority
        };

        self.fetcher.obj_scanline[x as usize] = Some(data);
      }
    }
  }

  fn fetcher_step(&mut self) {
    if !self.fetcher.wnd_hit && self.ctrl.contains(Ctrl::wnd_enabled) 
      && self.fetcher.pixel_x + 7 >= self.wx
      && self.ly >= self.wy
    {
      self.fetcher.wnd_hit = true;
      self.fetcher.x = 0;
      
      if self.wx < 7 {
        self.fetcher.wnd_scroll_x = 7- self.wx;
      }

      self.fetcher.bg_fifo.clear();
      self.fetcher.should_do_step = false;
      self.fetcher.state = FetcherState::Tile;
      return;
    }

    if self.fetcher.should_do_step {
      match self.fetcher.state {
        FetcherState::Tile => {
          let (y, tilemap_id) =
          if self.fetcher.wnd_hit
          {
            let tilemap = self.wnd_tilemap();
            let x = self.fetcher.x;
            let y = self.wnd_line;
            let tilemap_id = tilemap + 32 * (y/8) as u16 + x as u16;
            self.fetcher.x = (self.fetcher.x + 1) % 32;

            (y, tilemap_id)
          } else {
            let tilemap = self.bg_tilemap();
            let y = self.ly.wrapping_add(self.scy);
            let x = (self.fetcher.x + self.scx/8) & 31;
            let tilemap_id = tilemap + 32 * (y/8) as u16 + x as u16;
            self.fetcher.x = (self.fetcher.x + 1) % 32;

            (y, tilemap_id)
          };

          self.fetcher.tile_y = y;
          self.fetcher.tileset_id = self.vram_read(tilemap_id);
          self.fetcher.state = FetcherState::DataLow;
        }
        FetcherState::DataLow => {
          let tile_start = self.tileset_addr(self.fetcher.tileset_id);
          self.fetcher.tileset_addr = tile_start + 2*(self.fetcher.tile_y % 8) as u16;

          self.fetcher.tile_lo = self.vram_read(self.fetcher.tileset_addr);
          self.fetcher.state = FetcherState::DataHigh;
        }
        FetcherState::DataHigh => {
          self.fetcher.tile_hi = self.vram_read(self.fetcher.tileset_addr+1);
          self.fetcher.state = FetcherState::Push;
        }
        FetcherState::Push => {
          if self.fetcher.bg_fifo.is_empty() {
            for bit in 0..8 {
              let lo = (self.fetcher.tile_lo >> bit) & 1;
              let hi = (self.fetcher.tile_hi >> bit) & 1;
              let pixel = (hi << 1) | lo;
              self.fetcher.bg_fifo.push_front(pixel);
            }

            self.fetcher.state = FetcherState::Tile;
          } else {
            self.fetcher.should_do_step = false;
          }
        }
      }
    }
    
    self.fetcher.should_do_step = !self.fetcher.should_do_step;
    self.push_pixel();
  }

  fn push_pixel(&mut self) {
    if !self.is_lcd_enabled() {
      self.lcd.set_pixel(self.fetcher.pixel_x as usize, self.ly as usize, self.bg_palette(0));
      self.fetcher.pixel_x += 1;
      return;
    }

    // we always have at least 8 pixels ready
    if self.fetcher.bg_fifo.is_empty() { return; }

    // we should pop discarding the scrolling pixels
    let bg_color = self.fetcher.bg_fifo.pop_front().unwrap();
    
    if self.fetcher.wnd_scroll_x > 0 {
      self.fetcher.wnd_scroll_x -= 1;
      return;
    }
    if !self.fetcher.wnd_hit && self.fetcher.bg_scroll_x < self.scx % 8 {
      self.fetcher.bg_scroll_x += 1;
      return;
    }

    let obj = &self.fetcher.obj_scanline[self.fetcher.pixel_x as usize]
      .take().unwrap_or_default();

    let color = if self.ctrl.contains(Ctrl::obj_enabled) 
      && obj.color != 0 && (obj.priority || bg_color == 0)
    {
      self.obj_palette(obj.palette, obj.color)
    } else if self.ctrl.contains(Ctrl::bg_wnd_enabled) {
      self.bg_palette(bg_color)
    } else {
      self.bg_palette(0)
    };

    self.lcd.set_pixel(self.fetcher.pixel_x as usize, self.ly as usize, color);
    self.fetcher.pixel_x += 1;
  }
}