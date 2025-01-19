use noise::Noise;
use square::Square;

use crate::nth_bit;

mod envelope;
mod square;
mod noise;

#[derive(Default)]
pub struct Apu {
  apu_enabled: bool,
  volume_l: u8,
  volume_r: u8,
  volumef_l: f32,
  volumef_r: f32,

  pub tcycles: usize,

  square1: Square,
  square2: Square,
  noise: Noise, 

  samples: Vec<f32>,
  samples_cycles: f64
}

const CPU_CYCLES: usize = 4194304;
const CYCLES_PER_SAMPLE: f64 = CPU_CYCLES as f64 / 44100.0;

impl Apu {
  pub fn tick(&mut self) {
    if self.samples_cycles >= CYCLES_PER_SAMPLE {
      self.samples_cycles -= CYCLES_PER_SAMPLE;

      if !self.apu_enabled {
        self.samples.push(0.0);
        self.samples.push(0.0);
      } else {
        let (sq1_l, sq1_r) = self.square1.get_sample();
        let (sq2_l, sq2_r) = self.square2.get_sample();
        let (n_l, n_r) = self.noise.get_sample();

        let out_l = ((sq1_l + sq2_l + n_l) / 3.0) * 1.0;
        let out_r = ((sq1_r + sq2_r + n_r) / 3.0) * 1.0;

        self.samples.push(out_l as f32);
        self.samples.push(out_r as f32);
      }
    } else {
      self.samples_cycles += 1.0;
    }

    if !self.apu_enabled { return; }

    if self.tcycles % 4 == 0 {
      self.square1.tick_period();
      self.square2.tick_period();
      self.noise.tick_period();
    }

    if self.tcycles % CPU_CYCLES/256 == 0 {
      self.square1.tick_length();
      self.square2.tick_length();
      self.noise.tick_length();
    }

    if self.tcycles % CPU_CYCLES/128 == 0 {
      self.square1.tick_sweep();
    }

    if self.tcycles % CPU_CYCLES/64 == 0 {
      self.square1.env.tick();
      self.square2.env.tick();
      self.noise.env.tick();
    }

    self.tcycles += 1;
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    // if !self.apu_enabled && addr != 0xFF26 {
    //   return 0xFF;
    // }

    match addr {
      // NR50
      0xFF24 => {
        let mut res = 0;
        res |= self.volume_l << 4;
        res |= self.volume_r << 0;

        res
      }
      // NR51
      0xFF25 => {
        let mut res = 0;
        res |= (self.square1.panning_r as u8) << 0;
        res |= (self.square2.panning_r as u8) << 1;
        // res |= (self.wave.panning_r as u8) << 2;
        res |= (self.noise.panning_r as u8) << 3;
        res |= (self.square1.panning_l as u8) << 4;
        res |= (self.square2.panning_l as u8) << 5;
        // res |= (self.wave.panning_l as u8) << 7;
        res |= (self.noise.panning_l as u8) << 7;

        res
      }
      // NR52
      0xFF26 => {
        let mut res = 0;
        res |= (self.apu_enabled as u8) << 7;
        res |= 0b0111_0000; // open bus

        res |=  self.square1.enabled as u8;
        res |= (self.square2.enabled as u8) << 1;
        // res |= (self.wave.enabled as u8) << 2;
        res |= (self.noise.enabled as u8) << 3;
        
        res
      }

      0xFF10..=0xFF14 => self.square1.read(addr - 0xFF10),
      0xFF15..=0xFF19 => self.square2.read(addr - 0xFF15),
      0xFF20..=0xFF23 => self.noise.read(addr),
      _ => 0xFF,
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    if !self.apu_enabled && addr != 0xFF26 { return; }

    match addr {
      // NR50
      0xFF24 => {
        // A value of 0 is treated as a volume of 1 (very quiet), 
        // and a value of 7 is treated as a volume of 8 (no volume reduction). 
        // Importantly, the amplifier never mutes a non-silent input.
        self.volume_l = ((val >> 4) & 0b111) + 1;
        self.volume_r = ((val >> 0) & 0b111) + 1;
        
        // audio has to be normalized
        self.volumef_l = (self.volume_l as f32 / 4.5) - 1.0;
        self.volumef_r = (self.volume_r as f32 / 4.5) - 1.0;
      }
      // NR51
      0xFF25 => {
        self.square1.panning_r = nth_bit(val, 0);
        self.square2.panning_r = nth_bit(val, 1);
        // self.wave.panning_r = nth_bit(val, 2);
        self.noise.panning_r   = nth_bit(val, 3);

        self.square1.panning_l = nth_bit(val, 4);
        self.square2.panning_l = nth_bit(val, 5);
        // self.wave.panning_l = nth_bit(val, 6);
        self.noise.panning_l   = nth_bit(val, 7);
      }
      
      // NR52
      0xFF26 => {
        self.apu_enabled = nth_bit(val, 7);
        if !self.apu_enabled {
          self.square1.disable();
          self.square2.disable();
          self.noise.disable();

          self.volume_l = 0;
          self.volume_r = 0;
          self.volumef_l = 0.0;
          self.volumef_r = 0.0;

          self.square1.panning_r = false;
          self.square2.panning_r = false;
          // self.wave.panning_r = false;
          self.noise.panning_r   = false;

          self.square1.panning_l = false;
          self.square2.panning_l = false;
          // self.wave.panning_l = false;
          self.noise.panning_l   = false;
        }
      }

      0xFF10..=0xFF14 => self.square1.write(addr - 0xFF10, val),
      // BE CAREFUL HERE: square2 doesn't have the sweep register
      0xFF15..=0xFF19 => self.square2.write(addr - 0xFF15, val),
      0xFF20..=0xFF23 => self.noise.write(addr, val),
      _ => {}
    }
  }

  pub fn consume_samples(&mut self) -> Vec<f32> {
    core::mem::take(&mut self.samples)
  }
}