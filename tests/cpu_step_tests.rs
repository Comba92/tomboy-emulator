#[cfg(test)]
mod cpu_step_tests {
    use core::fmt;
    use std::fs;

    use prettydiff::diff_words;
    use serde::Deserialize;
    use tomboy_emulator::{cpu::{self, Cpu}, instr::INSTRUCTIONS, mem::Ram64kb};

  #[derive(Deserialize, Debug, PartialEq, Eq)]
  struct CpuMock {
    a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, h: u8, l: u8,
    pc: u16, sp: u16, ram: Vec<(u16, u8)>,
  }
  
  impl fmt::Display for CpuMock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "{}", format!("{:X?}", self))
    }
  }
  impl CpuMock {
    fn from_cpu(cpu: &Cpu<Ram64kb>) -> Self {
      Self {
        pc: cpu.pc, sp: cpu.sp, 
        a: cpu.a, b: cpu.bc.hi(), c: cpu.bc.lo(), 
        d: cpu.de.hi(), e: cpu.de.lo(), f: cpu.f.bits(), 
        h: cpu.hl.hi(), l: cpu.hl.lo(),
        ram: Vec::new(),
      }
    }
  }

  fn cpu_from_mock(mock: &CpuMock) -> Cpu<Ram64kb> {
    let mut cpu = Cpu::with_ram64kb();

    cpu.a = mock.a;
    cpu.f = cpu::Flags::from_bits_retain(mock.f);
    cpu.bc.set_hi(mock.b);
    cpu.bc.set_lo(mock.c);
    cpu.de.set_hi(mock.d);
    cpu.de.set_lo(mock.e);
    cpu.hl.set_hi(mock.h);
    cpu.hl.set_lo(mock.l);
    cpu.sp = mock.sp;
    cpu.pc = mock.pc;

    for (addr, byte) in &mock.ram {
      cpu.write(*addr, *byte);
    }

    cpu.mcycles = 0;
    cpu
  }

  #[derive(Deserialize, Debug)]
  struct Test {
    name: String,
    #[serde(alias = "initial")]
    start: CpuMock,
    #[serde(alias = "final")]
    end: CpuMock,
    cycles: Vec<Option<(u16, u8, String)>>,
  }

  #[test]
  fn cpu_test_one() {
    let json = include_str!("sm83/v1/00.json");
    let test: Vec<Test> = serde_json::from_str(json).unwrap();
  
    let mut cpu = cpu_from_mock(&test[0].start);

    while cpu.mcycles < test[0].cycles.len() {
      println!("{:#X?}", cpu);
      println!("{:#X?}", &INSTRUCTIONS[cpu.peek(cpu.pc) as usize]);
      cpu.step();
    }


    let mut my_end = CpuMock::from_cpu(&cpu);
    for (addr, _) in &test[0].end.ram {
      my_end.ram.push((*addr, cpu.peek(*addr)))
    }

    assert_eq!(test[0].end, my_end, 
      "Found error {:#X?}\n{}",
      test[0].name, diff_words(&my_end.to_string(), &test[0].end.to_string()));
  }

  #[test]
fn cpu_test() {
  let mut dir = fs::read_dir("./tests/sm83/v1/")
    .expect("directory not found")
    .enumerate();

  while let Some((i, Ok(f))) = dir.next() {
    let json_test = fs::read(f.path()).expect("couldnt't read file");
    let tests: Vec<Test> = serde_json::from_slice(&json_test).expect("couldn't parse json");

    println!("Testing file {i}: {:?}", f.file_name());

    'testing: for test in tests.iter() {
      let mut cpu = cpu_from_mock(&test.start);

      while cpu.mcycles < test.cycles.len() {
        cpu.step();
      }

      let mut my_end = CpuMock::from_cpu(&cpu);
      for (addr, _) in &test.end.ram {
        my_end.ram.push((*addr, cpu.read(*addr)))
      }

      if my_end != test.end {
        // let mut builder = colog::basic_builder();
        // builder.filter_level(log::LevelFilter::Trace);
        // builder.init();
        
        let mut log_cpu = cpu_from_mock(&test.start);

        while log_cpu.mcycles < test.cycles.len() {
          println!("{:X?}", log_cpu);
          println!("{:X?}", &INSTRUCTIONS[log_cpu.peek(log_cpu.pc) as usize]);
          log_cpu.step();
        }

        println!("{:X?}", log_cpu);

        assert_eq!(my_end, test.end,
          "Found error in file {:?}, test {:?}\n{}",
          f.file_name(), test.name, 
            diff_words(&my_end.to_string(), &test.end.to_string())
        );
      }
    }
  }
}
}