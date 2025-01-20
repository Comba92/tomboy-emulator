#[derive(Default)]
pub(super) struct Envelope {
  pub volume_initial: u8,
  pub volume: u8,
  pub direction: bool,
  timer: u8,
  pub period: u8,
}

impl Envelope {
  pub fn tick(&mut self) {
    if self.period == 0 { return; }

    if self.timer > 0 {
      self.timer -= 1;
      
      if self.timer == 0 {
        self.timer = self.period;
        if self.volume < 15 && self.direction {
          self.volume += 1;
        } else if self.volume > 0 && !self.direction {
          self.volume -= 1;
        }
      }
    }
  }

  pub fn is_dac_enabled(&self) -> bool {
    !(self.volume_initial == 0 && !self.direction)
  }

  pub fn trigger(&mut self) {
    self.timer = self.period;
    self.volume = self.volume_initial;
  }

  pub fn read(&self) -> u8 {
    let mut res = 0;
    res |= self.period;
    res |= (self.direction as u8) << 3;
    res |= self.volume_initial << 4;
    
    res
  }

  pub fn write(&mut self, val: u8) {
    self.period = val & 0b111;
    self.direction = (val >> 3) != 0;
    self.volume_initial = val >> 4;
    self.volume = self.volume_initial;
  }
}