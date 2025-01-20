use crate::nth_bit;

#[derive(Default, Clone, Copy)]
enum OutputLevel { #[default] Mute, Max, Half, Quarter }

#[derive(Default)]
pub(super) struct Wave {
  pub enabled: bool,
  pub dac_enabled: bool,
  pub panning_l: bool,
  pub panning_r: bool,

  output: OutputLevel,

  length_initial: u16,
  length_enabled: bool,
  length_timer: u16,

  period_initial: u16,
  timer: u16,
  position: u8,

  ram: [u8; 16],
  ram_enabled: bool,
  buffer: u8,
}

impl Wave {
  pub fn disable(&mut self) {
    self.timer = 2048;
    self.enabled = false;
  }

  pub fn get_sample(&self) -> (f32, f32) {
    let sample = if self.enabled {
      match self.output {
        OutputLevel::Mute => 0,
        OutputLevel::Max => self.buffer,
        OutputLevel::Half => self.buffer >> 1,
        OutputLevel::Quarter => self.buffer >> 2,
      }
    } else { 0 } as f32;

    let l = if self.panning_l { sample } else { 0.0 };
    let r = if self.panning_r { sample } else { 0.0 };
    (l, r)
  }

  pub fn tick_period(&mut self) {
    self.ram_enabled = false;
    if !self.enabled { return; }

    if self.timer > 0 {
      self.timer -= 1;
      if self.timer == 0 {
        self.timer = 2048 - self.period_initial;
        self.position = (self.position + 1) % 32;

        self.buffer = if self.position % 2 == 0 {
          self.ram[self.position as usize >> 1] >> 4
        } else {
          self.ram[self.position as usize >> 1] & 0x0F
        };

        self.ram_enabled = true;
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
      0xFF1A => self.dac_enabled as u8 | 0b0111_1111,
      0xFF1C => (self.output as u8) << 5 | 0b1001_1111,
      0xFF1E => ((self.length_enabled as u8) << 6) | 0b1011_1111,
      _ => unreachable!()
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0xFF1A => {
        self.dac_enabled = !nth_bit(val, 7);
        self.enabled = self.enabled && self.dac_enabled;
      }
      0xFF1B => self.length_initial = 256 - val as u16,
      0xFF1C => self.output = match (val >> 5) & 0b11 {
        0 => OutputLevel::Mute,
        1 => OutputLevel::Max,
        2 => OutputLevel::Half,
        _ => OutputLevel::Quarter,
      },
      0xFF1D => self.period_initial = (self.period_initial & 0xF00) | (val as u16),
      0xFF1E => {
        self.period_initial = (self.period_initial & 0x0FF) | ((val as u16 & 0b111) << 8);
        self.length_enabled = nth_bit(val, 6);

        if nth_bit(val, 7) {
          self.enabled = self.dac_enabled;
          self.timer = 2048 - self.period_initial;

          if self.length_timer == 0 {
            self.length_timer = self.length_initial;
          }

          self.position = 1;
        }
      }

      _ => unreachable!()
    }
  }

  pub fn ram_read(&self, addr: u16) -> u8 {
    if !self.enabled {
      self.ram[addr as usize >> 1]
    } else { 0 }
  }

  pub fn ram_write(&mut self, addr: u16, val: u8) {
    if !self.enabled {
      self.ram[addr as usize] = val;
    }
  } 
}