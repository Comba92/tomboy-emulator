#[cfg(test)]
mod cpu_tests {
  use circular_buffer::CircularBuffer;
  use instr::INSTRUCTIONS;
  use tomboy_emulator::*;
  use cpu::Cpu;

  #[test]
  fn run_test() {
    let roms = std::fs::read_dir("./tests/roms/").unwrap();
    let logs = std::fs::read_dir("./tests/logs/").unwrap();

    let mut roms = roms.map(|e| e.unwrap().path()).collect::<Vec<_>>();
    let mut logs = logs.map(|e| e.unwrap().path()).collect::<Vec<_>>();
    roms.sort();
    logs.sort();

    let mut iter = roms.iter().zip(logs.iter()).enumerate();

    while let Some((i, (rom_path, log_path))) = iter.next() {
      if i+1 >= 10 { continue; }

      let rom = std::fs::read(rom_path).unwrap();
      let log = std::fs::read_to_string(log_path).unwrap();
      let mut log_lines = log.lines().enumerate();

      println!("Executing {rom_path:?} {log_path:?}");

      let mut cpu = Cpu::new();
      cpu.pc = if [2, 6].contains(&(i+1)) {
        0x101
      } else { 0x100 };

      cpu.bus.mem[0xFF44] = 0x90;
      let mut last_ten = CircularBuffer::<10, String>::new();

      let (left, _) = cpu.bus.mem.split_at_mut(rom.len());
      left.copy_from_slice(&rom);
      
      while let Some((line, log)) = log_lines.next() {
        let mine = log_cpu(&mut cpu);
        
        let op = cpu.peek(cpu.pc);
        
        if mine != log {
          let diff = prettydiff
          ::diff_words(&mine, log);
          
          for instr in last_ten {
            println!("{instr}");
          }
          println!("{}\nLast OP {:02X}: {:X?}", mine, op, INSTRUCTIONS[op as usize]);
          
          println!("{:0X?}", cpu);
          println!("IE {:?} IF {:?}", cpu.bus.inte, cpu.bus.intf);
          println!("{diff}\n{} lines executed", line+1);
          panic!()
        }
        
        let last= format!("{}\nLast OP {:02X}: {} {:?}\n", mine, op, INSTRUCTIONS[op as usize].name, INSTRUCTIONS[op as usize].operands);
        last_ten.push_back(last);
        cpu.step();
      }
    }
  }
  
  fn log_cpu(cpu: &mut Cpu) -> String {
    let b0 = cpu.peek(cpu.pc);
    let b1 = cpu.peek(cpu.pc+1);
    let b2 = cpu.peek(cpu.pc+2);
    let b3 = cpu.peek(cpu.pc+3);

    format!("\
      A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} \
      H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})\
    ", cpu.a, cpu.f.bits(), cpu.bc.hi(), cpu.bc.lo(), cpu.de.hi(), cpu.de.lo(),
       cpu.hl.hi(), cpu.hl.lo(), cpu.sp, cpu.pc, b0, b1, b2, b3
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
