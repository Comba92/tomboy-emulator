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
    last_div: u16,
    tima: u8,
    tma: u8,
    tma_overflow_delay: bool,
    tac: Flags,
    mcycles: usize,
    intf: bus::InterruptFlags,
}

impl Timer {
    pub fn new(intf: bus::InterruptFlags) -> Self {
        Self {
            div: 0xAC00,
            last_div: 0,
            tima: 0,
            tma: 0,
            tma_overflow_delay: false,
            tac: Flags::default(),
            mcycles: 0,
            intf,
        }
    }

    pub fn tick(&mut self) {
        self.mcycles += 1;

        self.last_div = self.div;
        self.div = self.div.wrapping_add(1);

        if self.tma_overflow_delay {
            self.tma_overflow_delay = false;
            self.tima = self.tma;
            bus::send_interrupt(&self.intf, bus::IFlags::timer);
        }
        
        if self.mcycles % self.tima_clock() == 0 
        //&& self.tac_enabled() {
        && self.tac.contains(Flags::enable) {
            let (res, overflow) = self.tima.overflowing_add(1);
            self.tima = res;
            self.tma_overflow_delay = overflow;
        }
    }

    fn tima_clock(&self) -> usize {
        match self.tac.bits() & 0b11 {
            0b00 => 256,
            0b01 => 4,
            0b10 => 16,
            0b11 => 64,
            _ => unreachable!()
        }
    }

    // fn div_bit(&self) -> bool {
    //     let bit = match self.tac.bits() & 0b11 {
    //         0b00 => 9,
    //         0b01 => 3,
    //         0b10 => 5,
    //         0b11 => 7,
    //         _ => unreachable!()
    //     };

    //     (self.last_div >> bit) & 1 == 1 && (self.div >> bit) & 1 == 0
    // }

    // fn tac_enabled(&self) -> bool {
    //     self.tac.contains(Flags::enable) && self.div_bit()
    // }

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
            0xFF04 => self.div = 0,
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            0xFF07 => self.tac = Flags::from_bits_retain(val & 0b111),
            _ => {}
        }
    }
}