use crate::nth_bit;

use super::{envelope::Envelope, Period};

const SQUARE_DUTIES: [[u8; 8]; 4] = [
  [1,1,1,1,1,1,1,0],
  [0,1,1,1,1,1,1,0],
  [0,1,1,1,1,0,0,0],
  [1,0,0,0,0,0,0,1]
  // [0,0,0,0,0,0,0,1],
  // [0,0,0,0,0,0,1,1],
  // [0,0,0,0,1,1,1,1],
  // [1,1,1,1,1,1,0,0],
];

#[derive(Default)]
pub(super) struct Square {
  pub enabled: bool,
  pub panning_l: bool,
  pub panning_r: bool,

  sweep_enabled: bool,
  sweep_period: u8,
  sweep_direction: bool,
  sweep_shift: u8,
  sweep_timer: u8,
  sweep_shadow: u16,
  pub env: Envelope,

  wave_duty: u8,
  duty: u8,
  length_initial: u8,
  length_timer: u8,
  length_enabled: bool,


  period_initial: Period,
  period: u16,
}

impl Square {
  pub fn get_sample(&self) -> (f32, f32) {
    let sample = if self.enabled {
      let duty = 
        SQUARE_DUTIES[self.wave_duty as usize][self.duty as usize];
      
      // ((duty * self.env.volume) as f32 / 7.5) - 1.0
      ((duty * 2) - 1) as f32 * self.env.volume as f32 / 15.0
    } else {
      0.0
    };

    let l = if self.panning_l { sample } else { 0.0 };
    let r = if self.panning_r { sample } else { 0.0 };
    (l, r)
  }

  pub fn tick_period(&mut self) {
    if !self.enabled { return; }

    if self.period > 0 {
      self.period -= 1;
      
      if self.period == 0 {
        self.period = (2048 - self.period_initial.0) * 4;
        self.duty = (self.duty+1) % 8;
      }
    }
  }

  pub fn tick_length(&mut self) {
    if self.length_timer > 0 {
      self.length_timer -= 1;

      if self.length_timer == 0 {
        self.enabled = false;
      }
    }
  }

  pub fn tick_sweep(&mut self) {
    if self.sweep_timer > 0 {
      self.sweep_timer -= 1;

      if self.sweep_timer == 0 {
        self.sweep_timer = self.sweep_period.max(8);
      }

      if self.sweep_enabled && self.sweep_period > 0 {
        let freq = self.sweep_freq_get_and_check();

        if self.enabled && self.sweep_shift > 0 {
          self.sweep_shadow = freq;
          self.period = freq;
        }
      }
    }
  }

  fn sweep_freq_get_and_check(&mut self) -> u16 {
    let mut freq = self.sweep_shadow >> self.sweep_shift;

    if self.sweep_direction {
      freq -= self.sweep_shadow;
    } else {
      freq += self.sweep_shadow;
    }

    if freq >= 2048 {
      self.enabled = false;
    }

    freq
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    match addr {
      0 => {
        let mut res = 0;
        res |= self.sweep_shift;
        res |= (self.sweep_direction as u8) << 3;
        res |= self.sweep_period << 4;
        res |= 0x80;

        res
      }
      1 => (self.duty as u8) << 6 | 0b0011_1111,
      2 => self.env.read(),
      4 => (self.length_enabled as u8) >> 6 | 0b1011_1111,
      _ => 0xFF,
    }
  }
  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0 => {
        self.sweep_shift = val & 0b111;
        self.sweep_direction  = ((val >> 3) & 1) == 0;
        self.sweep_period = (val >> 4) & 0b111;
      }
      1 => {
        self.length_initial = val & 0b11_1111;
        self.wave_duty = val >> 6;
      }
      2 => {
        self.env.write(val);
        self.enabled = self.env.is_dac_enabled();
      }
      3 => self.period_initial.set_lo(val),
      4 => {
        self.period_initial.set_hi(val & 0b111);
        self.length_enabled = nth_bit(val, 6);

        // Trigger
        if self.env.is_dac_enabled() && nth_bit(val, 7) {
          if self.length_timer == 0 {
            self.length_timer = 64 - self.length_initial;
          }
          
          self.period = (2048 - self.period_initial.0) * 4;
          self.duty = 0;
          self.env.trigger();
          self.enabled = self.env.is_dac_enabled();

          self.sweep_shadow = self.period;
          self.sweep_timer = self.sweep_period.max(8);
          self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
          if self.sweep_shift > 0 {
            self.sweep_freq_get_and_check();
          }
        }
      }
      _ => {}
    }
  }
}