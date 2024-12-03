use std::ops::{Not, Shl, Shr};

use bitfield_struct::bitfield;
use bitflags::bitflags;

use crate::{bus::Bus, instr::{InstrTarget, Instruction, TargetKind, INSTRUCTIONS}};

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
	mcycles: usize,
	pub bus: [u8; 0x10000],
}

impl core::fmt::Debug for Cpu {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.debug_struct("Cpu").field("a", &self.a).field("f", &self.f).field("bc", &self.bc).field("de", &self.de).field("hl", &self.hl).field("sp", &self.sp).field("pc", &self.pc).field("ime", &self.ime).field("ime_to_set", &self.ime_to_set).field("cycles", &self.mcycles)
					.finish()
		}
}

impl Cpu {
	pub fn new() -> Self {
		Self {
			a: 1,
			f: Flags::from_bits_retain(0xB0),
			bc: Register16::from_bits(0x13),
			de: Register16::from_bits(0xD8),
			hl: Register16::from_bits(0x14D),
			sp: 0xFFFE,
			pc: 0x0100,
			ime: false,
			ime_to_set: false,
			mcycles: 0,
			bus: [0; 0x10000],
		}
	}

	fn af(&self) -> u16 {
		((self.a as u16) << 8) | self.f.bits() as u16
	}

	fn set_af(&mut self, val: u16) {
		self.a = (val >> 8) as u8;
		self.f = Flags::from_bits_retain(val as u8 & 0xFF)
	}

	fn update_hl(&mut self, target: &InstrTarget) {
		if target.increment { self.hl.0 = self.hl.0.wrapping_add(1); }
		else if target.decrement { self.hl.0 = self.hl.0.wrapping_sub(1); }
	}

	fn set_carry(&mut self, val: u16) {
		self.f.set(Flags::c, val > u8::MAX as u16);
	}

	fn set_carry16(&mut self, val: u32) {
		self.f.set(Flags::c, val > u16::MAX as u32);
	}

	// Be sure to always set after flag n
	fn set_hcarry(&mut self, a: u8, b: u8) {
		let res = if self.f.contains(Flags::n) {
			((a & 0xF).wrapping_sub(b & 0xF)) & 0x10 != 0
		} else {
			((a & 0xF).wrapping_add(b & 0xF)) & 0x10 != 0
		};
		self.f.set(Flags::h, res);
	}

	fn set_hcarry16(&mut self, a: u16, b: u16) {
		let res = if self.f.contains(Flags::n) {
			((a & 0xFFF).wrapping_sub(b & 0xFFF)) & 0x1000 != 0
		} else {
			((a & 0xFFF).wrapping_add(b & 0xFFF)) & 0x1000 != 0
		};
		self.f.set(Flags::h, res);
	}

	fn set_z(&mut self, val: u8) {
		self.f.set(Flags::z, val == 0);
	}

	pub fn peek(&mut self, addr: u16) -> u8 {
		self.bus[addr as usize]
	}

	fn read(&mut self, addr: u16) -> u8 {
		let res = self.peek(addr);
		self.tick();
		res
	}
	fn read16(&mut self, addr: u16) -> u16 {
		u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
	}
	fn write(&mut self, addr: u16, val: u8) {
		println!("Wrote {val:02X} to {addr:04X}");
		self.bus[addr as usize] = val;
	}
	fn write16(&mut self, addr: u16, val: u16){
		let [lo, hi] = val.to_le_bytes();
		self.write(addr as u16, lo);
		self.write(addr.wrapping_add(1) as u16, hi);
	}
	fn pc_fetch(&mut self) -> u8 {
		let res = self.read(self.pc);
		self.pc = self.pc.wrapping_add(1);
		res
	}
	fn pc_fetch16(&mut self) -> u16 {
		u16::from_le_bytes([
			self.pc_fetch(), self.pc_fetch()
		])
	}
	fn stack_push(&mut self, val: u16) {
		// TODO: not sure it is ok
		self.sp = self.sp.wrapping_sub(2);
		self.tick();
		self.write16(self.sp, val);
	}
	fn stack_pop(&mut self) -> u16 {
		let res = self.read16(self.sp);
		self.sp = self.sp.wrapping_add(2);
		res
	}
	fn tick(&mut self) {
		self.mcycles += 1;
	}

	pub fn step(&mut self) {
		if self.ime_to_set {
			self.ime = true;
			self.ime_to_set = false;
		} else if self.ime {
			// handle interrupts
		}

		let opcode = self.pc_fetch();
		
		if opcode == 0xCB {
			let opcode = self.pc_fetch();
			let instr = &INSTRUCTIONS[256 + opcode as usize];
			self.execute_prefix(instr);
		} else { 
			let instr = &INSTRUCTIONS[opcode as usize];
			self.execute_no_prefix(instr)
		}
	}

	fn hram(&self, offset: u8) -> u16 {
		0xFF00 | offset as u16
	}

	fn get_operand(&mut self, target: &InstrTarget) -> u8 {
		match (&target.kind, target.immediate) {
			(TargetKind::Immediate8 | TargetKind::Signed8, _) 
			| (TargetKind::Address8, true) => self.pc_fetch(),
			(TargetKind::Address8, false) => {
				let offset = self.pc_fetch();
				self.read(self.hram(offset))
			}
			(TargetKind::Address16, false) => {
				let addr = self.pc_fetch16();
				self.read(addr)
			}
			(TargetKind::A, _) => self.a,
			(TargetKind::B, _) => self.bc.hi(),
			(TargetKind::C, true)  => self.bc.lo(),
			(TargetKind::C, false) => {
				let offset = self.bc.lo();
				self.read(self.hram(offset))
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
				self.update_hl(target);
				res
			}
			_ => unreachable!("{:?}", target.kind),
		}
	}

	fn set_result(&mut self, target: &InstrTarget, val: u8) {
		match (&target.kind, target.immediate) {
			(TargetKind::Address8, false) => {
				let offset = self.pc_fetch();
				self.write(self.hram(offset), val);
			}
			(TargetKind::Address16, false) => {
				let addr = self.pc_fetch16();
				self.write(addr, val);
			}
			(TargetKind::A, _) => self.a = val,
			(TargetKind::B, _) => self.bc.set_hi(val),
			(TargetKind::C, true) => self.bc.set_lo(val),
			(TargetKind::C, false) => {
				let offset = self.bc.lo();
				self.write(self.hram(offset), val);
			},
			(TargetKind::D, _) => self.de.set_hi(val),
			(TargetKind::E, _) => self.de.set_lo(val),
			(TargetKind::F, _) => self.f = Flags::from_bits_retain(val),
			(TargetKind::H, _) => self.hl.set_hi(val),
			(TargetKind::L, _) => self.hl.set_lo(val),
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
				self.update_hl(target);
			}
			_ => unreachable!("{:?}", target.kind),
		}
	}

	fn get_operand16(&mut self, target: &InstrTarget) -> u16 {
		match (&target.kind, target.immediate) {
			(TargetKind::Address8, false) => {
				let offset = self.pc_fetch();
				self.read16(self.hram(offset))
			}
			(TargetKind::Immediate16, _) | (TargetKind::Address16, true) => self.pc_fetch16(),
			(TargetKind::Address16, false) => {
				let addr = self.pc_fetch16();
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
				self.update_hl(target);
				res
			}
			_ => unreachable!("{:?}", target.kind),
		}
	}

	fn set_result16(&mut self, target: &InstrTarget, val: u16) {
		match (&target.kind, target.immediate) {
			(TargetKind::Address8, false) => {
				let offset = self.pc_fetch();
				self.write16(self.hram(offset), val);
			}
			(TargetKind::Address16, false) => {
				let addr = self.pc_fetch16();
				self.write16(addr, val);
			}
			(TargetKind::SP, _) => self.sp = val,
			(TargetKind::AF, _) => self.set_af(val),
			(TargetKind::BC, true) => self.bc.0 = val,
			(TargetKind::BC, false) => self.write16(self.bc.0, val),
			(TargetKind::DE, true) => self.de.0 = val,
			(TargetKind::DE, false) => self.write16(self.de.0, val),
			(TargetKind::HL, true)  => self.hl.0 = val,
			_ => unreachable!("{:?}", target.kind),
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
	fn nop(&mut self) {}

	fn ld(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		self.set_result(&ops[0], val);
	}

	fn ld16(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand16(&ops[1]);
		self.set_result16(&ops[0], val);
	}

	// 0xf9
	fn ldhl(&mut self) {
		self.sp = self.hl.0;
		self.tick();
	}

	fn push(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand16(&ops[0]);
		self.stack_push(val);
	}

	fn pop(&mut self, ops: &[InstrTarget]) {
		let val = self.stack_pop();
		self.set_result16(&ops[0], val);
	}

	// 0xf8
	fn ldsp(&mut self, ops: &[InstrTarget]) {
		let offset = self.get_operand(&ops[1]) as i8;
		let (res, carry) = self.sp.overflowing_add_signed(offset as i16);
		
		self.f.remove(Flags::z);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, carry);
		self.set_hcarry16(self.sp, offset as u16);
		
		self.set_result16(&ops[0], res);
		self.tick();
	}

	fn add(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = self.a as u16 + val as u16;
		
		self.set_z(res as u8);
		self.f.remove(Flags::n);
		self.set_carry(res);
		self.set_hcarry(self.a, val);

		self.a = res as u8;
	}

	fn adc(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = self.a as u16 
			+ val as u16
			+ self.f.contains(Flags::c) as u16; 
		
		self.set_z(res as u8);
		self.f.remove(Flags::n);
		self.set_carry(res);
		self.set_hcarry(self.a, val);
		
		self.a = res as u8;
	}

	fn sub(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = (self.a as u16).wrapping_sub(val as u16);
		
		self.set_z(res as u8);
		self.f.insert(Flags::n);
		self.set_carry(res);
		self.set_hcarry(self.a, val);

		self.a = res as u8;
	}

	fn sbc(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = (self.a as u16)
			.wrapping_sub(val as u16)
			.wrapping_sub(self.f.contains(Flags::c) as u16);

		
		self.set_z(res as u8);
		self.f.insert(Flags::n);
		self.set_carry(res);
		self.set_hcarry(self.a, val);
		
		self.a = res as u8;
	}

	fn cp(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = (self.a as u16).wrapping_sub(val as u16);
		
		self.set_z(res as u8);
		self.f.insert(Flags::n);
		self.set_carry(res);
		self.set_hcarry(self.a, val);
	}

	fn inc(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let res = val.wrapping_add(1);
		
		self.set_z(res);
		self.f.remove(Flags::n);
		self.set_hcarry(val, 1);

		self.set_result(&ops[0], res);
	}

	fn inc16(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand16(&ops[0]);
		let res = val.wrapping_add(1);
		self.set_result16(&ops[0], res);
	}

	fn dec(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let res = val.wrapping_sub(1);
		
		self.f.set(Flags::z, res == 0);
		self.f.insert(Flags::n);
		self.set_hcarry(val, 1);

		self.set_result(&ops[0], res);
	}

	fn dec16(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand16(&ops[0]);
		let res = val.wrapping_sub(1);
		self.set_result16(&ops[0], res);
	}

	fn and(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = self.a & val;

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.insert(Flags::h);
		self.f.remove(Flags::c);

		self.a = res;
	}

	fn or(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = self.a | val;

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);

		self.a = res;
	}

	fn xor(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[1]);
		let res = self.a ^ val;

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);
		
		self.a = res;
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

	fn daa(&mut self, ops: &[InstrTarget]) { todo!() }

	fn cpl(&mut self) {
		self.a = self.a.not();
		self.f.insert(Flags::n);
		self.f.insert(Flags::h);
	}

	fn addhl(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand16(&ops[1]);
		let res = self.hl.0 as u32 + val as u32;

		self.f.remove(Flags::n);
		self.set_carry16(res);
		self.set_hcarry16(self.hl.0, val);

		self.hl.0 = res as u16;
		self.tick();
	}

	// 0xe8
	fn addsp(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand16(&ops[1]) as i8;
		let (res, carry) = self.sp.overflowing_add_signed(val as i16);
		
		self.set_z(res as u8);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, carry);
		self.set_hcarry16(self.sp, val as u16);
		
		self.hl.0 = res as u16;
		self.tick();
		self.tick();
	}

	fn rlca(&mut self, ops: &[InstrTarget]) {
		let val = self.a;
		let res = val.rotate_left(1);

		self.f.remove(Flags::z);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 0x80 != 0);
		self.f.remove(Flags::h);

		self.a = res;
	}

	fn rrca(&mut self, ops: &[InstrTarget]) {
		let val = self.a;
		let res = val.rotate_right(1);

		self.f.remove(Flags::z);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 1 != 0);
		self.f.remove(Flags::h);

		self.a = res;
	}

	fn rla(&mut self, ops: &[InstrTarget]) {
		let val = self.a;
		let res = val.rotate_left(1) | self.f.contains(Flags::c) as u8;

		self.f.remove(Flags::z);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 0x80 != 0);
		self.f.remove(Flags::h);

		self.a = res;
	}

	fn rra(&mut self, ops: &[InstrTarget]) {
		let val = self.a;
		let res = ((self.f.contains(Flags::c) as u8) << 7) | val.rotate_right(1);

		self.f.remove(Flags::z);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 1 != 0);
		self.f.remove(Flags::h);

		self.a = res;
	}
	
	fn rlc(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let res = val.rotate_left(1);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 0x80 != 0);
		self.f.remove(Flags::h);

		self.set_result(&ops[0], res);
	}

	fn rrc(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let res = val.rotate_right(1);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 1 != 0);
		self.f.remove(Flags::h);

		self.set_result(&ops[0], res);
	}

	fn rl(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let res = val.rotate_left(1) | self.f.contains(Flags::c) as u8;

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 0x80 != 0);
		self.f.remove(Flags::h);

		self.set_result(&ops[0], res);
	}

	fn rr(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let res = ((self.f.contains(Flags::c) as u8) << 7) | val.rotate_right(1);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 1 != 0);
		self.f.remove(Flags::h);

		self.set_result(&ops[0], res);
	}

	fn sla(&mut self, ops: &[InstrTarget]) {
		let val = self.a;
		let res = val.shl(1);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 0x80 != 0);
		self.f.remove(Flags::h);

		self.a = res;
	}

	fn sra(&mut self, ops: &[InstrTarget]) {
		let val = self.a;
		let res = val.shr(1);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 1 != 0);
		self.f.remove(Flags::h);

		self.a = res;
	}

	fn swap(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let low = val & 0b0000_1111;
		let high = val & 0b1111_0000;
		let res = (low << 4) | (high >> 4);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);

		self.set_result(&ops[0], res);
	}

	fn srl(&mut self, ops: &[InstrTarget]) {
		let val = self.get_operand(&ops[0]);
		let res = val.shr(1);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, val & 1 != 0);
		self.f.remove(Flags::h);

		self.set_result(&ops[0], res);
	}

	fn bit(&mut self, ops: &[InstrTarget]) {
		let bit = self.get_bit_op(&ops[0]);
		let val = self.get_operand(&ops[1]);
		let res = val & (1 << bit);

		self.f.set(Flags::z, res != 0);
		self.f.remove(Flags::n);
		self.f.insert(Flags::h);
	}

	fn res(&mut self, ops: &[InstrTarget]) {
		let bit = self.get_bit_op(&ops[0]);
		let val = self.get_operand(&ops[1]);
		let res = (val & !(1 << bit)) | (val & (1 << bit));

		self.set_result(&ops[1], res);
	}

	fn set(&mut self, ops: &[InstrTarget]) {
		let bit = self.get_bit_op(&ops[0]);
		let val = self.get_operand(&ops[1]);
		let res = val & (1 << bit);

		self.set_result(&ops[1], res);
	}

	fn jp(&mut self, ops: &[InstrTarget]) {
		let addr = self.get_operand16(&ops[0]);
		self.pc = addr;
		self.tick();
	}

	// 0xe9
	fn jphl(&mut self) {
		self.pc = self.hl.0;
	}

	fn jpc(&mut self, ops: &[InstrTarget]) {
		let addr = self.get_operand16(&ops[1]);
		if self.get_cond(&ops[0]) {
			self.pc = addr;
			self.tick();
		}
	}

	fn jr(&mut self, ops: &[InstrTarget]) {
		let offset = self.get_operand(&ops[0]) as i8;
		self.pc = self.pc.wrapping_add_signed(offset as i16);
		self.tick();
	}

	fn jrc(&mut self, ops: &[InstrTarget]) {
		let offset = self.get_operand(&ops[1]) as i8;
		if self.get_cond(&ops[0]) {
			self.pc = self.pc.wrapping_add_signed(offset as i16);
			self.tick();
		}
	}

	fn call(&mut self, ops: &[InstrTarget]) {
		let addr = self.get_operand16(&ops[0]);
		self.stack_push(self.pc);
		self.pc = addr;
		self.tick();
	}

	fn callc(&mut self, ops: &[InstrTarget]) {
		let addr = self.get_operand16(&ops[1]);
		if self.get_cond(&ops[0]) {
			self.stack_push(self.pc);
			self.pc = addr;
			self.tick();
		}
	}

	fn ret(&mut self) {
		self.pc = self.stack_pop();
		self.tick();
	}

	fn retc(&mut self, ops: &[InstrTarget]) {
		self.tick();

		if self.get_cond(&ops[0]) {
			self.pc = self.stack_pop();
			self.tick();
		}
	}

	fn reti(&mut self) {
		self.pc = self.stack_pop();
		self.ime = true;
		self.tick();
	}

	fn rst(&mut self, ops: &[InstrTarget]) {
		let addr = self.get_interrupt_addr(&ops[0]);
		self.stack_push(self.pc);
		self.pc = addr;
		self.tick();
	}

	fn di(&mut self) { self.ime = false; self.ime_to_set = false; }
	fn ei(&mut self) { self.ime_to_set = true; }

	fn stop(&mut self, ops: &[InstrTarget]) {  }
	fn halt(&mut self, ops: &[InstrTarget]) {  }
}


impl Cpu {
  fn execute_no_prefix(&mut self, instr: &Instruction) {
    let ops = &instr.operands;
		match instr.opcode {
			0x00 => self.nop(),
			0x02 | 0x06 | 0x08 | 0x0a | 0x0e | 0x12 | 0x16 | 0x1a | 0x1e |
			0x22 | 0x26 | 0x2a | 0x2e | 0x32 | 0x36 | 0x3a | 0x3e |
			0x40 ..= 0x75 | 0x77 ..= 0x7f |
			0xe0 | 0xe2 | 0xea | 0xf0 | 0xf2 | 0xfa => self.ld(ops),
			0x01 | 0x11 | 0x21 | 0x31 => self.ld16(ops),
			0xf8 => self.ldsp(ops),
			0xf9 => self.ldhl(),
			0x04 | 0x0c | 0x14 | 0x1c | 0x24 | 0x2c | 0x34 | 0x3c => self.inc(ops),
			0x03 | 0x13 | 0x23 | 0x33 => self.inc16(ops),
			0x05 | 0x0d | 0x15 | 0x1d | 0x25 | 0x2d | 0x35 | 0x3d => self.dec(ops),
			0x0b | 0x1b | 0x2b | 0x3b => self.dec16(ops),
			0x07 => self.rlca(ops),
			0x80 | 0x81 | 0x82 | 0x83 | 0x84 | 0x85 | 0x86 | 0x87 | 0xc6 => self.add(ops),
			0x09 | 0x19 | 0x29 | 0x39 => self.addhl(ops),
			0xe8 => self.addsp(ops),
			0x0f => self.rrca(ops), 
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
			0x88 ..= 0x8f | 0xce => self.adc(ops),
			0x90 ..= 0x97 | 0xd6 => self.sub(ops),
			0x98 ..= 0x9f | 0xde => self.sbc(ops),
			0xa0 ..= 0xa7 | 0xe6 => self.and(ops),
			0xa8 ..= 0xaf | 0xee => self.xor(ops),
			0xb0 ..= 0xb7 | 0xf6 => self.or(ops),
			0xb8 ..= 0xbf | 0xfe => self.cp(ops),
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
			_ => todo!("{:02X}: {} not reachable", instr.opcode, instr.name)
    }
  }

	fn execute_prefix(&mut self, instr: &Instruction) {
		let ops = &instr.operands;
		match instr.opcode {
			0x00 ..= 0x07 => self.rlc(ops),
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
		}
	}
}
