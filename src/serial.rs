use bitflags::bitflags;

use crate::bus::InterruptFlags;

bitflags! {
  #[derive(Clone, Copy)]
  struct Flags: u8 {
    const master  = 0b0000_0001;
    const speed   = 0b0000_0010;
    const enabled = 0b1000_0000;
    const unused  = 0b0111_1100;
  }
}

pub struct Serial {
  dummy: u8,
  flags: Flags,
  #[allow(unused)]
  intf: InterruptFlags
}

impl Serial {
  pub fn new(intf: InterruptFlags) -> Self {    
    Self {
      dummy: 0xFF,
      flags: Flags::empty(),
      intf,
    }
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    match addr {
      0xFF01 => self.dummy,
      0xFF02 => (self.flags | Flags::unused).bits(),
      _ => unreachable!()
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    match addr {
      0xFF01 => self.dummy = val,
      0xFF02 => self.flags = Flags::from_bits_retain(val),
      _ => {}
    }
  }
}