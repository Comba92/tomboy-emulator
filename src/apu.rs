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

#[derive(Default, Clone, Copy)]
enum PulseDuty { #[default] Duty12, Duty25, Duty50, Duty75 }
#[derive(Default)]
struct Pulse {
  sweep_pace: u8,
  sweep_dir: bool,
  sweep_step: u8,

  duty: PulseDuty,
  length_initial: u8,
  length_count: u8,

  volume_initial: u8,
  volume: u8,
  envelope_dir: bool,
  envelope_pace: u8,

  period: Period,
  count: u16,

  length_enabled: bool,
}
impl Pulse {
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
        self.duty = match val >> 6 {
          0 => PulseDuty::Duty12,
          1 => PulseDuty::Duty25,
          2 => PulseDuty::Duty50,
          _ => PulseDuty::Duty75,
        };
      }
      2 => {
        self.sweep_pace = val & 0b111;
        self.envelope_dir = val >> 3 != 0;
        self.volume_initial = val >> 4;
        self.volume = self.volume_initial;
      }
      3 => self.period.set_lo(val),
      4 => {
        self.period.set_hi(val & 0b111);
        self.length_enabled = nth_bit(val, 6);
        // TODO: trigger if bit 7 is set
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

  pulse1: Pulse,
  pulse2: Pulse,
}

impl Apu {
  pub fn read(&mut self, addr: u16) -> u8 {
    match addr {
      0xFF26 => {
        let mut res = 0;
        res |= (self.apu_enabled as u8) << 7;
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
      0xFF10..=0xFF14 => self.pulse1.read(addr - 0xFF10),
      0xFF16..=0xFF19 => self.pulse2.read(addr - 0xFF16),
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
      0xFF10..=0xFF14 => self.pulse1.write(addr - 0xFF10, val),
      0xFF16..=0xFF19 => self.pulse2.write(addr - 0xFF16, val),
      _ => {}
    }
  }
}