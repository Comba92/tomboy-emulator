pub mod gb;

pub mod cpu;
pub mod instr;

pub mod bus;

pub mod timer;
pub mod serial;
pub mod joypad;
pub mod apu;

pub mod ppu;
pub mod frame;

pub mod cart;
pub mod mbc;

pub fn nth_bit(value: u8, bit: u8) -> bool {
  value & (1 << bit) != 0
}

fn lsb(val: u8) -> bool { val & 1 != 0 }
fn msb(val: u8) -> bool { val & 0x80 != 0 }