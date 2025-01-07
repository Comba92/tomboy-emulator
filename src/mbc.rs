use std::{u8, usize};

use serde_json::Map;

use crate::{cart::CartHeader, nth_bit};

pub fn get_mbc(header: &CartHeader) -> Result<Box<dyn Mapper>, String> {
  let code = header.mapper_code;
  let mbc: Box<dyn Mapper> = match code {
    0x00 | 0x08 | 0x09 => NoMbc::new(header),
    0x01 | 0x02 | 0x03 => Mbc1::new(header),
    0x05 | 0x06 => Mbc2::new(header),
    0x0F ..= 0x13 => Mbc3::new(header),
    0x19 ..= 0x1E => Mbc5::new(header),
    _ => return Err(format!("Mapper {code} not implemented")),
  };

  Ok(mbc)
}

pub struct Cart {
  pub header: CartHeader,
  rom: Vec<u8>,
  exram: Vec<u8>,
  mbc: Box<dyn Mapper>,
}

impl Cart {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    let header = CartHeader::new(rom)?;
    println!("Loaded Gameboy ROM: {:#?}", header);

    let mbc = get_mbc(&header)?;
    let exram = vec![0xFF; header.ram_size];
    let rom = Vec::from(rom);

    Ok(Self { header, rom, exram, mbc })
  }

  pub fn rom_read(&mut self, addr: u16) -> u8 {
    self.rom[self.mbc.rom_addr(addr)]
  }
  pub fn rom_write(&mut self, addr: u16, val: u8) {
    self.mbc.rom_write(addr, val);
  }

  pub fn ram_read(&mut self, addr: u16) -> u8 {
    self.mbc.ram_read(&self.exram, addr)
  }
  pub fn ram_write(&mut self, addr: u16, val: u8) {
    self.mbc.ram_write(&mut self.exram, addr, val);
  }
}

pub trait Mapper {
  fn new(header: &CartHeader) -> Box<Self> where Self: Sized;

  fn rom_addr(&mut self, addr: u16) -> usize;
  fn ram_addr(&mut self, addr: u16) -> (bool, usize);

  fn ram_read(&mut self, exram: &[u8], addr: u16) -> u8 {
    let (enabled, addr) = self.ram_addr(addr);
    if enabled { exram[addr] } else { 0xFF }
  }
  fn ram_write(&mut self, exram: &mut[u8], addr: u16, val: u8) {
    let (enabled, addr) = self.ram_addr(addr);
    if enabled { exram[addr] = val; }
  }

  fn rom_write(&mut self, addr: u16, val: u8);

  fn tick(&mut self) {}
}

struct NoMbc;
impl Mapper for NoMbc {
  fn new(_: &CartHeader) -> Box<Self> { Box::new(NoMbc) }
  fn rom_write(&mut self, _: u16, _: u8) {}
  
  fn rom_addr(&mut self, addr: u16) -> usize { addr as usize }
  fn ram_addr(&mut self, addr: u16) -> (bool, usize) { (true, addr as usize) }
}

#[derive(Debug)]
struct Banking {
  #[allow(unused)]
  data_size: usize,
  bank_size: usize,
  banks_count: usize,
  banks: Box<[usize]>,
}
impl Banking {
  pub fn new(data_size: usize, pages_count: usize, bank_size: usize) -> Self {
    let banks = vec![0; pages_count].into_boxed_slice();
    let banks_count = data_size / bank_size;
    Self {data_size, bank_size, banks_count, banks}
  }

  pub fn new_rom(header: &CartHeader, pages_count: usize) -> Self {
    Self::new(header.rom_size, pages_count, 16 * 1024)
  }

  pub fn new_ram(header: &CartHeader) -> Self {
    Self::new(header.ram_size, 1, 8 * 1024)
  }

  pub fn set(&mut self, page: usize, bank: usize) {
    let pages_count = self.banks.len();
    self.banks[page % pages_count] = (bank % self.banks_count) * self.bank_size;
  }

  fn addr(&self, addr: usize) -> usize {
    let page = addr / self.bank_size;
    let pages_count = self.banks.len();
    self.banks[page % pages_count] + (addr % self.bank_size)
  }
}

// TODO: MBC1M
struct Mbc1 {
  rom_banks: Banking,
  ram_banks: Banking,
  rom_select: usize,
  ram_select: usize,
  ram_enabled: bool,
  extended_mode: bool,
}

impl Mbc1 {
  fn update_banks(&mut self) {
    let ext_rom_bank = self.ram_select << 5;

    self.rom_banks.set(0, if self.extended_mode { ext_rom_bank as usize } else { 0 });
    self.rom_banks.set(1,
      (ext_rom_bank + self.rom_select) as usize
    );
    self.ram_banks.set(0, if self.extended_mode { self.ram_select as usize } else { 0 });
  }
}

impl Mapper for Mbc1 {
    fn new(header: &CartHeader) -> Box<Self> where Self: Sized {
      let mut rom_banks = Banking::new_rom(header, 2);
      let ram_banks = Banking::new_ram(header);

      // Page 1 starts at 1
      rom_banks.set(1, 1);

      Box::new(Self{
        rom_banks, ram_banks, 
        ram_enabled: false, extended_mode: false,
        // rom_selects always default as 1
        rom_select: 1, ram_select: 0,
      })
    }

    fn rom_addr(&mut self, addr: u16) -> usize {
      self.rom_banks.addr(addr as usize)
    }

    fn ram_addr(&mut self, addr: u16) -> (bool, usize) {
      (self.ram_enabled, self.ram_banks.addr(addr as usize))
    }

    fn rom_write(&mut self, addr: u16, val: u8) {
      match addr {
        0x0000..=0x1FFF => self.ram_enabled = val & 0b1111 == 0x0A,
        0x2000..=0x3FFF => {
          self.rom_select = (val as usize & 0b1_1111).clamp(1, u8::MAX as usize);
          self.update_banks();
        }
        0x4000..=0x5FFF => {
          self.ram_select = val as usize & 0b11;
          self.update_banks();
        }
        0x6000..=0x7FFF => {
          self.extended_mode = nth_bit(val, 0);
          self.update_banks();
        }
        _ => {}
      }
    }
}

struct Mbc2 {
  rom_banks: Banking,
  ram_enabled: bool,
}
impl Mapper for Mbc2 {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut rom_banks = Banking::new_rom(header, 2);
    rom_banks.set(1, 1);
    Box::new(Self {rom_banks,ram_enabled: false})
  }

  fn rom_addr(&mut self, addr: u16) -> usize {
    self.rom_banks.addr(addr as usize)
  }

  fn rom_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x0000..=0x3FFF => {
        match (addr >> 8) & 1 != 0 {
          false => self.ram_enabled = val == 0x0A,
          true  => {
            let bank = (val & 0b1111)
              .clamp(1, u8::MAX) as usize;
            self.rom_banks.set(1, bank);
          }
        }
      }
      _ => {}
    }
  }

  fn ram_addr(&mut self, addr: u16) -> (bool, usize) {
    (self.ram_enabled, (addr) as usize % 512)
  }

  fn ram_read(&mut self, exram: &[u8], addr: u16) -> u8 {
    let (enabled, addr) = self.ram_addr(addr);
    if enabled { exram[addr] | 0xF0 } else { 0xFF }
  }

  fn ram_write(&mut self, exram: &mut[u8], addr: u16, val: u8) {
    let (enabled, addr) = self.ram_addr(addr);
    if enabled { exram[addr] = val | 0xF0; }
  }
}



struct Mbc3 {
  rom_banks: Banking,
  ram_banks: Banking,
  ram_enabled: bool,
  
  rtc_select: u8,
  rtc_seconds: u8,
  rtc_minutes: u8,
  rtc_hours: u8,
  rtc_day: u16,
  rtc_carry: bool,
  rtc_halted: bool,
}

impl Mapper for Mbc3 {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut rom_banks = Banking::new_rom(header, 2);
    let ram_banks = Banking::new_ram(header);
    rom_banks.set(1, 1);

    Box::new(Self {
      rom_banks, ram_banks, ram_enabled: false,
      rtc_select: 0,
      rtc_halted: false,
      rtc_seconds: 0,
      rtc_minutes: 0,
      rtc_hours: 0,
      rtc_day: 0,
      rtc_carry: false,
    })
  }

  fn rom_addr(&mut self, addr: u16) -> usize {
    self.rom_banks.addr(addr as usize)
  }

  fn ram_addr(&mut self, addr: u16) -> (bool, usize) {
    (self.ram_enabled, self.ram_banks.addr(addr as usize))
  }

  fn rom_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x0000..=0x1FFF => self.ram_enabled = val == 0x0A,
      0x2000..=0x3FFF => {
        let bank = (val & 0b0111_1111)
          .clamp(1, u8::MAX);
        self.rom_banks.set(1, bank as usize);
      }
      0x4000..=0x5FFF => {
        if (0x8..=0xC).contains(&val) {
          self.rtc_select = val;
        } else {
          self.ram_banks.set(0, val as usize & 0b11);
          self.rtc_select = 0;
        }
      }
      0x6000..=0x7FFF => {

      }
      _ => {}
    }
  }

  fn ram_read(&mut self, exram: &[u8], addr: u16) -> u8 {
    let (enabled, addr) = self.ram_addr(addr);
    if !enabled { return 0xFF; }

    if self.rtc_select != 0 {
      // TODO: rtc
      0xFF
    } else {
      exram[addr]
    }
  }

  fn ram_write(&mut self, exram: &mut[u8], addr: u16, val: u8) {
    let (enabled, addr) = self.ram_addr(addr);
    if !enabled { return; }

    if self.rtc_select != 0 {
      // TODO: rtc
    } else {
      exram[addr] = val;
    }
  }

  fn tick(&mut self) {
    // TODO: rtc
  }
}

struct Mbc5 {
  rom_banks: Banking,
  ram_banks: Banking,
  ram_enabled: bool,
  rom_select: usize,
}

impl Mapper for Mbc5 {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut rom_banks = Banking::new_rom(header, 2);
    let ram_banks = Banking::new_ram(header);

    rom_banks.set(1, 1);

    Box::new(Self{
      rom_banks, ram_banks,
      ram_enabled: false,
      rom_select: 1
    })
  }

  fn rom_addr(&mut self, addr: u16) -> usize {
    self.rom_banks.addr(addr as usize)
  }

  fn ram_addr(&mut self, addr: u16) -> (bool, usize) {
    (self.ram_enabled, self.ram_banks.addr(addr as usize))
  }

  fn rom_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x0000..=0x1FFF => self.ram_enabled = val == 0x0A,
      0x2000..=0x2FFF => {
        self.rom_select = (self.rom_select & 0xF0) | val as usize;
        self.rom_banks.set(1, self.rom_select);
      }
      0x3000..=0x3FFF => {
        self.rom_select = 
          (self.rom_select & 0x0F) | ((val as usize & 0b10) << 8);
        self.rom_banks.set(1, self.rom_select);
      }
      0x4000..=0x5FFF => self.ram_banks.set(0, val as usize & 0xF),
      _ => {}
    }
  }
}

struct Mbc6 {

}
impl Mapper for Mbc6 {
  fn new(header: &CartHeader) -> Box<Self> {
      todo!()
  }

  fn rom_addr(&mut self, addr: u16) -> usize {
      todo!()
  }

  fn ram_addr(&mut self, addr: u16) -> (bool, usize) {
      todo!()
  }

  fn rom_write(&mut self, addr: u16, val: u8) {
      todo!()
  }
}