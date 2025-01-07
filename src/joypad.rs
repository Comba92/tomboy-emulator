use bitflags::bitflags;

use crate::bus;

bitflags! {
  pub struct Flags: u8 {
    const start_down = 0b00_1000;
    const select_up  = 0b00_0100;
    const b_left     = 0b00_0010;
    const a_right    = 0b00_0001;
  }
}

#[derive(PartialEq)]
enum JoypadSelect { None, Dpad, Buttons, Both }
pub struct Joypad {
  selected: JoypadSelect,
  buttons: Flags,
  dpad:    Flags,
  intf: bus::InterruptFlags,
}

impl Joypad {
  pub fn new(intf: bus::InterruptFlags) -> Self {
    Self {
      selected: JoypadSelect::None,
      buttons: Flags::all(),
      dpad: Flags::all(),
      intf,
    }
  }

  pub fn button_pressed(&mut self, button: Flags) {
    if self.selected == JoypadSelect::Buttons {
      bus::send_interrupt(&self.intf, bus::IFlags::joypad);
    }

    self.buttons.remove(button);
  }

  pub fn button_released(&mut self, button: Flags) {
    self.buttons.insert(button);
  }

  pub fn dpad_pressed(&mut self, button: Flags) {
    if self.selected == JoypadSelect::Dpad {
      bus::send_interrupt(&self.intf, bus::IFlags::joypad);
    }

    self.dpad.remove(button);
  }

  pub fn dpad_released(&mut self, button: Flags) {
    self.dpad.insert(button);
  }

  pub fn read(&self) -> u8 {
    match self.selected {
      JoypadSelect::Both => 0xFF,
      JoypadSelect::Dpad => self.dpad.bits() & 0b1111,
      JoypadSelect::Buttons => self.buttons.bits() & 0b1111,
      _ => 0xFF,
    }
  }

  pub fn write(&mut self, val: u8) {
    self.selected = match (val >> 4) & 0b11 {
      0b00 => JoypadSelect::None,
      0b01 => JoypadSelect::Buttons,
      0b10 => JoypadSelect::Dpad,
      _ => JoypadSelect::Both,
    };
  }
}