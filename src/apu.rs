#[derive(Default)]
pub struct Apu {

}


impl Apu {
  pub fn tick(&mut self) {}

  pub fn read(&self, addr: u16) -> u8 {
    0xff
  }

  pub fn write(&self, addr: u16, val: u8) {

  }

  pub fn consume_samples(&mut self) -> Vec<f32> {
    Vec::new()
  }
}