pub fn get_mapper(code: u8) -> Box<dyn Mapper> {
  match code {
    0x00 => Box::new(NoMbc),
    0x01 | 0x02 | 0x03 => Box::new(Mbc1::default()),
    _ => panic!("Mapper {code} not implemented")
  }
}

pub trait Mapper {
  fn read_rom(&self, rom: &[u8], addr: u16) -> u8 { rom[addr as usize] }
  fn write_rom(&mut self, rom: &mut [u8], addr: u16, val: u8) { rom[addr as usize] = val; }

  fn rom_bank_size(&self) -> usize { 16*1024 }
  fn rom_banks(&self, rom: &[u8]) -> usize { rom.len() / self.rom_bank_size() }

  fn ram_bank_size(&self) -> usize { 8*1024 }
  fn ram_banks(&self, ram: &[u8]) -> usize { ram.len() / self.ram_bank_size() }

  fn read_ram(&self, ram: &[u8], addr: u16) -> u8 { ram[addr as usize] }
  fn write_ram(&self, ram: &mut [u8], addr: u16, val: u8) { ram[addr as usize] = val; }
}

struct NoMbc;
impl Mapper for NoMbc {}


#[derive(Default)]
struct Mbc1 {
  rom_bank: usize,
  ram_enabled: bool,
  ram_bank: usize,
  extended_mode: bool,
}
impl Mapper for Mbc1 {
  fn read_rom(&self, rom: &[u8], addr: u16) -> u8 {
    let bank = match addr {
      0x0000..=0x3FFF => {
        (if self.extended_mode && rom.len() > 512*1024 {
           self.ram_bank << 5 
        } else { 0 }) % self.rom_bank_size()
      }
      0x4000..=0x7FFF => {
        let mut bank = (self.ram_bank << 5 | self.rom_bank)
          % self.rom_bank_size();
        if self.rom_bank == 0 { bank = 1; }
        bank
      }
      _ => unreachable!()
    };

    let addr = bank*self.rom_bank_size() + addr as usize%self.rom_bank_size();
    rom[addr]
  }

  fn read_ram(&self, ram: &[u8], addr: u16) -> u8 {
    if !self.ram_enabled { 0xFF }
    else {
      let bank = if self.extended_mode { 
        self.ram_bank
      } else { 0 } % self.ram_bank_size();
      let addr = bank*self.ram_bank_size() + addr as usize%self.ram_bank_size();
      ram[addr]
    }
  }

  fn write_ram(&self, ram: &mut [u8], addr: u16, val: u8) {
      if !self.ram_enabled { return; }
      
      let bank = if self.extended_mode { 
        self.ram_bank
      } else { 0 } % self.ram_bank_size();
      let addr = bank*self.ram_bank_size() + addr as usize%self.ram_bank_size();
      ram[addr] = val;
  }

  fn write_rom(&mut self, _: &mut [u8], addr: u16, val: u8) {
    match addr {
      0x0000..=0x1FFF => self.ram_enabled = val & 0b1111 == 0x0A,
      0x2000..=0x3FFF => self.rom_bank = val as usize & 0b1_1111,
      0x4000..=0x5FFF => self.ram_bank = val as usize & 0b11,
      0x6000..=0x7FFF => self.extended_mode = val & 1 != 0,
      _ => unreachable!()
    }
  }
}