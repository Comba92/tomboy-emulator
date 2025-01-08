#![allow(unused)]
use crate::nth_bit;

use bitfield_struct::bitfield;

#[bitfield(u16, order = Lsb)]
struct Period {
  #[bits(8)]
  lo: u8,
  #[bits(3)]
  hi: u8,
  #[bits(5)]
  __: u8,
}

const SQUARE_DUTIES: [[u8; 8]; 4] = [
  [1,1,1,1,1,1,1,0],
  [0,1,1,1,1,1,1,0],
  [0,1,1,1,1,0,0,0],
  [1,0,0,0,0,0,0,1]
];

#[derive(Default, Clone, Copy)]
enum SquareDuty { #[default] Duty12, Duty25, Duty50, Duty75 }
#[derive(Default)]
struct Square {
  enabled: bool,

  sweep_pace: u8,
  sweep_dir: bool,
  sweep_step: u8,

  wave_duty: SquareDuty,
  duty: u8,
  length_initial: u8,
  length_timer: u8,

  volume_initial: u8,
  volume: u8,
  envelope_dir: bool,
  envelope_pace: u8,

  period_initial: Period,
  period: u16,

  length_enabled: bool,
}
impl Square {
  pub fn tick_period(&mut self) {
    self.period += 1;
    if self.period >= 2048 {
      self.period = 2048 - self.period_initial.0;
      self.duty = (self.duty+1) % 8;
    }
  }

  pub fn tick_length(&mut self) {
    if self.length_enabled {
      self.length_timer += 1;
      if self.length_timer >= 64 {
        self.enabled = false;
      }
    }
  }

  pub fn tick_sweep(&mut self) {

  }

  pub fn read(&mut self, addr: u16) -> u8 {
    match addr {
      0 => {
        let mut res = 0;
        res |= self.sweep_step;
        res |= (self.sweep_dir as u8) << 3;
        res |= self.sweep_pace << 4;

        res
      }
      1 => (self.duty as u8) << 6,
      2 => {
        let mut res = 0;
        res |= self.sweep_pace;
        res |= (self.envelope_dir as u8) << 3;
        res |= self.volume_initial << 4;
        
        res
      }
      4 => (self.length_enabled as u8) >> 6,
      _ => 0,
    }
  }
  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0 => {
        self.sweep_step = val & 0b111;
        self.sweep_dir  = ((val >> 3) & 1) == 0;
        self.sweep_pace = (val >> 4) & 0b111;
      }
      1 => {
        self.length_initial = val & 0b1_1111;
        self.wave_duty = match val >> 6 {
          0 => SquareDuty::Duty12,
          1 => SquareDuty::Duty25,
          2 => SquareDuty::Duty50,
          _ => SquareDuty::Duty75,
        };
      }
      2 => {
        self.sweep_pace = val & 0b111;
        self.envelope_dir = val >> 3 != 0;
        self.volume_initial = val >> 4;
        self.volume = self.volume_initial;
      }
      3 => self.period_initial.set_lo(val),
      4 => {
        self.period_initial.set_hi(val & 0b111);
        self.length_enabled = nth_bit(val, 6);
        if nth_bit(val, 7) {
          self.enabled = true;
          self.length_timer = self.length_initial;
          self.period = self.period_initial.0;
          self.duty = 0;
          // TODO: reset envelope
          self.volume = self.volume_initial;
          // TODO: handle sweep trigger 
        }
      }
      _ => {}
    }
  }
}

#[derive(Default)]
pub struct Apu {
  apu_enabled: bool,
  volume_l: u8,
  volume_r: u8,
  tcycles: usize,
  div: u16,

  square1: Square,
  square2: Square,

  samples: Vec<f32>,
}

impl Apu {
  pub fn tick(&mut self) {
    self.div = self.div.wrapping_add(1);
    if self.div == 0 {
      // step envelope
    }
    if self.div % 16384  == 0 {
      self.square1.tick_length();
      self.square2.tick_length();
    }
    if self.div % 32768 == 0 {
      // square sweep
    }

    // The following events occur every N DIV-APU ticks:
    // Envelope sweep	8	64 Hz
    // Sound length	2	256 Hz
    // CH1 freq sweep	4	128 Hz
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    match addr {
      0xFF26 => {
        let mut res = 0;
        res |= (self.apu_enabled as u8) << 7;
        res |= 0b0111_0000;
        // TODO: check channels active
        res
      }
      0xFF25 => {
        // TODO: read channels panning
        0
      }
      0xFF24 => {
        let mut res = 0;
        res |= self.volume_l << 4;
        res |= self.volume_r << 0;
        res
      }
      0xFF10..=0xFF14 => self.square1.read(addr - 0xFF10),
      0xFF16..=0xFF19 => self.square2.read(addr - 0xFF16),
      _ => 0,
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0xFF26 => {
        self.apu_enabled = nth_bit(val, 7);
        // TODO: clear all apu registers, except wave ram and div apu
      }
      0xFF25 => {
        // TODO: set channels panning
      }
      0xFF24 => {
        self.volume_l = (val >> 4) & 0b111;
        self.volume_r = (val >> 0) & 0b111;
      }
      0xFF10..=0xFF14 => self.square1.write(addr - 0xFF10, val),
      0xFF16..=0xFF19 => self.square2.write(addr - 0xFF16, val),
      _ => {}
    }
  }

  pub fn get_samples(&mut self) -> Vec<f32> {
    core::mem::take(&mut self.samples)
  }
}