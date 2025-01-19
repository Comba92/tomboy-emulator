use crate::nth_bit;

use super::envelope::Envelope;

const SQUARE_DUTIES: [[u8; 8]; 4] = [
  // [1,1,1,1,1,1,1,0],
  // [0,1,1,1,1,1,1,0],
  // [0,1,1,1,1,0,0,0],
  // [1,0,0,0,0,0,0,1]
  [0,0,0,0,0,0,0,1],
  [0,0,0,0,0,0,1,1],
  [0,0,0,0,1,1,1,1],
  [1,1,1,1,1,1,0,0],
];

#[derive(Default)]
pub(super) struct Square {
  pub enabled: bool,
  pub panning_l: bool,
  pub panning_r: bool,

  pub env: Envelope,
  pub sweep: Sweep,

  wave_duty: u8,
  duty: u8,
  length_initial: u8,
  length_timer: u8,
  length_enabled: bool,

  period_initial: u16,
  timer: u16,
}

#[derive(Default)]
pub(super) struct Sweep {
  enabled: bool,
  period: u8,
  direction: bool,
  shift: u8,
  timer: u8,
  shadow: u16,
}

impl Square {
  pub fn disable(&mut self) {
    self.timer = 2048;
  }

  pub fn get_sample(&self) -> (f32, f32) {
    let sample = if self.enabled {
      let duty = 
      SQUARE_DUTIES[self.wave_duty as usize][self.duty as usize];
      ((duty * self.env.volume) as f32 / 7.5) - 1.0
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
        self.timer = 2048 - self.period_initial;
        self.duty = (self.duty+1) % 8;
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

  pub fn tick_sweep(&mut self) {
    if self.sweep.timer > 0 {
      self.sweep.timer -= 1;

      if self.sweep.timer == 0 {
        self.sweep.timer = if self.sweep.period == 0 {
          8
        } else { self.sweep.period };
      }

      if self.sweep.enabled && self.sweep.period > 0 {
        let freq = self.sweep_freq_get_and_check();

        if self.enabled && self.sweep.shift > 0 {
          self.sweep.shadow = freq;
          self.timer = freq;
        }
      }
    }
  }

  fn sweep_freq_get_and_check(&mut self) -> u16 {
    let mut freq = self.sweep.shadow >> self.sweep.shift;

    if self.sweep.direction {
      freq -= self.sweep.shadow;
    } else {
      freq += self.sweep.shadow;
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
        res |= self.sweep.shift;
        res |= (self.sweep.direction as u8) << 3;
        res |= self.sweep.period << 4;
        res |= 0x80;

        res
      }
      1 => (self.duty as u8) << 6 | 0b0011_1111,
      2 => self.env.read(),
      4 => (self.length_enabled as u8) << 6 | 0b1011_1111,
      _ => unreachable!(),
    }
  }
  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0 => {
        self.sweep.shift = val & 0b111;
        self.sweep.direction  = ((val >> 3) & 1) == 0;
        self.sweep.period = (val >> 4) & 0b111;
      }
      1 => {
        self.length_initial = 64 - (val & 0b11_1111);
        self.wave_duty = val >> 6;
      }
      2 => {
        self.env.write(val);
        self.enabled = self.env.is_dac_enabled();
      }
      3 => self.period_initial = (self.period_initial & 0xF00) | (val as u16),
      4 => {
        self.period_initial = (self.period_initial & 0x0FF) | ((val as u16 & 0b111) << 8);
        self.length_enabled = nth_bit(val, 6);
        
        // Trigger
        if self.env.is_dac_enabled() && nth_bit(val, 7) {
          if self.length_timer == 0 {
            self.length_timer = self.length_initial;
          }
          
          self.timer = 2048 - self.period_initial;
          self.duty = 0;

          self.env.trigger();
          self.enabled = self.env.is_dac_enabled();

          self.sweep.shadow = self.timer;
          self.sweep.timer = self.sweep.period;
          self.sweep.enabled = self.sweep.period > 0 || self.sweep.shift > 0;
          if self.sweep.shift > 0 {
            self.sweep_freq_get_and_check();
          }
        }
      }
      _ => unreachable!()
    }
  }
}