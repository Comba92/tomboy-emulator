use std::ops::{Neg, Not};

use bitfield_struct::bitfield;
use bitflags::bitflags;

use crate::instr::{InstrTarget, Instruction, TargetKind, INSTRUCTIONS};

bitflags! {
	#[derive(Default, Debug)]
	pub struct Flags: u8 {
		const z = 0b1000_0000;
		const n = 0b0100_0000;
		const h = 0b0010_0000;
		const c = 0b0001_0000;
		const unused = 0b0000_1111;
	}
}

#[bitfield(u16)]
pub struct Register16 {
	#[bits(8)]
	pub lo: u8,
	#[bits(8)]
	pub hi: u8,
}

pub struct Cpu {
	pub a: u8,
	pub f: Flags,
	pub bc: Register16,
	pub de: Register16,
	pub hl: Register16,
	pub sp: u16,
	pub pc: u16,
	pub ime: bool,
	ime_to_set: bool,
	cycles: usize,
	pub mem: [u8; 0x10000],
}
impl Cpu {
	pub fn new() -> Self {
		Self {
			mem: [0; 0x10000],
			a: 1,
			f: Flags::from_bits_retain(0xB0),
			bc: Register16::from_bits(0x13),
			de: Register16::from_bits(0xD8),
			hl: Register16::from_bits(0x14D),
			sp: 0xFFFE,
			pc: 0x0100,
			ime: false,
			ime_to_set: false,
			cycles: 0,
		}
	}

	fn af(&self) -> u16 {
		((self.a as u16) << 8) | self.f.bits() as u16
	}

	fn set_af(&mut self, val: u16) {
		self.a = (val >> 8) as u8;
		self.f = Flags::from_bits_retain(val as u8 & 0xFF)
	}

	fn set_carry(&mut self, val: u16) {
		self.f.set(Flags::c, val > u8::MAX as u16);
	}

	fn set_carry16(&mut self, val: u32) {
		self.f.set(Flags::c, val > u16::MAX as u32);
	}

	fn set_hcarry(&mut self, a: u8, b: u8) {
		// TODO: obviously not working
		let res = ((a & 0xF) + (b & 0xF)) & 0x10 != 0;
		self.f.set(Flags::h, res);
	}

	fn set_hcarry16(&mut self, a: u16, b: u16) {
		let res = ((a & 0xFFF) + (b & 0xFFF)) & 0x1000 != 0;
		self.f.set(Flags::h, res);
	}

	fn set_z(&mut self, val: u16) {
		self.f.set(Flags::z, val == 0);
	}

	pub fn read(&mut self, addr: u16) -> u8 {
		let res = self.mem[addr as usize];
		self.tick();
		res
	}
	fn read16(&mut self, addr: u16) -> u16 {
		u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
	}
	fn write(&mut self, addr: u16, val: u8) { 
		self.mem[addr as usize] = val;
		self.tick();
	}
	fn write16(&mut self, addr: u16, val: u16){
		let [lo, hi] = val.to_le_bytes();
		self.write(addr as u16, lo);
		self.write(addr.wrapping_add(1) as u16, hi);
	}
	fn fetch_pc(&mut self) -> u8 {
		let res = self.read(self.pc);
		self.pc = self.pc.wrapping_add(1);
		res
	}
	fn stack_push(&mut self, val: u16) {
		self.sp = self.sp.wrapping_sub(1);
		self.tick();
		self.write16(self.sp, val);
	}
	fn stack_pop(&mut self) -> u16 {
		self.read16(self.sp)
	}
	fn tick(&mut self) {
		self.cycles += 1;
	}

	pub fn step(&mut self) {
		if self.ime_to_set {
			self.ime = true;
			self.ime_to_set = false;
		} else if self.ime {
			// handle interrupts
		}

		let opcode = self.fetch_pc();
		
		if opcode == 0xCB {
			let opcode = self.fetch_pc();
			let instr = &INSTRUCTIONS[256 + opcode as usize];
			self.execute_prefix(instr);
		} else { 
			let instr = &INSTRUCTIONS[opcode as usize];
			self.execute_no_prefix(instr)
		}
	}

	fn get_operand(&mut self, target: &InstrTarget) -> u8 {
		match (&target.kind, target.immediate) {
			(TargetKind::Immediate8 | TargetKind::Signed8, _) |
			(TargetKind::Address8, false) => self.fetch_pc(),
			(TargetKind::Address8, true) => {
				let addr = u16::from_be_bytes([
					0xFF, self.fetch_pc()
				]);
				self.read(addr)
			}
			(TargetKind::Address16, false) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(), self.fetch_pc()
				]);
				self.read(addr)
			}
			(TargetKind::A, _) => self.a,
			(TargetKind::B, _) => self.bc.hi(),
			(TargetKind::C, true)  => self.bc.lo(),
			(TargetKind::C, false) => {
				let addr = u16::from_be_bytes([
					0xFF, self.bc.lo()
				]);
				self.read(addr)
			},
			(TargetKind::D, _) => self.de.hi(),
			(TargetKind::E, _) => self.de.lo(),
			(TargetKind::F, _) => self.f.bits(),
			(TargetKind::H, _) => self.hl.hi(),
			(TargetKind::L, _) => self.hl.lo(),
			(TargetKind::BC, false) => self.read(self.bc.0),
			(TargetKind::DE, false) => self.read(self.de.0),
			(TargetKind::HL, false) => {
				let res = self.read(self.hl.0);
				if target.increment { self.hl.0 = self.hl.0.wrapping_add(1); }
				else if target.decrement { self.hl.0 = self.hl.0.wrapping_sub(1); }
				res
			}
			_ => todo!("{:?}", target.kind),
		}
	}

	fn get_operand16(&mut self, target: &InstrTarget) -> u16 {
		match (&target.kind, target.immediate) {
			(TargetKind::Address8, false) => {
				let addr = u16::from_be_bytes([
					0xFF, self.fetch_pc()
				]);
				self.read16(addr)
			}
			(TargetKind::Immediate16, _) | (TargetKind::Address16, true) => {
				u16::from_le_bytes([
					self.fetch_pc(), self.fetch_pc(),
				])
			}
			(TargetKind::Address16, false) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(), self.fetch_pc()
				]);
				self.read16(addr)
			}
			(TargetKind::SP, _) => self.sp,
			(TargetKind::AF, _) => self.af(),
			(TargetKind::BC, true)  => self.bc.0,
			(TargetKind::BC, false) => self.read16(self.bc.0),
			(TargetKind::DE, true)  => self.de.0,
			(TargetKind::DE, false) => self.read16(self.de.0),
			(TargetKind::HL, true)  => self.hl.0,
			(TargetKind::HL, false) => {
				let res = self.read16(self.hl.0);
				if target.increment { self.hl.0 = self.hl.0.wrapping_add(1); }
				else if target.decrement { self.hl.0 = self.hl.0.wrapping_sub(1); }
				res
			}
			_ => todo!("{:?}", target.kind),
		}
	}

	fn set_result(&mut self, target: &InstrTarget, val: u8) {
		match (&target.kind, target.immediate) {
		(TargetKind::Address8, false) => {
			let addr = u16::from_le_bytes([
				0xFF, self.fetch_pc()
			]);
			self.write(addr, val)
		}
		(TargetKind::Address8, true) => {
			let addr = u16::from_le_bytes([
				0xFF, self.fetch_pc()
			]);
			self.write(addr, val)
		}
		(TargetKind::Address16, true) => {
			let addr = u16::from_le_bytes([
				self.fetch_pc(), self.fetch_pc()
			]);
			self.write(addr, val);
		}
		(TargetKind::Address16, false) => {
			let addr = u16::from_le_bytes([
				self.fetch_pc(), self.fetch_pc()
			]);
			let lookup = self.read16(addr);
			self.write(lookup, val)
		}
		(TargetKind::A, _) => self.a = val as u8,
		(TargetKind::B, _) => self.bc.set_hi(val as u8),
		(TargetKind::C, true) => self.bc.set_lo(val as u8),
		(TargetKind::C, false) => {
			let addr = u16::from_be_bytes([
				0xFF, self.bc.lo() 
			]);
			self.write(addr, val);
		},
		(TargetKind::D, _) => self.de.set_hi(val as u8),
		(TargetKind::E, _) => self.de.set_lo(val as u8),
		(TargetKind::F, _) => self.f = Flags::from_bits_retain(val as u8),
		(TargetKind::H, _) => self.hl.set_hi(val as u8),
		(TargetKind::L, _) => self.hl.set_lo(val as u8),
		(TargetKind::BC, false) => {
			self.write(self.bc.0, val);
			if target.increment { self.bc.0 = self.bc.0.wrapping_add(1); }
			else if target.decrement { self.bc.0 = self.bc.0.wrapping_sub(1); }
		}
		(TargetKind::DE, false) => {
			self.write(self.de.0, val);
			if target.increment { self.de.0 = self.de.0.wrapping_add(1); }
			else if target.decrement { self.de.0 = self.de.0.wrapping_sub(1); }
		}
		(TargetKind::HL, false) => {
			self.write(self.hl.0, val);
			if target.increment { self.hl.0 = self.hl.0.wrapping_add(1); }
			else if target.decrement { self.hl.0 = self.hl.0.wrapping_sub(1); }
		}
		_ => todo!("{:?}", target.kind),
		}
	}

	fn set_result16(&mut self, target: &InstrTarget, val: u16) {
		match (&target.kind, target.immediate) {
			(TargetKind::Address8, false) => {
				let addr = u16::from_le_bytes([
					0xFF, self.fetch_pc()
				]);
				self.write16(addr, val)
			}
			(TargetKind::Address16, true) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(), self.fetch_pc()
				]);
				self.write16(addr, val);
			}
			(TargetKind::Address16, false) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(), self.fetch_pc()
				]);
				let lookup = self.read16(addr);
				self.write16(lookup, val)
			}
			(TargetKind::SP, _) => {
				self.sp = val;
			}
			(TargetKind::AF, _) => self.set_af(val),
			(TargetKind::BC, true) => self.bc.0 = val,
			(TargetKind::BC, false) => self.write16(self.bc.0, val),
			(TargetKind::DE, true) => self.de.0 = val,
			(TargetKind::DE, false) => self.write16(self.de.0, val),
			(TargetKind::HL, true)  => self.hl.0 = val,
			_ => todo!("{:?}", target.kind),
		}
	}

	fn get_cond(&self, flag: &InstrTarget) -> bool {
		match flag.kind {
			TargetKind::Z => self.f.contains(Flags::z),
			TargetKind::N => self.f.contains(Flags::n),
			TargetKind::C => self.f.contains(Flags::c),
			TargetKind::H => self.f.contains(Flags::h),

			TargetKind::NZ => !self.f.contains(Flags::z),
			TargetKind::NC => !self.f.contains(Flags::c),
			TargetKind::NH => !self.f.contains(Flags::h),

			_ => todo!("{:?}", flag.kind),
		}
	}

	fn get_interrupt_addr(&self, int: &InstrTarget) -> u16 {
		match int.kind {
			TargetKind::RST00 => 0x00,
			TargetKind::RST08 => 0x08,
			TargetKind::RST10 => 0x10,
			TargetKind::RST18 => 0x18,
			TargetKind::RST20 => 0x20,
			TargetKind::RST28 => 0x28,
			TargetKind::RST30 => 0x30,
			TargetKind::RST38 => 0x38,
			_ => unreachable!()
		}
	}

	fn get_bit_op(&self, bit: &InstrTarget) -> u8 {
		match bit.kind {
			TargetKind::Bit0 => 0,
			TargetKind::Bit1 => 1,
			TargetKind::Bit2 => 2,
			TargetKind::Bit3 => 3,
			TargetKind::Bit4 => 4,
			TargetKind::Bit5 => 5,
			TargetKind::Bit6 => 6,
			TargetKind::Bit7 => 7,
			_ => unreachable!()
		}
	}
}



impl Cpu {
	fn nop(&mut self) { todo!() }

	fn ld(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[1]);
		self.set_result(&op[0], val);
		self.tick();
	}

	fn ld16(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand16(&op[1]);
		self.set_result16(&op[0], val);
	}

	// 0xf9
	fn ldsp(&mut self) {
		self.sp = self.hl.0;
		self.tick();
	}

	fn push(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand16(&op[0]);
		self.stack_push(val);
	}

	fn pop(&mut self, op: &[InstrTarget]) {
		let val = self.stack_pop();
		self.set_result16(&op[0], val);
	}

	// 0xf8
	fn ldrel(&mut self, op: &[InstrTarget]) {
		let offset = self.get_operand(&op[1]) as i8;
		let (res, carry) = self.sp.overflowing_add_signed(offset as i16);
		
		self.f.remove(Flags::z);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, carry);
		self.set_hcarry16(self.sp, offset as u16);
		
		self.set_result16(&op[0], res);
		self.tick();
	}

	fn add(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[0]);
		let res = self.a as u16 + val as u16;
		
		self.set_z(res);
		self.f.remove(Flags::n);
		self.set_carry(res);
		self.set_hcarry(self.a, val);

		self.a = res as u8;
	}

	fn adc(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[0]);
		let res = self.a as u16 + val as u16 + self.f.contains(Flags::c) as u16; 
		
		self.set_z(res);
		self.f.remove(Flags::n);
		self.set_carry(res);
		self.set_hcarry(self.a, val);
		
		self.a = res as u8;
	}

	fn sub(&mut self, op: &[InstrTarget]) {
		todo!()
	}

	fn sbc(&mut self, op: &[InstrTarget]) {
		todo!()
	}

	fn cp(&mut self, op: &[InstrTarget]) {
		todo!()
	}

	fn inc(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[0]);
		let res = val.wrapping_add(1);
		
		self.set_z(res as u16);
		self.f.remove(Flags::n);
		self.set_hcarry(val, 1);

		self.set_result(&op[0], res);
	}

	fn inc16(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand16(&op[0]);
		let res = val.wrapping_add(1);
		self.set_result16(&op[0], res);
	}

	fn dec(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[0]);
		let res = val.wrapping_sub(1);
		
		self.f.set(Flags::z, res == 0);
		self.f.insert(Flags::n);
		self.set_hcarry(val, !1 + 1);

		self.set_result(&op[0], res);
	}

	fn dec16(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand16(&op[0]);
		let res = val.wrapping_sub(1);
		self.set_result16(&op[0], res);
	}

	fn and(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[0]);
		
		self.set_z(val as u16);
		self.f.remove(Flags::n);
		self.f.insert(Flags::h);
		self.f.remove(Flags::c);

		self.a &= val;
	}

	fn or(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[0]);
		
		self.set_z(val as u16);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);

		self.a |= val;
	}

	fn xor(&mut self, op: &[InstrTarget]) {
		let val = self.get_operand(&op[0]);
		
		self.set_z(val as u16);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);
		
		self.a ^= val;
	}

	fn ccf(&mut self) {
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.toggle(Flags::c);
	}

	fn scf(&mut self) {
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.insert(Flags::c);
	}

	fn daa(&mut self, op: &[InstrTarget]) { todo!() }

	fn cpl(&mut self) {
		self.a = self.a.not();
		self.f.insert(Flags::n);
		self.f.insert(Flags::h);
	}

	fn addhl(&mut self, op: &[InstrTarget]) { todo!() }
	fn addsp(&mut self, op: &[InstrTarget]) { todo!() }
	fn rlca(&mut self, op: &[InstrTarget]) { todo!() }
	fn rrca(&mut self, op: &[InstrTarget]) { todo!() }
	fn rla(&mut self, op: &[InstrTarget]) { todo!() }
	fn rra(&mut self, op: &[InstrTarget]) { todo!() }
	fn rlc(&mut self, op: &[InstrTarget]) { todo!() }
	fn rrc(&mut self, op: &[InstrTarget]) { todo!() }
	fn rl(&mut self, op: &[InstrTarget]) { todo!() }
	fn rr(&mut self, op: &[InstrTarget]) { todo!() }
	fn sla(&mut self, op: &[InstrTarget]) { todo!() }
	fn sra(&mut self, op: &[InstrTarget]) { todo!() }
	fn swap(&mut self, op: &[InstrTarget]) { todo!() }
	fn srl(&mut self, op: &[InstrTarget]) { todo!() }

	fn bit(&mut self, op: &[InstrTarget]) { todo!() }
	fn res(&mut self, op: &[InstrTarget]) { todo!() }
	fn set(&mut self, op: &[InstrTarget]) { todo!() }

	fn jp(&mut self, op: &[InstrTarget]) {
		let addr = self.get_operand16(&op[0]);
		self.pc = addr;
		self.tick();
	}

	// 0xe9
	fn jphl(&mut self) {
		self.pc = self.hl.0;
	}

	fn jpc(&mut self, op: &[InstrTarget]) {
		let addr = self.get_operand16(&op[0]);
		if self.get_cond(&op[0]) {
			self.pc = addr;
			self.tick();
		}
	}

	fn jr(&mut self, op: &[InstrTarget]) {
		let offset = self.get_operand(&op[0]) as i8;
		self.pc = self.pc.wrapping_add_signed(offset as i16);
		self.tick();
	}

	fn jrc(&mut self, op: &[InstrTarget]) {
		let offset = self.get_operand(&op[1]) as i8;
		if self.get_cond(&op[0]) {
			self.pc = self.pc.wrapping_add_signed(offset as i16);
			self.tick();
		}
	}

	fn call(&mut self, op: &[InstrTarget]) {
		let addr = self.get_operand16(&op[0]);
		self.stack_push(self.pc);
		self.pc = addr;
		self.tick();
	}

	fn callc(&mut self, op: &[InstrTarget]) {
		let addr = self.get_operand16(&op[1]);
		if self.get_cond(&op[0]) {
			self.stack_push(self.pc);
			self.pc = addr;
			self.tick();
		}
	}

	fn ret(&mut self) {
		self.pc = self.stack_pop();
		self.tick();
	}

	fn retc(&mut self, op: &[InstrTarget]) {
		self.tick();

		if self.get_cond(&op[0]) {
			self.pc = self.stack_pop();
			self.tick();
		}
	}

	fn reti(&mut self) {
		self.pc = self.stack_pop();
		self.ime = true;
		self.tick();
	}

	fn rst(&mut self, op: &[InstrTarget]) {
		let addr = self.get_interrupt_addr(&op[0]);
		self.stack_push(self.pc);
		self.pc = addr;
		self.tick();
	}

	fn di(&mut self) { self.ime = false; self.ime_to_set = false; }
	fn ei(&mut self) { self.ime_to_set = true; }

	fn stop(&mut self, op: &[InstrTarget]) { todo!() }
	fn halt(&mut self, op: &[InstrTarget]) { todo!() }
}


impl Cpu {
  fn execute_no_prefix(&mut self, instr: &Instruction) {
    let ops = &instr.operands;
	match instr.opcode {
      0x0 => self.nop(),
      0x02 | 0x06 | 0x08 | 0x0a | 0x0e | 0x12 | 0x16 | 0x1a | 0x1e |
	  0x22 | 0x26 | 0x2a | 0x2e | 0x32 | 0x36 | 0x3a | 0x3e | 0x40 | 0x41 | 0x42 | 
      0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x48 | 0x49 | 0x4a | 0x4b | 0x4c | 0x4d | 0x4e |
      0x4f | 0x50 | 0x51 | 0x52 | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 | 0x58 | 0x59 | 0x5a |
      0x5b | 0x5c | 0x5d | 0x5e | 0x5f | 0x60 | 0x61 | 0x62 | 0x63 | 0x64 | 0x65 | 0x66 |
      0x67 | 0x68 | 0x69 | 0x6a | 0x6b | 0x6c | 0x6d | 0x6e | 0x6f | 0x70 | 0x71 | 0x72 |
      0x73 | 0x74 | 0x75 | 0x77 | 0x78 | 0x79 | 0x7a | 0x7b | 0x7c | 0x7d | 0x7e | 0x7f |
      0xe2 | 0xea | 0xf2 | 0xfa => self.ld(ops),
	  0x01 | 0x11 | 0x21 | 0x31 => self.ld16(ops),
	  0xf8 => self.ldrel(ops),
	  0xf9 => self.ldsp(),
      0x3 | 0x4 | 0xc | 0x13 | 0x14 | 0x1c | 0x23 | 0x24 |
	  0x2c | 0x33 | 0x34 | 0x3c => self.inc(ops),
      0x5 | 0xb | 0xd | 0x15 | 0x1b | 0x1d | 0x25 | 0x2b |
	  0x2d | 0x35 | 0x3b | 0x3d => self.dec(ops),
      0x7 => self.rlca(ops),
      0x9 | 0x19 | 0x29 | 0x39 | 0x80 | 0x81 | 0x82 | 0x83 |
      0x84 | 0x85 | 0x86 | 0x87 | 0xc6 | 0xe8 => self.add(ops),
      0xf => self.rrca(ops),
      0x10 => self.stop(ops),
      0x17 => self.rla(ops),
	  0x18 => self.jr(ops),
      0x20 | 0x28 | 0x30 | 0x38 => self.jrc(ops),
      0x1f => self.rra(ops),
      0x27 => self.daa(ops),
      0x2f => self.cpl(),
      0x37 => self.scf(),
      0x3f => self.ccf(),
      0x76 => self.halt(ops),
      0x88 | 0x89 | 0x8a | 0x8b | 0x8c | 0x8d | 0x8e | 0x8f | 0xce => self.adc(ops),
      0x90 | 0x91 | 0x92 | 0x93 | 0x94 | 0x95 | 0x96 | 0x97 | 0xd6 => self.sub(ops),
      0x98 | 0x99 | 0x9a | 0x9b | 0x9c | 0x9d | 0x9e | 0x9f | 0xde => self.sbc(ops),
      0xa0 | 0xa1 | 0xa2 | 0xa3 | 0xa4 | 0xa5 | 0xa6 | 0xa7 | 0xe6 => self.and(ops),
      0xa8 | 0xa9 | 0xaa | 0xab | 0xac | 0xad | 0xae | 0xaf | 0xee => self.xor(ops),
      0xb0 | 0xb1 | 0xb2 | 0xb3 | 0xb4 | 0xb5 | 0xb6 | 0xb7 | 0xf6 => self.or(ops),
      0xb8 | 0xb9 | 0xba | 0xbb | 0xbc | 0xbd | 0xbe | 0xbf | 0xfe => self.cp(ops),
      0xc9 => self.ret(),
	  0xc0 | 0xc8 | 0xd0 | 0xd8 => self.retc(ops),
	  0xd9 => self.reti(),
      0xc1 | 0xd1 | 0xe1 | 0xf1 => self.pop(ops),
      0xc3 => self.jp(ops),
	  0xc2 | 0xd2 | 0xca | 0xda => self.jpc(ops),
	  0xe9 => self.jphl(),
	  0xcd => self.call(ops),
      0xc4 | 0xcc | 0xd4 | 0xdc => self.callc(ops),
      0xc5 | 0xd5 | 0xe5 | 0xf5 => self.push(ops),
      0xc7 | 0xcf | 0xd7 | 0xdf | 0xe7 | 0xef | 0xf7 | 0xff => self.rst(ops),
	  0xf3 => self.di(),
	  0xfb => self.ei(),
	  _ => todo!("{}: {} not reachable", instr.opcode, instr.name)
    }
  }

	fn execute_prefix(&mut self, instr: &Instruction) {
		let ops = &instr.operands;
		match instr.opcode {
			0x08 ..= 0x0f => self.rrc(ops),
			0x10 ..= 0x17 => self.rl(ops),
			0x18 ..= 0x1f => self.rr(ops),
			0x20 ..= 0x27 => self.sla(ops),
			0x28 ..= 0x2f => self.sra(ops),
			0x30 ..= 0x37 => self.swap(ops),
			0x38 ..= 0x3f => self.srl(ops),
			0x40 ..= 0x7f => self.bit(ops),
			0x80 ..= 0xbf => self.res(ops),
			0xc0 ..= 0xff => self.set(ops),
			_ => todo!("{}: {} not reachable", instr.opcode, instr.name)
		}
	}
}