#[cfg(test)]
mod cpu_tests {
  use tomboy_emulator::*;
  use cpu::Cpu;
  use cart::Cart;

  #[test]
  fn run_test() {
    let mut cpu = Cpu::new();

    let rom = std::fs::read("./tests/roms/01-special.gb").unwrap();
    let cart = Cart::new(&rom).unwrap();
    println!("{:?}", cart);
  
    let mut log_lines = include_str!("./Gameboy-logs/EpicLog.txt").lines();

    let (left, _) = cpu.mem.split_at_mut(rom.len());
    left.copy_from_slice(&rom);
  
    for _ in 0..20 {
      let mine = log_cpu(&mut cpu);
      let log = log_lines.next().unwrap();
      
      let diff = prettydiff::diff_words(&mine, log);
      println!("{diff}");

      cpu.step();
    }  
  
  }
  
  fn log_cpu(cpu: &mut Cpu) -> String {
    let b0 = cpu.read(cpu.pc);
    let b1 = cpu.read(cpu.pc+1);
    let b2 = cpu.read(cpu.pc+2);
    let b3 = cpu.read(cpu.pc+3);

    format!("\
      A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} \
      H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})\
    ", cpu.a, cpu.f.bits(), cpu.bc.lo(), cpu.bc.hi(), cpu.de.lo(), cpu.de.hi(),
       cpu.hl.lo(), cpu.hl.hi(), cpu.sp, cpu.pc, b0, b1, b2, b3
    )
  }

  // fn log_line(line: &str) -> String {
  //   let mut iter = line.split_whitespace();
  //   let a: u8 = iter.nth(1).unwrap().parse().unwrap();
  //   let b: u8 = iter.nth(1).unwrap().parse().unwrap();
  //   let c: u8 = iter.nth(1).unwrap().parse().unwrap();
  //   let d: u8 = iter.nth(1).unwrap().parse().unwrap();
  //   let e: u8 = iter.nth(1).unwrap().parse().unwrap();
  //   let f: u8 = iter.nth(1).unwrap().parse().unwrap();
  //   let h: u8 = iter.nth(1).unwrap().parse().unwrap();
  //   let l: u8 = iter.nth(1).unwrap().parse().unwrap();
  // }
}
