use bitflags::bitflags;

use crate::bus;

bitflags! {
  struct Flags: u8 {
    const buttons = 0b10_0000;
    const dpad    = 0b01_0000;
    const start_down = 0b00_1000;
    const select_up  = 0b00_0100;
    const b_left     = 0b00_0010;
    const a_right    = 0b00_0001;
  }
}

pub struct Joypad {
  flags: Flags,
  intf: bus::InterruptFlags,
}

impl Joypad {
  pub fn new(intf: bus::InterruptFlags) -> Self {
    Self {
      flags: Flags::all(),
      intf,
    }
  }

  pub fn button_pressed(&mut self, button: Flags) {
    // TODO: interrupt sending logic (not simple)

    self.flags.remove(button);
  }

  pub fn button_released(&mut self, button: Flags) {
    self.flags.insert(button);
  }

  pub fn read(&self) -> u8 {
    if self.flags.contains(Flags::buttons) && self.flags.contains(Flags::dpad) {
      0xFF
    } else { self.flags.bits() & 0b1111 }
  }

  pub fn write(&mut self, val: u8) {
    self.flags.set(Flags::buttons, val & 0b0010_0000 != 0);
    self.flags.set(Flags::dpad,    val & 0b0001_0000 != 0);
  }
}