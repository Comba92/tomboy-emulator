pub fn get_mapper_from_header(code: u8) -> Box<dyn Mapper> {
  match code {
    0x00 => Box::new(NoMapper),
    _ => panic!("Mapper {code} not implemented")
  }
}

pub trait Mapper {
  fn read_rom(&self, rom: &[u8], addr: u16) -> u8 { rom[addr as usize] }
  fn write_rom(&self, rom: &mut [u8], addr: u16, val: u8) { rom[addr as usize] = val; }
}

struct NoMapper;
impl Mapper for NoMapper {}