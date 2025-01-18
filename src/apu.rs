use bitfield_struct::bitfield;
use square::Square;

use crate::nth_bit;

mod envelope;
mod square;

#[bitfield(u16)]
struct Period {
  #[bits(8)]
  lo: u8,
  #[bits(3)]
  hi: u8,
  #[bits(5)]
  __: u8,
}

#[derive(Default)]
pub struct Apu {
  apu_enabled: bool,
  volume_l: u8,
  volume_r: u8,
  volumef_l: f32,
  volumef_r: f32,

  tcycles: usize,
  frame_count: u8,

  square1: Square,
  square2: Square,

  samples: Vec<f32>,
  samples_cycles: f64
}

const CYCLES_PER_SAMPLE: f64 = 4194304.0 / 44100.0;

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

        let out_l = ((sq1_l + sq2_l)) * self.volumef_l;
        let out_r = ((sq1_r + sq2_r)) * self.volumef_r;

        self.samples.push(out_l as f32);
        self.samples.push(out_r as f32);
      }
    } else {
      self.samples_cycles += 1.0;
    }

    if !self.apu_enabled { return; }

    self.square1.tick_period();
    self.square2.tick_period();

    // length
    if self.tcycles % 16384 == 0 {
      // self.square1.tick_length();
      // self.square2.tick_length();
    }

    if self.tcycles % 32768 == 0 {
      // self.square1.tick_sweep();
    }

    // env
    if self.tcycles >= 65536 {
      self.tcycles = 0;

      // self.square1.env.tick();
      // self.square2.env.tick();
    }

    self.tcycles += 1;

    // if self.tcycles >= 8192 {
    //   self.tcycles = 0;
    //   self.frame_count = (self.frame_count + 1) % 8;

    //   // The following events occur every N DIV-APU ticks:
    //   // Envelope sweep	8	64 Hz
    //   // Sound length	2	256 Hz
    //   // CH1 freq sweep	4	128 Hz

    //   if self.frame_count == 7 {
    //     self.square1.env.tick();
    //     self.square2.env.tick();
    //   }

    //   if self.frame_count % 2 == 0 {
    //     self.square1.tick_length();
    //     self.square2.tick_length();

    //     if self.frame_count == 2 || self.frame_count == 6 {
    //       self.square1.tick_sweep();
    //     }
    //   }
    // } else {
    //   self.tcycles += 1;
    // }
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    if !self.apu_enabled && addr != 0xFF26 {
      return 0xFF;
    }

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
        res |=  self.square1.panning_r as u8;
        res |= (self.square2.panning_r as u8) << 1;
        res |= (self.square1.panning_l as u8) << 4;
        res |= (self.square2.panning_l as u8) << 5;
        // TODO: read channels panning
        res
      }
      // NR52
      0xFF26 => {
        let mut res = 0;
        res |= (self.apu_enabled as u8) << 7;
        res |= 0b0111_0000; // open bus
        res |=  self.square1.enabled as u8;
        res |= (self.square2.enabled as u8) << 1;
        // TODO: check channels active
        res
      }

      0xFF10..=0xFF14 => self.square1.read(addr - 0xFF10),
      0xFF16..=0xFF19 => self.square2.read(addr - 0xFF16),
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
        self.volumef_l = self.volume_l as f32 / 8.0;
        self.volumef_r = self.volume_r as f32 / 8.0;
      }
      // NR51
      0xFF25 => {
        self.square1.panning_r = nth_bit(val, 0);
        self.square2.panning_r = nth_bit(val, 1);

        self.square1.panning_l = nth_bit(val, 4);
        self.square2.panning_l = nth_bit(val, 5);
        // TODO: set channels panning
      }
      // NR52
      0xFF26 => {
        self.apu_enabled = nth_bit(val, 7);
        if !self.apu_enabled {
          self.square1.panning_l = false;
          self.square1.panning_r = false;
          self.square2.panning_l = false;
          self.square2.panning_r = false;

          self.volume_l = 1;
          self.volume_r = 1;
          self.volumef_l = 1.0;
          self.volumef_r = 1.0;
        }
        // TODO: if apu is disabled, clear all apu registers, except wave ram and div apu
      }

      0xFF10..=0xFF14 => self.square1.write(addr - 0xFF10, val),
      0xFF16..=0xFF19 => self.square2.write(addr - 0xFF16, val),
      _ => {}
    }
  }

  pub fn consume_samples(&mut self) -> Vec<f32> {
    core::mem::take(&mut self.samples)
  }
}