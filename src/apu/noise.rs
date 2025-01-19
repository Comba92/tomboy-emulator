use crate::nth_bit;

use super::envelope::Envelope;

#[derive(Default)]
pub(super) struct Noise {
  pub enabled: bool,
  pub panning_l: bool,
  pub panning_r: bool,

  pub env: Envelope,

  length_initial: u8,
  length_enabled: bool,
  length_timer: u8,

  div: u8,
  timer: u8,
  shift: u8,
  lfsr: u16,
  lfsr_7bit: bool,
}

impl Noise {
  pub fn disable(&mut self) {
    let div_code = if self.div == 0 {
      8
    } else { self.div << 4 };

    self.timer = div_code << self.shift;
  }

  pub fn get_sample(&self) -> (f32, f32) {
    let sample = if self.enabled {
      ((!self.lfsr & 1) * self.env.volume as u16) as f32
    } else { 0.0 };

    let l = if self.panning_l { sample } else { 0.0 };
    let r = if self.panning_r { sample } else { 0.0 };
    (l, r)
  }

  pub fn tick_period(&mut self) {
    if !self.enabled { return; }

    if self.timer > 0 {
      self.timer -= 1;

      if self.timer == 0 {
        let div_code = if self.div == 0 {
          8
        } else { self.div << 4 };

        self.timer = div_code << self.shift;
        let res = (self.lfsr & 1) ^ ((self.lfsr & 0b10) >> 1);
        self.lfsr = (self.lfsr >> 1) | (res << 14);

        if self.lfsr_7bit {
          self.lfsr &= !0b0010_0000;
          self.lfsr |= res << 6;
        }
      }
    }
  }

  pub fn tick_length(&mut self) {
    if self.length_enabled && self.length_timer > 0 {
      self.length_timer -= 1;

      if self.length_timer == 0 {
        self.enabled = false;
      }
    }
  }

  pub fn read(&self, addr: u16) -> u8 {
    match addr {
      0xFF21 => self.env.read(),
      0xFF22 => {
        let mut res = 0;
        res |= self.div;
        res |= (self.lfsr as u8) >> 3;
        res |= self.shift >> 4;
        res
      }
      0xFF23 => ((self.length_enabled as u8) >> 6) | 0b1011_1111,
      _ => unreachable!()
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0xFF20 => self.length_initial = 64 - (val & 0b11_1111),
      0xFF21 => {
        self.env.write(val);
        self.enabled = self.env.is_dac_enabled();
      }
      0xFF22 => {
        self.div = val & 0b111;
        self.lfsr_7bit = nth_bit(val, 3);
        self.shift = (val >> 4) & 0b1111;
      }
      0xFF23 => {
        self.length_enabled = nth_bit(val, 6);

        // Trigger
        if self.env.is_dac_enabled() && nth_bit(val, 7) {
          if self.length_timer == 0 {
            self.length_timer = self.length_initial;
          }

          self.env.trigger();
          self.enabled = self.env.is_dac_enabled();
          self.lfsr = 0x7FFF;
        }
      }
      _ => unreachable!()
    }
  }
}