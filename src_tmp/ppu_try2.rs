use crate::{bus::{self, IFlags, InterruptFlags}, frame::FrameBuffer};
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

#[derive(Default, Clone, Copy, PartialEq)]
enum PpuMode {
  Hblank, // Mode0
  Vblank, // Mode1
  #[default]
  OamScan, // Mode2
  DrawingPixels, // Mode3
}

const VRAM0: u16 = 0x8000;
const _VRAM1: u16 = 0x8800;
const VRAM2: u16 = 0x9000;
const MAP0: u16 = 0x9800;
const MAP1: u16 = 0x9C00; 

#[derive(Default, Clone, Copy)]
struct ObjEntry {
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
    let priority = attr >> 7 == 0;
    let y_flip = (attr >> 6) & 1 != 0;
    let x_flip = (attr >> 5) & 1 != 0;
    let dmg_palette = (attr >> 4) & 1 != 0;

    Self {
      i, y, x, tile_id, priority, y_flip, x_flip, dmg_palette
    }
  }
}

pub struct Ppu {
  pub lcd: FrameBuffer,
  obj_visible: Vec<OamObject>,
  obj_scanline: [Option<ObjEntry>; 160],
  bg_scanline: [u8; 160],

  pub vram: [u8; 8*1024],
  pub oam: [u8; 160],

  mode: PpuMode,
  pub frame_ready: Option<()>,

  ctrl: Ctrl,
  stat: Stat,
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
      obj_visible: Vec::with_capacity(10),
      obj_scanline: [const {None}; 160],
      bg_scanline: [0; 160],

      vram: [0; 8*1024],
      oam: [0; 160],

      mode: Default::default(),
      frame_ready: None,

      ctrl: Ctrl::lcd_enabled,
      stat: Stat::empty(),

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
    let old_stat = self.stat;

    match self.mode {
      PpuMode::OamScan => {
        if self.tcycles >= 80 {
          self.oam_scan();
          self.mode = PpuMode::DrawingPixels;
        }
        
        self.tcycles += 1;
      }
      PpuMode::DrawingPixels => {
        if self.tcycles >= 80 + 172 {
          self.fill_bg_scanline();
          self.fill_obj_scanline();
          self.render_scanline();
          
          self.send_lcd_int(Stat::mode0_int);
          self.mode = PpuMode::Hblank;
        }

        self.tcycles += 1;
      }
      PpuMode::Hblank => {
        if self.tcycles >= 456 {
          self.tcycles = 0;

          if self.ly >= 143 {
            bus::send_interrupt(&self.intf, IFlags::vblank);
            self.frame_ready = Some(());

            self.send_lcd_int(Stat::mode1_int);
            self.mode = PpuMode::Vblank;
          } else {
          self.send_lcd_int(Stat::mode2_int);
            self.mode = PpuMode::OamScan;
          };

          self.ly += 1;
        } else {
          self.tcycles += 1;
        }
      }
      PpuMode::Vblank => {
        if self.tcycles >= 456 {
          self.tcycles = 0;
          if self.ly > 153 {
            self.ly = 0;
          self.send_lcd_int(Stat::mode2_int);
            self.mode = PpuMode::OamScan;
          } else {
            self.ly += 1;
          }
        } else {
          self.tcycles += 1;
        }
      }
    }


    if old_stat != self.stat {
      self.send_lyc_int();
    }
  }

  fn send_lcd_int(&mut self, flag: Stat) {
    if self.stat.contains(flag) && self.is_lcd_enabled() {
      bus::send_interrupt(&self.intf, bus::IFlags::lcd);
    }
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

  pub fn tileset_addr(&self, tileset_id: u8) -> u16 {
    match self.ctrl.contains(Ctrl::tileset_addr) {
      true  => VRAM0 + 16*tileset_id as u16,
      false => {
        let offset = tileset_id as i8;
        (VRAM2 as i32 + 16*offset as i32) as u16
      }
    }
  }

  fn vram_read(&self, addr: u16) -> u8 {
    self.vram[(addr - VRAM0) as usize]
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
        self.ctrl = Ctrl::from_bits_retain(val);
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
      }
      0xFF4A => self.wy = val,
      0xFF4B => self.wx = val,
      0xFF47 => self.bgp = val,
      0xFF48 => self.obp0 = val,
      0xFF49 => self.obp1 = val,
      _ => eprintln!("Ppu register write {addr:04X} not implemented"),
    }
  }
}

impl Ppu {
  fn oam_scan(&mut self) {
    self.obj_visible.clear();

    for i in (0..self.oam.len()).step_by(4) {
      let y = self.oam[i];

      if self.ly.wrapping_add(16) >= y
      && self.ly.wrapping_add(16) < y.wrapping_add(self.obj_size())
      {
        let obj = OamObject::new(&self.oam[i..i+4], i as u8/4);
        self.obj_visible.push(obj);
      }

      if self.obj_visible.len() >= 10 { break; }
    }

    // we sort them in reverse (lower to higher), so that we always set for last to the scanline the higher priority object
    self.obj_visible.sort_by(|a, b| {
      if a.x == b.x { b.i.cmp(&a.i) } else { b.x.cmp(&a.x) } 
    });
  }

  fn fill_obj_scanline(&mut self) {
    if !self.ctrl.contains(Ctrl::obj_enabled) { return; }
    self.obj_scanline.fill(None);

    for obj in &self.obj_visible {
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

        let data = ObjEntry { 
          color,
          palette: obj.dmg_palette,
          priority: obj.priority
        };

        self.obj_scanline[x as usize] = Some(data);
      }
    }
  }

  fn fill_bg_scanline(&mut self) {
    if !self.ctrl.contains(Ctrl::bg_wnd_enabled) { return; }
    self.bg_scanline.fill(0);

    let tilemap = self.bg_tilemap();
    let y = self.ly.wrapping_add(self.scy);

    for x in 0..160 {
      let tilemap_x = self.scx.wrapping_add(x);
      let tilemap_id = tilemap + 32 * (y/8) as u16 + (tilemap_x/8) as u16;
      let tileset_id = self.vram_read(tilemap_id);

      let tile_start = self.tileset_addr(tileset_id);
      let tileset_addr = tile_start + 2*(y%8) as u16;

      let tile_lo = self.vram_read(tileset_addr);
      let tile_hi = self.vram_read(tileset_addr+1);

      let lo = (tile_lo >> (7-(tilemap_x%8))) & 1;
      let hi = (tile_hi >> (7-(tilemap_x%8))) & 1;
      let pixel = (hi << 1) | lo;
      self.bg_scanline[x as usize] = pixel;

      // for bit in 0..8 {
      //   let lo = (tile_lo >> (7-bit)) & 1;
      //   let hi = (tile_hi >> (7-bit)) & 1;
      //   let pixel = (hi << 1) | lo;
      //   self.bg_scanline[pixel_x as usize] = pixel;
      //   pixel_x += 1;
      // }
    }
  }

  fn render_scanline(&mut self) {
    let pixels = self.bg_scanline.into_iter()
      .zip(self.obj_scanline.into_iter())
      .enumerate();

    for (x, (bg, obj)) in pixels {
      let obj = obj.unwrap_or_default();

      let color = if self.ctrl.contains(Ctrl::obj_enabled) 
        && obj.color != 0 && (obj.priority || bg == 0)
      {
        self.obj_palette(obj.palette, obj.color)
      } else if self.ctrl.contains(Ctrl::bg_wnd_enabled) {
        self.bg_palette(bg)
      } else {
        self.bg_palette(0)
      };

      self.lcd.set_pixel(x, self.ly as usize, color);
    }
  }
}