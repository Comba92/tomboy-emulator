// CPU freq: 4.194304 MHz = 4194304 Hz
// Timer divider: 16384 Hz
// CPU freq / Timer divider =  4194304 Hz / 16384 Hz = 256

use bitflags::bitflags;

use crate::bus;


bitflags! {
  #[derive(Default, Clone, Copy)]
  struct Flags: u8 {
    const unused = 0b1111_1000;
    const enable = 0b100;
    const clock  = 0b011;
  }
}

pub struct Timer {
  div: u16,
  tima: u8,
  tima_clock: u16,
  tima_overflow_delay: u8,
  tima_just_reloaded: bool,
  tma: u8,
  tac: Flags,
  intf: bus::InterruptFlags,
}

impl Timer {
  pub fn new(intf: bus::InterruptFlags) -> Self {
    Self {
      div: 0xABCC,
      tima: 0,
      tima_clock: 1 << 9,
      tima_overflow_delay: 0,
      tima_just_reloaded: false,
      tma: 0,
      tac: Flags::default(),
      intf,
    }
  }

  fn tick_tima(&mut self) {
    if self.tac.contains(Flags::enable) {
      let (res, overflow) = self.tima.overflowing_add(1);
      self.tima = res;
      self.tima_overflow_delay = if overflow { 4 } else { 0 };
    }
  }

  pub fn tick(&mut self) {
    self.tima_just_reloaded = false;

    if self.tima_overflow_delay > 0 {
        self.tima_overflow_delay -= 1;
        if self.tima_overflow_delay == 0 {
          self.tima = self.tma;
          self.tima_just_reloaded = true;
          bus::send_interrupt(&self.intf, bus::IFlags::timer);
        }
    }
      
    let new_div = self.div.wrapping_add(1);
    if self.div & self.tima_clock != 0 && new_div & self.tima_clock == 0 {
      self.tick_tima();
    }

    self.div = new_div;
  }

  fn tima_clock_bit(&self) -> u16 {
    match self.tac.bits() & 0b11 {
      0b00 => 1 << 9,
      0b01 => 1 << 3,
      0b10 => 1 << 5,
      0b11 => 1 << 7,
      _ => unreachable!()
    }
  }

  pub fn read(&self, addr: u16) -> u8 {
    match addr {
      0xFF04 => (self.div >> 8) as u8,
      0xFF05 => self.tima,
      0xFF06 => self.tma,
      0xFF07 => (self.tac | Flags::unused).bits(),
      _ => unreachable!(),
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0xFF04 => {
        if self.div & self.tima_clock != 0 {
          self.tick_tima();
        }

        self.div = 0;
      }
      0xFF05 => {
        // https://gbdev.io/pandocs/Timer_Obscure_Behaviour.html#timer-overflow-behavior

        // ignore write if tima just got reloaded
        if !self.tima_just_reloaded { 
            self.tima = val;
        }

        // Cancel tima update after overflow
        self.tima_overflow_delay = 0;
      }
      0xFF06 => {
        self.tma = val;
        if self.tima_just_reloaded {
            self.tima = val;
        }
      }
      0xFF07 => {
        self.tac = Flags::from_bits_retain(val & 0b111);
        self.tima_clock = self.tima_clock_bit();

        if self.div & self.tima_clock != 0 && !self.tac.contains(Flags::enable) {
          self.tick_tima();
        }

        // TODO: TAC TIMA increment glitch
      }
      _ => {}
    }
  }
}