use std::usize;

use crate::{cart::CartHeader, nth_bit};

pub fn get_mbc(header: &CartHeader) -> Result<Box<dyn Mapper>, String> {
  let code = header.mapper_code;
  let mbc: Box<dyn Mapper> = match code {
    0x00 | 0x08 | 0x09 => NoMbc::new(header),
    0x01 | 0x02 | 0x03 => Mbc1::new(header),
    // 0x05 | 0x06 => Box::new(Mbc2::default()),
    // 0x0F ..= 0x13 => Box::new(Mbc3::default()),
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
    let exram = vec![0; header.ram_size];
    let rom = Vec::from(rom);

    Ok(Self { header, rom, exram, mbc })
  }

  pub fn rom_read(&mut self, addr: u16) -> u8 {
    self.rom[self.mbc.rom_addr(addr)]
  }
  pub fn rom_write(&mut self, addr: u16, val: u8) {
    self.mbc.write_rom(addr, val);
  }

  pub fn ram_read(&mut self, addr: u16) -> u8 {
    let (enabled, addr) = self.mbc.ram_addr(addr);
    if enabled { self.exram[addr] } else { 0xFF }
   }
  pub fn ram_write(&mut self, addr: u16, val: u8) {
    let (enabled, addr) = self.mbc.ram_addr(addr);
    if enabled { self.exram[addr] = val; }
  }
}

pub trait Mapper {
  fn new(header: &CartHeader) -> Box<Self> where Self: Sized;

  fn rom_addr(&mut self, addr: u16) -> usize;
  fn ram_addr(&mut self, addr: u16) -> (bool, usize);

  fn write_rom(&mut self, addr: u16, val: u8);
}

struct NoMbc;
impl Mapper for NoMbc {
  fn new(_: &CartHeader) -> Box<Self> { Box::new(NoMbc) }
  fn write_rom(&mut self, _: u16, _: u8) {}
  
  fn rom_addr(&mut self, addr: u16) -> usize { addr as usize }
  fn ram_addr(&mut self, addr: u16) -> (bool, usize) { (true, addr as usize) }
}

#[derive(Debug)]
struct Banking {
  #[allow(unused)]
  data_size: usize,
  bank_size: usize,
  banks_count: usize,
  page_start: usize,
  banks: Box<[usize]>,
}
impl Banking {
  pub fn new(data_size: usize, page_start: usize, pages_count: usize, bank_size: usize) -> Self {
    let banks_count = data_size / bank_size;
    let banks = vec![0; pages_count].into_boxed_slice();
    Self {data_size, bank_size, banks_count, page_start, banks}
  }

  pub fn new_rom(header: &CartHeader, pages_count: usize) -> Self {
    Self::new(header.rom_size, 0, pages_count, 16 * 1024)
  }

  pub fn new_ram(header: &CartHeader) -> Self {
    Self::new(header.ram_size, 0xA000, 1, 8 * 1024)
  }

  pub fn set(&mut self, page: usize, bank: usize) {
    let pages_count = self.banks.len();
    self.banks[page % pages_count] = (bank % self.banks_count) * self.bank_size;
  }

  fn addr(&self, addr: usize) -> usize {
    let addr = addr - self.page_start;
    let page = addr / self.bank_size;
    let pages_count = self.banks.len();
    self.banks[page % pages_count] + (addr % self.bank_size)
  }
}

struct Mbc1 {
  rom_banks: Banking,
  ram_banks: Banking,
  rom_select: u8,
  ram_select: u8,
  ram_enabled: bool,
  extended_mode: bool,
}

impl Mbc1 {
  fn update_banks(&mut self) {
    let ext = if self.extended_mode && self.rom_banks.data_size > 512*1024 {
      (self.ram_select as usize) << 5
    } else { 0 };

    self.rom_banks.set(0, ext);
    self.rom_banks.set(1,
      ((self.ram_select << 5) + self.rom_select) as usize
    );
    self.ram_banks.set(0, ext);
  }
}

impl Mapper for Mbc1 {
    fn new(header: &CartHeader) -> Box<Self> where Self: Sized {
      let rom_banks = Banking::new_rom(header, 2);
      let ram_banks = Banking::new_ram(header);

      println!("{rom_banks:?} {ram_banks:?}");

      Box::new(Self{
        rom_banks, ram_banks, 
        ram_enabled: false, extended_mode: false,
        rom_select: 0, ram_select: 0,
      })
    }

    fn rom_addr(&mut self, addr: u16) -> usize {
      self.rom_banks.addr(addr as usize) 
    }

    fn ram_addr(&mut self, addr: u16) -> (bool, usize) {
      (self.ram_enabled, self.ram_banks.addr(addr as usize))
    }

    fn write_rom(&mut self, addr: u16, val: u8) {
      match addr {
        0x0000..=0x1FFF => self.ram_enabled = val & 0b1111 == 0x0A,
        0x2000..=0x3FFF => {
          self.rom_select = (val & 0b1_1111).clamp(1, u8::MAX);
          self.update_banks();
        }
        0x4000..=0x5FFF => {
          self.ram_select = val & 0b11;
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


// struct Mbc1 {
//   rom_bank: usize,
//   ram_enabled: bool,
//   ram_bank: usize,
//   extended_mode: bool,
// }
// impl Default for Mbc1 {
//   fn default() -> Self {
//     Self { rom_bank: 1, ram_enabled: Default::default(), ram_bank: Default::default(), extended_mode: Default::default() }
//   }
// }

// impl Mapper for Mbc1 {
//   fn read_rom(&self, rom: &[u8], addr: u16) -> u8 {
//     let bank = match addr {
//       0x0000..=0x3FFF => {
//         if self.extended_mode && rom.len() > 512*1024 {
//           (self.ram_bank << 5) % self.rom_banks(rom)
//         } else { 0 }
//       }
//       0x4000..=0x7FFF => {
//         (self.ram_bank << 5 | self.rom_bank)
//           % self.rom_banks(rom)
//       }
//       _ => unreachable!()
//     };

//     let addr = bank*self.rom_bank_size() + addr as usize%self.rom_bank_size();
//     rom[addr]
//   }

//   fn read_ram(&self, ram: &[u8], addr: u16) -> u8 {
//     if !self.ram_enabled || ram.is_empty() { 0xFF }
//     else {
//       let bank = if self.extended_mode { 
//         self.ram_bank % self.ram_banks(ram)
//       } else { 0 };

//       let addr = bank*self.ram_bank_size() + addr as usize%self.ram_bank_size();
//       ram[addr]
//     }
//   }

//   fn write_ram(&self, ram: &mut [u8], addr: u16, val: u8) {
//       if !self.ram_enabled || ram.is_empty() { return; }
      
//       let bank = if self.extended_mode { 
//         self.ram_bank % self.ram_banks(ram)
//       } else { 0 };

//       let addr = bank*self.ram_bank_size() + addr as usize%self.ram_bank_size();
//       ram[addr] = val;
//   }

//   fn write_rom(&mut self, _: &mut [u8], addr: u16, val: u8) {
//     match addr {
//       0x0000..=0x1FFF => self.ram_enabled = val & 0b1111 == 0x0A,
//       0x2000..=0x3FFF => self.rom_bank = (val as usize & 0b1_1111).clamp(1, usize::MAX),
//       0x4000..=0x5FFF => self.ram_bank = val as usize & 0b11,
//       0x6000..=0x7FFF => self.extended_mode = nth_bit(val, 0),
//       _ => unreachable!()
//     }
//   }
// }