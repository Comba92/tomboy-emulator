// CPU freq: 4.194304 MHz = 4194304 Hz
// Timer divider: 16384 Hz
// CPU freq / Timer divider =  4194304 Hz / 16384 Hz = 256

use bitflags::bitflags;

use crate::bus::{IFlags, InterruptFlags};

bitflags! {
    #[derive(Default)]
    struct Flags: u8 {
        const enable = 0b100;
        const clock  = 0b011;
    }
}

pub struct Timer {
    div: u8,
    tima: u8,
    tma: u8,
    tma_write: Option<u8>,
    tac: Flags,
    cycles: usize,
    intf: InterruptFlags,
}

impl Timer {
    pub fn new(intf: InterruptFlags) -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tma_write: None,
            tac: Flags::default(),
            cycles: 0,
            intf,
        }
    }

    pub fn tick(&mut self) {
        self.cycles += 1;

        if self.cycles % 256 == 0 {
            self.div = self.div.wrapping_add(1);
        }

        if (self.cycles + 0x100) % self.tima_clock() == 0 && self.tac.contains(Flags::enable) {
            let (res, overflow) = self.tima.overflowing_add(1);
            let tma = self.tma_write.take().unwrap_or(self.tma);
            self.tima = if overflow { tma } else { res };
            
            self.intf.borrow_mut().insert(IFlags::timer);
        } else {
            self.tma = self.tma_write.take().unwrap_or(self.tma);
        }
    }

    fn tima_clock(&mut self) -> usize {
        match self.tac.bits() & 0b11 {
            0b00 => 256,
            0b01 => 4,
            0b10 => 16,
            0b11 => 64,
            _ => unreachable!()
        }
    }

    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => self.div,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac.bits(),
            _ => unreachable!(),
        }
    }

    pub fn write_reg(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF04 => self.div = 0,
            0xFF05 => self.tima = val,
            0xFF06 => self.tma_write = Some(val),
            0xFF07 => self.tac = Flags::from_bits_truncate(val & 0b111),
            _ => {}
        }
    }
}