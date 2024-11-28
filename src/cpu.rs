use std::{cell::OnceCell, ops::Not};

use bitfield_struct::bitfield;
use bitflags::bitflags;

use crate::instr::{TargetKind, Instruction, InstrTarget};

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
	hi: u8,
	#[bits(8)]
	lo: u8
}

pub struct Cpu {
	a: u8,
	f: Flags,
	bc: Register16,
	de: Register16,
	hl: Register16,
	sp: u16,
	pc: u16,
	ime: bool,
	cycles: usize,
	mem: [u8; 0x10000],
}
impl Cpu {
	pub fn new() -> Self {
		Self {
			mem: [0; 0x10000],
			a: 0,
			f: Flags::empty(),
			bc: Register16::new(),
			de: Register16::new(),
			hl: Register16::new(),
			sp: 0,
			pc: 0,
			ime: false,
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

	fn read(&mut self, addr: u16) -> u8 { todo!() }
	fn read16(&mut self, addr: u16) -> u16 { todo!() }
	fn write(&mut self, addr: u16, val: u8) { todo!() }
	fn write16(&mut self, addr: u16, val: u16){ todo!() }
	fn fetch_pc(&mut self) -> u8 { todo!() }
	fn stack_push(&mut self, val: u16) {
		self.sp = self.sp.wrapping_sub(1);
		self.tick();
		self.write16(self.sp, val);
	}
	fn stack_pop(&mut self) -> u16 {
		self.read16(self.sp)
	}
	fn tick(&mut self) { todo!() }

	pub fn step(&mut self) {
		// decode
		// execute
	}

	fn get_operand(&mut self, target: &InstrTarget) -> u8 {
		match (&target.kind, target.immediate) {
			(TargetKind::Immediate8 | TargetKind::Signed8, _) => self.fetch_pc(),
			(TargetKind::Immediate16, _) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(),
					self.fetch_pc(),
				]);
				self.read(addr)
			}
			(TargetKind::Address8, false) => {
				let addr = u16::from_be_bytes([
					0xFF, self.fetch_pc()
				]);
				self.read(addr)
			}
			(TargetKind::Address16, true) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(),
					self.fetch_pc()
				]);
				self.read(addr)
			}
			(TargetKind::Address16, false) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(),
					self.fetch_pc()
				]);
				let lookup = self.read16(addr);
				self.read(lookup)
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
			_ => todo!("not implemented")
		}
	}

	fn get_operand16(&mut self, target: &InstrTarget) -> u16 {
		match (&target.kind, target.immediate) {
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
			_ => todo!()
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
		(TargetKind::Address16, true) => {
			let addr = u16::from_le_bytes([
				self.fetch_pc(),
				self.fetch_pc()
			]);
			self.write(addr, val);
		}
		(TargetKind::Address16, false) => {
			let addr = u16::from_le_bytes([
				self.fetch_pc(),
				self.fetch_pc()
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
		(TargetKind::HL, false) => {
			self.write(self.hl.0, val);
			if target.increment { self.hl.0 = self.hl.0.wrapping_add(1); }
			else if target.decrement { self.hl.0 = self.hl.0.wrapping_sub(1); }
		}
		_ => {}
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
					self.fetch_pc(),
					self.fetch_pc()
				]);
				self.write16(addr, val);
			}
			(TargetKind::Address16, false) => {
				let addr = u16::from_le_bytes([
					self.fetch_pc(),
					self.fetch_pc()
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
			_ => todo!()
		}
	}
}



impl Cpu {
	fn nop(&mut self) {}

	fn ld(&mut self, src: &InstrTarget, dst: &InstrTarget) {
		let val = self.get_operand(&src);
		self.set_result(&dst, val);
		self.tick();
	}

	fn ld16(&mut self, src: &InstrTarget, dst: &InstrTarget) {
		let val = self.get_operand16(&src);
		self.set_result16(&dst, val);
	}

	fn ldsp(&mut self) {
		self.sp = self.hl.0;
		self.tick();
	}

	fn ldrel(&mut self, src: &InstrTarget, dst: &InstrTarget) {
		let offset = self.get_operand(src) as i8;
		let val = self.sp.wrapping_add_signed(offset as i16);
		self.set_result16(dst, val);

		self.f.remove(Flags::z);
		self.f.remove(Flags::n);
		// TODO: set flags h c

		self.tick();
	}

	fn push(&mut self, src: &InstrTarget) {
		let val = self.get_operand16(&src);
		self.stack_push(val);
	}

	fn pop(&mut self, dst: &InstrTarget) {
		let val = self.stack_pop();
		self.set_result16(dst, val);
	}

	fn inc(&mut self, dst: &InstrTarget) {
		let val = self.get_operand(dst).wrapping_add(1);
		self.set_result(dst, val);
		self.f.set(Flags::z, val == 0);
		self.f.remove(Flags::n);
		// TODO: set flag h
	}

	fn inc16(&mut self, dst: &InstrTarget) {
		let val = self.get_operand16(dst).wrapping_add(1);
		self.set_result16(dst, val);
		self.f.set(Flags::z, val == 0);
		self.f.remove(Flags::n);
		// TODO: set flag h
	}

	fn dec(&mut self, dst: &InstrTarget) {
		let val = self.get_operand(dst).wrapping_sub(1);
		self.set_result(dst, val);
		self.f.set(Flags::z, val == 0);
		self.f.insert(Flags::n);
		// TODO: set flag h
	}

	fn dec16(&mut self, dst: &InstrTarget) {
		let val = self.get_operand16(dst).wrapping_sub(1);
		self.set_result16(dst, val);
		self.f.set(Flags::z, val == 0);
		self.f.insert(Flags::n);
		// TODO: set flag h
	}

	fn and(&mut self, src: &InstrTarget) {
		let val = self.get_operand(src);
		self.a &= val;
		self.f.set(Flags::z, val == 0);
		self.f.remove(Flags::n);
		self.f.insert(Flags::h);
		self.f.remove(Flags::c);
	}

	fn or(&mut self, src: &InstrTarget) {
		let val = self.get_operand(src);
		self.a |= val;
		self.f.set(Flags::z, val == 0);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);
	}

	fn xor(&mut self, src: &InstrTarget) {
		let val = self.get_operand(src);
		self.a ^= val;
		self.f.set(Flags::z, val == 0);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);
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

	fn daa(&mut self) { todo!() }

	fn cpl(&mut self) {
		self.a = self.a.not();
		self.f.insert(Flags::n);
		self.f.insert(Flags::h);
	}

	fn jp(&mut self, jmp: &InstrTarget) {
		let addr = self.get_operand16(jmp);
		self.pc = addr;
		self.tick();
	}

	fn jphl(&mut self) {
		self.pc = self.hl.0;
	}

	fn jpc(&mut self, jmp: &InstrTarget, cond: bool) {
		let addr = self.get_operand16(jmp);
		if cond {
			self.pc = addr;
			self.tick();
		}
	}

	fn jr(&mut self, jmp: &InstrTarget) {
		let offset = self.get_operand(jmp) as i8;
		self.pc = self.pc.wrapping_add_signed(offset as i16);
		self.tick();
	}

	fn jrc(&mut self, jmp: &InstrTarget, cond: bool) {
		let offset = self.get_operand(jmp) as i8;
		if cond {
			self.pc = self.pc.wrapping_add_signed(offset as i16);
			self.tick();
		}
	}

	fn call(&mut self, jmp: &InstrTarget) {
		let addr = self.get_operand16(jmp);
		self.stack_push(self.pc);
		self.pc = addr;
	}

	fn callc(&mut self, jmp: &InstrTarget, cond: bool) {
		let addr = self.get_operand16(jmp);
		if cond {
			self.stack_push(self.pc);
			self.pc = addr;
		}
	}

	fn ret(&mut self) {
		self.pc = self.stack_pop();
		self.tick();
	}

	fn retc(&mut self, cond: bool) {
		self.tick();

		if cond {
			self.pc = self.stack_pop();
			self.tick();
		}
	}

	fn reti(&mut self) {
		let addr = self.stack_pop();
		// TODO: set IME=1
		self.tick();
	}

	fn rst(&mut self, interrupt: u16) {
		self.stack_push(self.pc);
		self.pc = interrupt;
	}
}