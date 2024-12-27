use std::ops::{Not, Shl, Shr, BitAnd, BitOr, BitXor};

use bitfield_struct::bitfield;
use bitflags::bitflags;

use crate::{
	bus::{Bus, IFlags, SharedBus}, instr::{InstrTarget, Instruction, TargetKind, ACC_TARGET, INSTRUCTIONS}, ppu::Ppu
};

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

	dma: Dma,
	halted: bool,

	pub mcycles: usize,

	pub bus: SharedBus,
	pub ppu: Ppu,
}

impl core::fmt::Debug for Cpu {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.debug_struct("Cpu").field("a", &self.a).field("f", &self.f).field("bc", &self.bc).field("de", &self.de).field("hl", &self.hl).field("sp", &self.sp).field("pc", &self.pc).field("ime", &self.ime).field("ime_to_set", &self.ime_to_set).field("cycles", &self.mcycles)
					.finish()
		}
}

#[derive(Default)]
struct Dma {
	pub transfering: bool,
	pub start: u16,
	pub offset: u16,
	pub start_delay: bool,
}
impl Dma {
	pub fn init(&mut self, val: u8) {
		self.start = (val as u16) << 8;
		self.offset = 0;
		self.start_delay = true;
	}

	pub fn current(&self) -> u16 {
		self.start + self.offset
	}

	fn is_done(&self) -> bool {
		self.offset >= 0x9F
	}

	pub fn tick(&mut self) {
		self.offset += 1;
		self.transfering = !self.is_done();
	}
}

impl Cpu {
	fn immediate8(&mut self) -> u8 { self.pc_fetch() }
	fn indirect_zero8(&mut self) -> u8 {
		let offset = self.pc_fetch();
		self.read(self.hram(offset))
	}
	fn indirect_abs8(&mut self) -> u8 {
		let addr = self.pc_fetch16();
		self.read(addr)
	}
	fn immediate16(&mut self) -> u16 {
		self.pc_fetch16()
	}
	fn indirect_zero16(&mut self) -> u16 {
		let offset = self.pc_fetch();
		self.read16(self.hram(offset))
	}
	fn indirect_abs16(&mut self) -> u16 {
		let addr = self.pc_fetch16();
		self.read16(addr)
	}

	fn a(&mut self) -> u8 { self.a }
	fn f(&mut self) -> u8 { self.f.bits() }
	fn b(&mut self) -> u8 { self.bc.hi() }
	fn c(&mut self) -> u8 { self.bc.lo() }
	fn c_indirect(&mut self) -> u8 {
		let offset = self.c();
		self.read(self.hram(offset))
	}

	fn d(&mut self) -> u8 { self.de.hi() }
	fn e(&mut self) -> u8 { self.de.lo() }
	fn h(&mut self) -> u8 { self.hl.hi() }
	fn l(&mut self) -> u8 { self.hl.lo() }

	fn bc_indirect(&mut self) -> u8 { self.read(self.bc.0) }
	fn de_indirect(&mut self) -> u8 { self.read(self.de.0) }
	fn hl_indirect8(&mut self) -> u8 { self.read(self.hl.0) }
	fn hl_inc_indirect8(&mut self) -> u8 {
		let res = self.hl_indirect8();
		self.hl.0 = self.hl.0.wrapping_add(1);
		res
	}
	fn hl_dec_indirect8(&mut self) -> u8 {
		let res = self.hl_indirect8();
		self.hl.0 = self.hl.0.wrapping_sub(1);
		res
	}

	fn sp(&mut self) -> u16 { self.sp }
	fn af(&mut self) -> u16 { ((self.a as u16) << 8) | self.f.bits() as u16 }
	fn bc(&mut self) -> u16 { self.bc.0 }
	fn de(&mut self) -> u16 { self.de.0 }
	fn hl(&mut self) -> u16 { self.hl.0 }
	fn hl_indirect16(&mut self) -> u16 { self.read16(self.hl.0) }
	fn hl_inc_indirect16(&mut self) -> u16 {
		let res = self.hl_indirect16();
		self.hl.0 = self.hl.0.wrapping_add(1);
		res
	}
	fn hl_dec_indirect16(&mut self) -> u16 {
		let res = self.hl_indirect16();
		self.hl.0 = self.hl.0.wrapping_sub(1);
		res
	}

	fn set_indirect_zero8(&mut self, val: u8) {
		let offset = self.pc_fetch();
		self.write(self.hram(offset), val);
	}
	fn set_indirect_abs8(&mut self, val: u8) {
		let addr = self.pc_fetch16();
		self.write(addr, val);
	}
	fn set_indirect_zero16(&mut self, val: u16) {
		let offset = self.pc_fetch();
		self.write16(self.hram(offset), val);
	}
	fn set_indirect_abs16(&mut self, val: u16) {
		let addr = self.pc_fetch16();
		self.write16(addr, val);
	}

	fn set_a(&mut self, val: u8) { self.a = val; }
	fn set_f(&mut self, val: u8) { self.f = Flags::from_bits_truncate(val & 0xF0); }
	fn set_b(&mut self, val: u8) { self.bc.set_hi(val) }
	fn set_c(&mut self, val: u8) { self.bc.set_lo(val) }
	fn set_c_indirect(&mut self, val: u8) {
		let offset = self.c();
		self.write(self.hram(offset), val);
	}

	fn set_d(&mut self, val: u8) { self.de.set_hi(val) }
	fn set_e(&mut self, val: u8) { self.de.set_lo(val) }
	fn set_h(&mut self, val: u8) { self.hl.set_hi(val) }
	fn set_l(&mut self, val: u8) { self.hl.set_lo(val) }

	fn set_bc_indirect8(&mut self, val: u8) { self.write(self.bc.0, val); }
	fn set_de_indirect8(&mut self, val: u8) { self.write(self.de.0, val); }
	fn set_hl_indirect8(&mut self, val: u8) { self.write(self.hl.0, val); }
	fn set_hl_inc_indirect8(&mut self, val: u8) {
		self.set_hl_indirect8(val);
		self.hl.0 = self.hl.0.wrapping_add(1);
	}
	fn set_hl_dec_indirect8(&mut self, val: u8) {
		self.set_hl_indirect8(val);
		self.hl.0 = self.hl.0.wrapping_sub(1);
	}

	fn set_sp(&mut self, val: u16) { self.sp = val; }
	fn set_af(&mut self, val: u16) {
		self.a = (val >> 8) as u8;
		self.set_f(val as u8);
	}
	fn set_bc(&mut self, val: u16) { self.bc.0 = val; }
	fn set_de(&mut self, val: u16) { self.de.0 = val; }
	fn set_hl(&mut self, val: u16) { self.hl.0 = val; }

	fn z(&mut self) -> bool { self.f.contains(Flags::z) }
	fn carry(&mut self) -> bool { self.f.contains(Flags::c) }
	fn nz(&mut self) -> bool { !self.f.contains(Flags::z) }
	fn ncarry(&mut self) -> bool { !self.f.contains(Flags::c) }
}

impl Cpu {
	pub fn new(rom: &[u8]) -> Self {
		let bus = Bus::new(rom);

		Self {
			a: 1,
			f: Flags::from_bits_truncate(0xB0),
			bc: Register16::from_bits(0x13),
			de: Register16::from_bits(0xD8),
			hl: Register16::from_bits(0x14D),
			sp: 0xFFFE,
			pc: 0x0100,
			ime: false,
			ime_to_set: false,
			halted: false,
			mcycles: 0,
			ppu: Ppu::new(bus.clone()),
			dma: Dma::default(),
			bus,
		}
	}

	fn set_carry(&mut self, val: u16) {
		self.f.set(Flags::c, val > u8::MAX as u16);
	}

	fn set_carry16(&mut self, val: u32) {
		self.f.set(Flags::c, val > u16::MAX as u32);
	}

	// Be sure to always set after flag n
	fn set_hcarry_full(&mut self, a: u8, b: u8, c: u8) {
		let res = if self.f.contains(Flags::n) {
			((a & 0xF).wrapping_sub(b & 0xF).wrapping_sub(c & 0xF)) & 0x10 != 0
		} else {
			((a & 0xF).wrapping_add(b & 0xF).wrapping_add(c & 0xF)) & 0x10 != 0
		};
		self.f.set(Flags::h, res);
	}

	fn set_hcarry(&mut self, a: u8, b: u8) {
		self.set_hcarry_full(a, b, 0);
	}
	fn set_hcarry_with_carry(&mut self, a: u8, b: u8) {
		self.set_hcarry_full(a, b, self.f.contains(Flags::c) as u8);
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
		self.bus.borrow().read(addr)
	}

	pub fn read(&mut self, addr: u16) -> u8 {
		let res = self.peek(addr);
		self.tick();
		res
	}
	fn read16(&mut self, addr: u16) -> u16 {
		u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
	}

	pub fn write(&mut self, addr: u16, val: u8) {
		if addr == 0xFF46 {
			self.dma.init(val);
		} else {
			self.bus.borrow_mut().write(addr, val);
		}

		self.tick();
	}
	fn dma_write(&mut self) {
		let val = self.peek(self.dma.current());
		self.bus.borrow_mut().write(0xFE00 + self.dma.offset, val);
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
		self.tick();
		self.write16(self.sp.wrapping_sub(2), val);
		self.sp = self.sp.wrapping_sub(2);
	}
	fn stack_pop(&mut self) -> u16 {
		let value = self.read16(self.sp);
		self.sp = self.sp.wrapping_add(2);
		value
	}

	fn tick(&mut self) {
		self.mcycles += 1;
		for _ in 0..4 { self.ppu.tick(); }

		let mut bus = self.bus.borrow_mut();
		bus.timer.tick();
	}

	pub fn step(&mut self) {
		if self.halted {
			let bus = self.bus.borrow();
			let inte = bus.inte;
			let intf = bus.intf();
			drop(bus);

			if !(inte & intf).is_empty() { self.halted = false; }
			else { self.tick(); }

			return;
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

		if self.dma.start_delay {
			self.dma.start_delay = false;
			self.dma.transfering = true;
		} else if self.dma.transfering {
			self.dma_write();
			self.dma.tick();
		}

		if self.ime_to_set {
			self.ime = true;
			self.ime_to_set = false;
		} else if self.ime {
			self.handle_interrupts();
		}
	}

	pub fn debug_step(&mut self) {
		let opcode = self.peek(self.pc-1);

		if opcode == 0xCB {
			let opcode = self.pc_fetch();
			let instr = &INSTRUCTIONS[256 + opcode as usize];
			self.execute_prefix(instr);
		} else { 
			let instr = &INSTRUCTIONS[opcode as usize];
			self.execute_no_prefix(instr)
		}

		self.pc_fetch();
	}

	fn handle_interrupts(&mut self) {
		let bus = self.bus.borrow();
		let mut intf = bus.intf();

		let mut pending_ints = (bus.inte & intf).iter().collect::<Vec<_>>();
		pending_ints.reverse();

		for int in pending_ints {
			let addr = match int {
				IFlags::vblank => 0x40,
				IFlags::lcd    => 0x48,
				IFlags::timer  => 0x50,
				IFlags::serial => 0x58, 
				IFlags::joypad => 0x60,
				_ => unreachable!(),
			};

			intf.remove(int);
			bus.set_intf(intf);
			drop(bus);

			self.ime = false;

			// 2 wait states are executed
			self.tick();
			self.tick();

			self.stack_push(self.pc);
			self.pc = addr;
			self.tick();
			
			// we don't want to handle any more interrupt
			break;
		}
	}

	fn hram(&self, offset: u8) -> u16 {
		0xFF00 | offset as u16
	}
}

type InstrSrc8  = fn(&mut Cpu) -> u8;
type InstrDst8  = fn(&mut Cpu, u8);
type InstrSrc16 = fn(&mut Cpu) -> u16;
type InstrDst16 = fn(&mut Cpu, u16);
type InstrCond  = fn(&mut Cpu) -> bool;

impl Cpu {
	fn nop(&mut self) {}

	fn ld(&mut self, dst: InstrDst8, src: InstrSrc8) {
		let val = src(self);
		dst(self, val);
	}

	fn ld16(&mut self, dst: InstrDst16, src: InstrSrc16) {
		let val = src(self);
		dst(self, val);
	}

	// 0xf9
	fn ldhl(&mut self) {
		self.sp = self.hl.0;
		self.tick();
	}

	fn push(&mut self, src: InstrSrc16) {
		let val = src(self);
		self.stack_push(val);
	}

	fn pop(&mut self, dst: InstrDst16) {
		let val = self.stack_pop();
		dst(self, val);
	}

	// 0xf8
	// https://stackoverflow.com/questions/5159603/gbz80-how-does-ld-hl-spe-affect-h-and-c-flags
	fn ldsp(&mut self, dst: InstrDst16, src: InstrSrc8) {
		let offset = src(self) as i8;
		let res = self.sp.wrapping_add_signed(offset as i16);
		
		self.f.remove(Flags::z);
		self.f.remove(Flags::n);

		// TODO: probably not correct
		if offset.is_negative() {
			self.f.set(Flags::c, res & 0xFF <= self.sp & 0xFF);
			self.f.set(Flags::h, res & 0xF <= self.sp & 0xF);
		} else {
			self.set_carry((self.sp & 0xFF).wrapping_add_signed(offset as i16));
			self.set_hcarry(self.sp as u8, offset as u8);
		}

		dst(self, res);
		self.tick();
	}

	// fn add_full(&mut self) {
		// TODO
	// }

	fn add(&mut self, src: InstrSrc8) {
		let val = src(self);
		let res = self.a as u16 + val as u16;
		
		self.set_z(res as u8);
		self.f.remove(Flags::n);
		self.set_hcarry(self.a, val);
		self.set_carry(res);

		self.a = res as u8;
	}

	fn adc(&mut self, src: InstrSrc8) {
		let val = src(self);
		let res = self.a as u16 
			+ val as u16
			+ self.f.contains(Flags::c) as u16; 
		
		self.set_z(res as u8);
		self.f.remove(Flags::n);
		self.set_hcarry_with_carry(self.a, val);
		self.set_carry(res);
		
		self.a = res as u8;
	}

	fn sub(&mut self, src: InstrSrc8) {
		let val = src(self);
		let res = (self.a as u16).wrapping_sub(val as u16);
		
		self.set_z(res as u8);
		self.f.insert(Flags::n);
		self.set_hcarry(self.a, val);
		self.set_carry(res);

		self.a = res as u8;
	}

	fn sbc(&mut self, src: InstrSrc8) {
		let val = src(self);
		let res = (self.a as u16)
			.wrapping_sub(val as u16)
			.wrapping_sub(self.f.contains(Flags::c) as u16);

		
		self.set_z(res as u8);
		self.f.insert(Flags::n);
		self.set_hcarry_with_carry(self.a, val);
		self.set_carry(res);
		
		self.a = res as u8;
	}

	fn cp(&mut self, src: InstrSrc8) {
		let val = src(self);
		let res = (self.a as u16).wrapping_sub(val as u16);
		
		self.set_z(res as u8);
		self.f.insert(Flags::n);
		self.set_hcarry(self.a, val);
		self.set_carry(res);
	}

	fn inc(&mut self, dst: InstrDst8, src: InstrSrc8) {
		let val = src(self);
		let res = val.wrapping_add(1);
		
		self.set_z(res);
		self.f.remove(Flags::n);
		self.set_hcarry(val, 1);

		dst(self, res);
	}

	fn inc16(&mut self, dst: InstrDst16, src: InstrSrc16) {
		let val = src(self);
		let res = val.wrapping_add(1);
		dst(self, res);
		self.tick();
	}

	fn dec(&mut self, dst: InstrDst8, src: InstrSrc8) {
		let val = src(self);
		let res = val.wrapping_sub(1);
		
		self.f.set(Flags::z, res == 0);
		self.f.insert(Flags::n);
		self.set_hcarry(val, 1);

		dst(self, res);
	}

	fn dec16(&mut self, dst: InstrDst16, src: InstrSrc16) {
		let val = src(self);
		let res = val.wrapping_sub(1);
		dst(self, res);
		self.tick();
	}

	fn logical<F: Fn(u8, u8) -> u8>(&mut self, src: InstrSrc8, f: F) {
		let val = src(self);
		let res = f(self.a, val);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.remove(Flags::c);
		self.a = res;
	}

	fn and(&mut self, src: InstrSrc8) {
		self.logical(src, u8::bitand);
		self.f.insert(Flags::h);
	}

	fn or(&mut self, src: InstrSrc8) {
		self.logical(src, u8::bitor);
		self.f.remove(Flags::h);
	}

	fn xor(&mut self, src: InstrSrc8) {
		self.logical(src, u8::bitxor);
		self.f.remove(Flags::h);
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

	// https://ehaskins.com/2018-01-30%20Z80%20DAA/
	fn daa(&mut self) {
		let mut correction = 0u8;
		let mut carry = false;

		if self.f.contains(Flags::h)
		|| (!self.f.contains(Flags::n) && self.a & 0xF > 0x9) {
			correction += 0x6;
		}

		if self.f.contains(Flags::c)
		|| (!self.f.contains(Flags::n) && self.a > 0x99) {
			correction += 0x60;
			carry = true;
		}

		correction = if self.f.contains(Flags::n) {
			correction.wrapping_neg()
		} else { correction };

		let res = self.a.wrapping_add(correction);

		self.set_z(res);
		self.f.set(Flags::c, carry);
		self.f.remove(Flags::h);

		self.a = res;
	}

	fn cpl(&mut self) {
		self.a = self.a.not();
		self.f.insert(Flags::n);
		self.f.insert(Flags::h);
	}

	fn addhl(&mut self, src: InstrSrc16) {
		let val = src(self);
		let res = self.hl.0 as u32 + val as u32;

		self.f.remove(Flags::n);
		self.set_carry16(res);
		self.set_hcarry16(self.hl.0, val);

		self.hl.0 = res as u16;
		self.tick();
	}

	// 0xe8
	// https://stackoverflow.com/questions/5159603/gbz80-how-does-ld-hl-spe-affect-h-and-c-flags
	fn addsp(&mut self, src: InstrSrc8) {
		let offset = src(self) as i8;
		let res = self.sp.wrapping_add_signed(offset as i16);
		
		self.f.remove(Flags::z);
		self.f.remove(Flags::n);

		// TODO: probably not correct
		if offset.is_negative() {
			self.f.set(Flags::c, res & 0xFF <= self.sp & 0xFF);
			self.f.set(Flags::h, res & 0x0F <= self.sp & 0x0F);
		} else {
			self.set_carry((self.sp & 0xFF).wrapping_add_signed(offset as i16));
			self.set_hcarry(self.sp as u8, offset as u8);
		}
		
		self.sp = res as u16;

		self.tick();
		self.tick();
	}

	fn shift_acc<FS: Fn(u8) -> u8, FB: Fn(u8) -> bool>(&mut self, f: FS, carry: FB) {
		self.shift(|cpu, val| cpu.a = val, |cpu| cpu.a, f, carry);
		self.f.remove(Flags::z);
	}

	fn rlca(&mut self) {
		self.shift_acc(|val| val.rotate_left(1), |val| val & 0x80 != 0);
	}

	fn rrca(&mut self) {
		self.shift_acc(|val| val.rotate_right(1), |val| val & 1 != 0);
	}

	fn rla(&mut self) {
		let carry = self.f.contains(Flags::c) as u8;
		self.shift_acc(|val| val.shl(1) | carry, |val| val & 0x80 != 0);
	}

	fn rra(&mut self) {
		let carry = self.f.contains(Flags::c) as u8;
		self.shift_acc(|val| (carry << 7) | val.shr(1), |val| val & 1 != 0);
	}

	fn shift<FS: Fn(u8) -> u8, FB: Fn(u8) -> bool>(&mut self, dst: InstrDst8, src: InstrSrc8, f: FS, carry: FB) {
		let val = src(self);
		let res = f(val);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.set(Flags::c, carry(val));
		self.f.remove(Flags::h);

		dst(self, res);
	}

	fn rlc(&mut self, dst: InstrDst8, src: InstrSrc8) {
		self.shift(dst, src, |val| val.rotate_left(1), |val| val & 0x80 != 0);
	}

	fn rrc(&mut self, dst: InstrDst8, src: InstrSrc8) {
		self.shift(dst, src, |val| val.rotate_right(1), |val| val & 1 != 0);
	}

	fn rl(&mut self, dst: InstrDst8, src: InstrSrc8) {
		let carry = self.f.contains(Flags::c) as u8;
		self.shift(dst, src, |val| val.shl(1) | carry, |val| val & 0x80 != 0);
	}

	fn rr(&mut self, dst: InstrDst8, src: InstrSrc8) {
		let carry = self.f.contains(Flags::c) as u8;
		self.shift(dst, src, |val| (carry << 7) | val.shr(1), |val| val & 1 != 0);
	}

	fn sla(&mut self, dst: InstrDst8, src: InstrSrc8) {
		self.shift(dst, src, |val| val.shl(1), |val| val & 0x80 != 0);
	}

	fn sra(&mut self, dst: InstrDst8, src: InstrSrc8) {
		self.shift(dst, src, |val| (val & 0b1000_0000) | val.shr(1), |val| val & 1 != 0);
	}

	fn srl(&mut self, dst: InstrDst8, src: InstrSrc8) {
		self.shift(dst, src, |val| val.shr(1), |val| val & 1 != 0);
	}

	fn swap(&mut self, dst: InstrDst8, src: InstrSrc8,) {
		let val = src(self);
		let low = val & 0b0000_1111;
		let high = val & 0b1111_0000;
		let res = (low << 4) | (high >> 4);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.remove(Flags::h);
		self.f.remove(Flags::c);

		dst(self, res);
	}

	fn bit(&mut self, bit: u8, src: InstrSrc8) {
		let val = src(self);
		let res = val & (1 << bit);

		self.set_z(res);
		self.f.remove(Flags::n);
		self.f.insert(Flags::h);
	}

	fn res(&mut self, bit: u8, dst: InstrDst8, src: InstrSrc8) {
		let val = src(self);
		let res = val & !(1 << bit);

		dst(self, res);
	}

	fn set(&mut self, bit: u8, dst: InstrDst8, src: InstrSrc8) {
		let val = src(self);
		let res = val | (1 << bit);

		dst(self, res);
	}

	fn jp(&mut self, src: InstrSrc16) {
		let addr = src(self);
		self.pc = addr;
		self.tick();
	}

	// 0xe9
	fn jphl(&mut self) {
		self.pc = self.hl.0;
	}

	fn jpc(&mut self, cond: InstrCond, src: InstrSrc16) {
		let addr = src(self);
		if cond(self) {
			self.pc = addr;
			self.tick();
		}
	}

	fn jr(&mut self, src: InstrSrc8) {
		let offset = src(self) as i8;
		self.pc = self.pc.wrapping_add_signed(offset as i16);
		self.tick();
	}

	fn jrc(&mut self, cond: InstrCond, src: InstrSrc8) {
		let offset = src(self) as i8;
		if cond(self) {
			self.pc = self.pc.wrapping_add_signed(offset as i16);
			self.tick();
		}
	}

	fn call(&mut self, src: InstrSrc16) {
		let addr = src(self);
		self.stack_push(self.pc);
		self.pc = addr;
		self.tick();
	}

	fn callc(&mut self, cond: InstrCond, src: InstrSrc16) {
		let addr = src(self);
		
		if cond(self) {
			self.stack_push(self.pc);
			self.pc = addr;
			self.tick();
		}
	}

	fn ret(&mut self) {
		self.pc = self.stack_pop();
		self.tick();
	}

	fn retc(&mut self, cond: InstrCond) {
		self.tick();
		if cond(self) {
			self.pc = self.stack_pop();
			self.tick();
		}
	}

	fn reti(&mut self) {
		self.ime = true;
		self.pc = self.stack_pop();
		self.tick();
	}

	fn rst(&mut self, int: u16) {
		self.tick();
		self.stack_push(self.pc);
		self.pc = int;
	}

	fn di(&mut self) { self.ime = false; self.ime_to_set = false; }
	fn ei(&mut self) { self.ime_to_set = true; }

	fn stop(&mut self, _src: InstrSrc8) {  } // TODO
	fn halt(&mut self) {
		// TODO: halt bug
		self.halted = true;
	}
}


impl Cpu {
  fn execute_no_prefix(&mut self, instr: &Instruction) {
    let ops = &instr.operands;
		match instr.opcode {
			0x00 => self.nop(),
			0x01 => self.ld16(Self::set_bc,Self::immediate16),
			0x02 => self.ld(Self::set_bc_indirect8,Self::a),
			0x03 => self.inc16(Self::set_bc,Self::bc),
			0x04 => self.inc(Self::set_b,Self::b),
			0x05 => self.dec(Self::set_b,Self::b),
			0x06 => self.ld(Self::set_b,Self::immediate8),
			0x07 => self.rlca(),
			0x08 => self.ld16(Self::set_indirect_abs16,Self::sp),
			0x09 => self.addhl(Self::bc),
			0x0A => self.ld(Self::set_a,Self::bc_indirect),
			0x0B => self.dec16(Self::set_bc,Self::bc),
			0x0C => self.inc(Self::set_c,Self::c),
			0x0D => self.dec(Self::set_c,Self::c),
			0x0E => self.ld(Self::set_c,Self::immediate8),
			0x0F => self.rrca(),
			0x10 => self.stop(Self::immediate8),
			0x11 => self.ld16(Self::set_de,Self::immediate16),
			0x12 => self.ld(Self::set_de_indirect8,Self::a),
			0x13 => self.inc16(Self::set_de,Self::de),
			0x14 => self.inc(Self::set_d,Self::d),
			0x15 => self.dec(Self::set_d,Self::d),
			0x16 => self.ld(Self::set_d,Self::immediate8),
			0x17 => self.rla(),
			0x18 => self.jr(Self::immediate8),
			0x19 => self.addhl(Self::de),
			0x1A => self.ld(Self::set_a,Self::de_indirect),
			0x1B => self.dec16(Self::set_de,Self::de),
			0x1C => self.inc(Self::set_e,Self::e),
			0x1D => self.dec(Self::set_e,Self::e),
			0x1E => self.ld(Self::set_e,Self::immediate8),
			0x1F => self.rra(),
			0x20 => self.jrc(Self::nz,Self::immediate8),
			0x21 => self.ld16(Self::set_hl,Self::immediate16),
			0x22 => self.ld(Self::set_hl_inc_indirect8,Self::a),
			0x23 => self.inc16(Self::set_hl,Self::hl),
			0x24 => self.inc(Self::set_h,Self::h),
			0x25 => self.dec(Self::set_h,Self::h),
			0x26 => self.ld(Self::set_h,Self::immediate8),
			0x27 => self.daa(),
			0x28 => self.jrc(Self::z,Self::immediate8),
			0x29 => self.addhl(Self::hl),
			0x2A => self.ld(Self::set_a,Self::hl_inc_indirect8),
			0x2B => self.dec16(Self::set_hl,Self::hl),
			0x2C => self.inc(Self::set_l,Self::l),
			0x2D => self.dec(Self::set_l,Self::l),
			0x2E => self.ld(Self::set_l,Self::immediate8),
			0x2F => self.cpl(),
			0x30 => self.jrc(Self::ncarry,Self::immediate8),
			0x31 => self.ld16(Self::set_sp,Self::immediate16),
			0x32 => self.ld(Self::set_hl_dec_indirect8,Self::a),
			0x33 => self.inc16(Self::set_sp,Self::sp),
			0x34 => self.inc(Self::set_hl_indirect8,Self::hl_indirect8),
			0x35 => self.dec(Self::set_hl_indirect8,Self::hl_indirect8),
			0x36 => self.ld(Self::set_hl_indirect8,Self::immediate8),
			0x37 => self.scf(),
			0x38 => self.jrc(Self::carry,Self::immediate8),
			0x39 => self.addhl(Self::sp),
			0x3A => self.ld(Self::set_a,Self::hl_dec_indirect8),
			0x3B => self.dec16(Self::set_sp,Self::sp),
			0x3C => self.inc(Self::set_a,Self::a),
			0x3D => self.dec(Self::set_a,Self::a),
			0x3E => self.ld(Self::set_a,Self::immediate8),
			0x3F => self.ccf(),
			0x40 => self.ld(Self::set_b,Self::b),
			0x41 => self.ld(Self::set_b,Self::c),
			0x42 => self.ld(Self::set_b,Self::d),
			0x43 => self.ld(Self::set_b,Self::e),
			0x44 => self.ld(Self::set_b,Self::h),
			0x45 => self.ld(Self::set_b,Self::l),
			0x46 => self.ld(Self::set_b,Self::hl_indirect8),
			0x47 => self.ld(Self::set_b,Self::a),
			0x48 => self.ld(Self::set_c,Self::b),
			0x49 => self.ld(Self::set_c,Self::c),
			0x4A => self.ld(Self::set_c,Self::d),
			0x4B => self.ld(Self::set_c,Self::e),
			0x4C => self.ld(Self::set_c,Self::h),
			0x4D => self.ld(Self::set_c,Self::l),
			0x4E => self.ld(Self::set_c,Self::hl_indirect8),
			0x4F => self.ld(Self::set_c,Self::a),
			0x50 => self.ld(Self::set_d,Self::b),
			0x51 => self.ld(Self::set_d,Self::c),
			0x52 => self.ld(Self::set_d,Self::d),
			0x53 => self.ld(Self::set_d,Self::e),
			0x54 => self.ld(Self::set_d,Self::h),
			0x55 => self.ld(Self::set_d,Self::l),
			0x56 => self.ld(Self::set_d,Self::hl_indirect8),
			0x57 => self.ld(Self::set_d,Self::a),
			0x58 => self.ld(Self::set_e,Self::b),
			0x59 => self.ld(Self::set_e,Self::c),
			0x5A => self.ld(Self::set_e,Self::d),
			0x5B => self.ld(Self::set_e,Self::e),
			0x5C => self.ld(Self::set_e,Self::h),
			0x5D => self.ld(Self::set_e,Self::l),
			0x5E => self.ld(Self::set_e,Self::hl_indirect8),
			0x5F => self.ld(Self::set_e,Self::a),
			0x60 => self.ld(Self::set_h,Self::b),
			0x61 => self.ld(Self::set_h,Self::c),
			0x62 => self.ld(Self::set_h,Self::d),
			0x63 => self.ld(Self::set_h,Self::e),
			0x64 => self.ld(Self::set_h,Self::h),
			0x65 => self.ld(Self::set_h,Self::l),
			0x66 => self.ld(Self::set_h,Self::hl_indirect8),
			0x67 => self.ld(Self::set_h,Self::a),
			0x68 => self.ld(Self::set_l,Self::b),
			0x69 => self.ld(Self::set_l,Self::c),
			0x6A => self.ld(Self::set_l,Self::d),
			0x6B => self.ld(Self::set_l,Self::e),
			0x6C => self.ld(Self::set_l,Self::h),
			0x6D => self.ld(Self::set_l,Self::l),
			0x6E => self.ld(Self::set_l,Self::hl_indirect8),
			0x6F => self.ld(Self::set_l,Self::a),
			0x70 => self.ld(Self::set_hl_indirect8,Self::b),
			0x71 => self.ld(Self::set_hl_indirect8,Self::c),
			0x72 => self.ld(Self::set_hl_indirect8,Self::d),
			0x73 => self.ld(Self::set_hl_indirect8,Self::e),
			0x74 => self.ld(Self::set_hl_indirect8,Self::h),
			0x75 => self.ld(Self::set_hl_indirect8,Self::l),
			0x76 => self.halt(),
			0x77 => self.ld(Self::set_hl_indirect8,Self::a),
			0x78 => self.ld(Self::set_a,Self::b),
			0x79 => self.ld(Self::set_a,Self::c),
			0x7A => self.ld(Self::set_a,Self::d),
			0x7B => self.ld(Self::set_a,Self::e),
			0x7C => self.ld(Self::set_a,Self::h),
			0x7D => self.ld(Self::set_a,Self::l),
			0x7E => self.ld(Self::set_a,Self::hl_indirect8),
			0x7F => self.ld(Self::set_a,Self::a),
			0x80 => self.add(Self::b),
			0x81 => self.add(Self::c),
			0x82 => self.add(Self::d),
			0x83 => self.add(Self::e),
			0x84 => self.add(Self::h),
			0x85 => self.add(Self::l),
			0x86 => self.add(Self::hl_indirect8),
			0x87 => self.add(Self::a),
			0x88 => self.adc(Self::b),
			0x89 => self.adc(Self::c),
			0x8A => self.adc(Self::d),
			0x8B => self.adc(Self::e),
			0x8C => self.adc(Self::h),
			0x8D => self.adc(Self::l),
			0x8E => self.adc(Self::hl_indirect8),
			0x8F => self.adc(Self::a),
			0x90 => self.sub(Self::b),
			0x91 => self.sub(Self::c),
			0x92 => self.sub(Self::d),
			0x93 => self.sub(Self::e),
			0x94 => self.sub(Self::h),
			0x95 => self.sub(Self::l),
			0x96 => self.sub(Self::hl_indirect8),
			0x97 => self.sub(Self::a),
			0x98 => self.sbc(Self::b),
			0x99 => self.sbc(Self::c),
			0x9A => self.sbc(Self::d),
			0x9B => self.sbc(Self::e),
			0x9C => self.sbc(Self::h),
			0x9D => self.sbc(Self::l),
			0x9E => self.sbc(Self::hl_indirect8),
			0x9F => self.sbc(Self::a),
			0xA0 => self.and(Self::b),
			0xA1 => self.and(Self::c),
			0xA2 => self.and(Self::d),
			0xA3 => self.and(Self::e),
			0xA4 => self.and(Self::h),
			0xA5 => self.and(Self::l),
			0xA6 => self.and(Self::hl_indirect8),
			0xA7 => self.and(Self::a),
			0xA8 => self.xor(Self::b),
			0xA9 => self.xor(Self::c),
			0xAA => self.xor(Self::d),
			0xAB => self.xor(Self::e),
			0xAC => self.xor(Self::h),
			0xAD => self.xor(Self::l),
			0xAE => self.xor(Self::hl_indirect8),
			0xAF => self.xor(Self::a),
			0xB0 => self.or(Self::b),
			0xB1 => self.or(Self::c),
			0xB2 => self.or(Self::d),
			0xB3 => self.or(Self::e),
			0xB4 => self.or(Self::h),
			0xB5 => self.or(Self::l),
			0xB6 => self.or(Self::hl_indirect8),
			0xB7 => self.or(Self::a),
			0xB8 => self.cp(Self::b),
			0xB9 => self.cp(Self::c),
			0xBA => self.cp(Self::d),
			0xBB => self.cp(Self::e),
			0xBC => self.cp(Self::h),
			0xBD => self.cp(Self::l),
			0xBE => self.cp(Self::hl_indirect8),
			0xBF => self.cp(Self::a),
			0xC0 => self.retc(Self::nz),
			0xC1 => self.pop(Self::set_bc),
			0xC2 => self.jpc(Self::nz,Self::immediate16),
			0xC3 => self.jp(Self::immediate16),
			0xC4 => self.callc(Self::nz,Self::immediate16),
			0xC5 => self.push(Self::bc),
			0xC6 => self.add(Self::immediate8),
			0xC7 => self.rst(0x00),
			0xC8 => self.retc(Self::z),
			0xC9 => self.ret(),
			0xCA => self.jpc(Self::z,Self::immediate16),
			0xCC => self.callc(Self::z,Self::immediate16),
			0xCD => self.call(Self::immediate16),
			0xCE => self.adc(Self::immediate8),
			0xCF => self.rst(0x08),
			0xD0 => self.retc(Self::ncarry),
			0xD1 => self.pop(Self::set_de),
			0xD2 => self.jpc(Self::ncarry,Self::immediate16),
			0xD4 => self.callc(Self::ncarry,Self::immediate16),
			0xD5 => self.push(Self::de),
			0xD6 => self.sub(Self::immediate8),
			0xD7 => self.rst(0x10),
			0xD8 => self.retc(Self::carry),
			0xD9 => self.reti(),
			0xDA => self.jpc(Self::carry,Self::immediate16),
			0xDC => self.callc(Self::carry,Self::immediate16),
			0xDE => self.sbc(Self::immediate8),
			0xDF => self.rst(0x18),
			0xE0 => self.ld(Self::set_indirect_abs8,Self::a),
			0xE1 => self.pop(Self::set_hl),
			0xE2 => self.ld(Self::set_c_indirect,Self::a),
			0xE5 => self.push(Self::hl),
			0xE6 => self.and(Self::immediate8),
			0xE7 => self.rst(0x20),
			0xE8 => self.addsp(Self::immediate8),
			0xE9 => self.jphl(),
			0xEA => self.ld(Self::set_indirect_abs8,Self::a),
			0xEE => self.xor(Self::immediate8),
			0xEF => self.rst(0x28),
			0xF0 => self.ld(Self::set_a,Self::indirect_abs8),
			0xF1 => self.pop(Self::set_af),
			0xF2 => self.ld(Self::set_a,Self::c_indirect),
			0xF3 => self.di(),
			0xF5 => self.push(Self::af),
			0xF6 => self.or(Self::immediate8),
			0xF7 => self.rst(0x30),
			0xF8 => self.ldsp(Self::set_hl, Self::immediate8),
			0xF9 => self.ldhl(),
			0xFA => self.ld(Self::set_a,Self::indirect_abs8),
			0xFB => self.ei(),
			0xFE => self.cp(Self::immediate8),
			0xFF => self.rst(0x38),
			_ => eprintln!("{:02X}: {} not reachable", instr.opcode, instr.name)
    }
  }

	fn execute_prefix(&mut self, instr: &Instruction) {
		let ops = &instr.operands;
		match instr.opcode {
			0x00 => self.rlc(Self::set_b,Self::b),
			0x01 => self.rlc(Self::set_c,Self::c),
			0x02 => self.rlc(Self::set_d,Self::d),
			0x03 => self.rlc(Self::set_e,Self::e),
			0x04 => self.rlc(Self::set_h,Self::h),
			0x05 => self.rlc(Self::set_l,Self::l),
			0x06 => self.rlc(Self::set_hl_indirect8,Self::hl_indirect8),
			0x07 => self.rlc(Self::set_a,Self::a),
			0x08 => self.rrc(Self::set_b,Self::b),
			0x09 => self.rrc(Self::set_c,Self::c),
			0x0A => self.rrc(Self::set_d,Self::d),
			0x0B => self.rrc(Self::set_e,Self::e),
			0x0C => self.rrc(Self::set_h,Self::h),
			0x0D => self.rrc(Self::set_l,Self::l),
			0x0E => self.rrc(Self::set_hl_indirect8,Self::hl_indirect8),
			0x0F => self.rrc(Self::set_a,Self::a),
			0x10 => self.rl(Self::set_b,Self::b),
			0x11 => self.rl(Self::set_c,Self::c),
			0x12 => self.rl(Self::set_d,Self::d),
			0x13 => self.rl(Self::set_e,Self::e),
			0x14 => self.rl(Self::set_h,Self::h),
			0x15 => self.rl(Self::set_l,Self::l),
			0x16 => self.rl(Self::set_hl_indirect8,Self::hl_indirect8),
			0x17 => self.rl(Self::set_a,Self::a),
			0x18 => self.rr(Self::set_b,Self::b),
			0x19 => self.rr(Self::set_c,Self::c),
			0x1A => self.rr(Self::set_d,Self::d),
			0x1B => self.rr(Self::set_e,Self::e),
			0x1C => self.rr(Self::set_h,Self::h),
			0x1D => self.rr(Self::set_l,Self::l),
			0x1E => self.rr(Self::set_hl_indirect8,Self::hl_indirect8),
			0x1F => self.rr(Self::set_a,Self::a),
			0x20 => self.sla(Self::set_b,Self::b),
			0x21 => self.sla(Self::set_c,Self::c),
			0x22 => self.sla(Self::set_d,Self::d),
			0x23 => self.sla(Self::set_e,Self::e),
			0x24 => self.sla(Self::set_h,Self::h),
			0x25 => self.sla(Self::set_l,Self::l),
			0x26 => self.sla(Self::set_hl_indirect8,Self::hl_indirect8),
			0x27 => self.sla(Self::set_a,Self::a),
			0x28 => self.sra(Self::set_b,Self::b),
			0x29 => self.sra(Self::set_c,Self::c),
			0x2A => self.sra(Self::set_d,Self::d),
			0x2B => self.sra(Self::set_e,Self::e),
			0x2C => self.sra(Self::set_h,Self::h),
			0x2D => self.sra(Self::set_l,Self::l),
			0x2E => self.sra(Self::set_hl_indirect8,Self::hl_indirect8),
			0x2F => self.sra(Self::set_a,Self::a),
			0x30 => self.swap(Self::set_b,Self::b),
			0x31 => self.swap(Self::set_c,Self::c),
			0x32 => self.swap(Self::set_d,Self::d),
			0x33 => self.swap(Self::set_e,Self::e),
			0x34 => self.swap(Self::set_h,Self::h),
			0x35 => self.swap(Self::set_l,Self::l),
			0x36 => self.swap(Self::set_hl_indirect8,Self::hl_indirect8),
			0x37 => self.swap(Self::set_a,Self::a),
			0x38 => self.srl(Self::set_b,Self::b),
			0x39 => self.srl(Self::set_c,Self::c),
			0x3A => self.srl(Self::set_d,Self::d),
			0x3B => self.srl(Self::set_e,Self::e),
			0x3C => self.srl(Self::set_h,Self::h),
			0x3D => self.srl(Self::set_l,Self::l),
			0x3E => self.srl(Self::set_hl_indirect8,Self::hl_indirect8),
			0x3F => self.srl(Self::set_a,Self::a),
			0x40 => self.bit(0,Self::b),
			0x41 => self.bit(0,Self::c),
			0x42 => self.bit(0,Self::d),
			0x43 => self.bit(0,Self::e),
			0x44 => self.bit(0,Self::h),
			0x45 => self.bit(0,Self::l),
			0x46 => self.bit(0,Self::hl_indirect8),
			0x47 => self.bit(0,Self::a),
			0x48 => self.bit(1,Self::b),
			0x49 => self.bit(1,Self::c),
			0x4A => self.bit(1,Self::d),
			0x4B => self.bit(1,Self::e),
			0x4C => self.bit(1,Self::h),
			0x4D => self.bit(1,Self::l),
			0x4E => self.bit(1,Self::hl_indirect8),
			0x4F => self.bit(1,Self::a),
			0x50 => self.bit(2,Self::b),
			0x51 => self.bit(2,Self::c),
			0x52 => self.bit(2,Self::d),
			0x53 => self.bit(2,Self::e),
			0x54 => self.bit(2,Self::h),
			0x55 => self.bit(2,Self::l),
			0x56 => self.bit(2,Self::hl_indirect8),
			0x57 => self.bit(2,Self::a),
			0x58 => self.bit(3,Self::b),
			0x59 => self.bit(3,Self::c),
			0x5A => self.bit(3,Self::d),
			0x5B => self.bit(3,Self::e),
			0x5C => self.bit(3,Self::h),
			0x5D => self.bit(3,Self::l),
			0x5E => self.bit(3,Self::hl_indirect8),
			0x5F => self.bit(3,Self::a),
			0x60 => self.bit(4,Self::b),
			0x61 => self.bit(4,Self::c),
			0x62 => self.bit(4,Self::d),
			0x63 => self.bit(4,Self::e),
			0x64 => self.bit(4,Self::h),
			0x65 => self.bit(4,Self::l),
			0x66 => self.bit(4,Self::hl_indirect8),
			0x67 => self.bit(4,Self::a),
			0x68 => self.bit(5,Self::b),
			0x69 => self.bit(5,Self::c),
			0x6A => self.bit(5,Self::d),
			0x6B => self.bit(5,Self::e),
			0x6C => self.bit(5,Self::h),
			0x6D => self.bit(5,Self::l),
			0x6E => self.bit(5,Self::hl_indirect8),
			0x6F => self.bit(5,Self::a),
			0x70 => self.bit(6,Self::b),
			0x71 => self.bit(6,Self::c),
			0x72 => self.bit(6,Self::d),
			0x73 => self.bit(6,Self::e),
			0x74 => self.bit(6,Self::h),
			0x75 => self.bit(6,Self::l),
			0x76 => self.bit(6,Self::hl_indirect8),
			0x77 => self.bit(6,Self::a),
			0x78 => self.bit(7,Self::b),
			0x79 => self.bit(7,Self::c),
			0x7A => self.bit(7,Self::d),
			0x7B => self.bit(7,Self::e),
			0x7C => self.bit(7,Self::h),
			0x7D => self.bit(7,Self::l),
			0x7E => self.bit(7,Self::hl_indirect8),
			0x7F => self.bit(7,Self::a),
			0x80 => self.res(0,Self::set_b,Self::b),
			0x81 => self.res(0,Self::set_c,Self::c),
			0x82 => self.res(0,Self::set_d,Self::d),
			0x83 => self.res(0,Self::set_e,Self::e),
			0x84 => self.res(0,Self::set_h,Self::h),
			0x85 => self.res(0,Self::set_l,Self::l),
			0x86 => self.res(0,Self::set_hl_indirect8,Self::hl_indirect8),
			0x87 => self.res(0,Self::set_a,Self::a),
			0x88 => self.res(1,Self::set_b,Self::b),
			0x89 => self.res(1,Self::set_c,Self::c),
			0x8A => self.res(1,Self::set_d,Self::d),
			0x8B => self.res(1,Self::set_e,Self::e),
			0x8C => self.res(1,Self::set_h,Self::h),
			0x8D => self.res(1,Self::set_l,Self::l),
			0x8E => self.res(1,Self::set_hl_indirect8,Self::hl_indirect8),
			0x8F => self.res(1,Self::set_a,Self::a),
			0x90 => self.res(2,Self::set_b,Self::b),
			0x91 => self.res(2,Self::set_c,Self::c),
			0x92 => self.res(2,Self::set_d,Self::d),
			0x93 => self.res(2,Self::set_e,Self::e),
			0x94 => self.res(2,Self::set_h,Self::h),
			0x95 => self.res(2,Self::set_l,Self::l),
			0x96 => self.res(2,Self::set_hl_indirect8,Self::hl_indirect8),
			0x97 => self.res(2,Self::set_a,Self::a),
			0x98 => self.res(3,Self::set_b,Self::b),
			0x99 => self.res(3,Self::set_c,Self::c),
			0x9A => self.res(3,Self::set_d,Self::d),
			0x9B => self.res(3,Self::set_e,Self::e),
			0x9C => self.res(3,Self::set_h,Self::h),
			0x9D => self.res(3,Self::set_l,Self::l),
			0x9E => self.res(3,Self::set_hl_indirect8,Self::hl_indirect8),
			0x9F => self.res(3,Self::set_a,Self::a),
			0xA0 => self.res(4,Self::set_b,Self::b),
			0xA1 => self.res(4,Self::set_c,Self::c),
			0xA2 => self.res(4,Self::set_d,Self::d),
			0xA3 => self.res(4,Self::set_e,Self::e),
			0xA4 => self.res(4,Self::set_h,Self::h),
			0xA5 => self.res(4,Self::set_l,Self::l),
			0xA6 => self.res(4,Self::set_hl_indirect8,Self::hl_indirect8),
			0xA7 => self.res(4,Self::set_a,Self::a),
			0xA8 => self.res(5,Self::set_b,Self::b),
			0xA9 => self.res(5,Self::set_c,Self::c),
			0xAA => self.res(5,Self::set_d,Self::d),
			0xAB => self.res(5,Self::set_e,Self::e),
			0xAC => self.res(5,Self::set_h,Self::h),
			0xAD => self.res(5,Self::set_l,Self::l),
			0xAE => self.res(5,Self::set_hl_indirect8,Self::hl_indirect8),
			0xAF => self.res(5,Self::set_a,Self::a),
			0xB0 => self.res(6,Self::set_b,Self::b),
			0xB1 => self.res(6,Self::set_c,Self::c),
			0xB2 => self.res(6,Self::set_d,Self::d),
			0xB3 => self.res(6,Self::set_e,Self::e),
			0xB4 => self.res(6,Self::set_h,Self::h),
			0xB5 => self.res(6,Self::set_l,Self::l),
			0xB6 => self.res(6,Self::set_hl_indirect8,Self::hl_indirect8),
			0xB7 => self.res(6,Self::set_a,Self::a),
			0xB8 => self.res(7,Self::set_b,Self::b),
			0xB9 => self.res(7,Self::set_c,Self::c),
			0xBA => self.res(7,Self::set_d,Self::d),
			0xBB => self.res(7,Self::set_e,Self::e),
			0xBC => self.res(7,Self::set_h,Self::h),
			0xBD => self.res(7,Self::set_l,Self::l),
			0xBE => self.res(7,Self::set_hl_indirect8,Self::hl_indirect8),
			0xBF => self.res(7,Self::set_a,Self::a),
			0xC0 => self.set(0,Self::set_b,Self::b),
			0xC1 => self.set(0,Self::set_c,Self::c),
			0xC2 => self.set(0,Self::set_d,Self::d),
			0xC3 => self.set(0,Self::set_e,Self::e),
			0xC4 => self.set(0,Self::set_h,Self::h),
			0xC5 => self.set(0,Self::set_l,Self::l),
			0xC6 => self.set(0,Self::set_hl_indirect8,Self::hl_indirect8),
			0xC7 => self.set(0,Self::set_a,Self::a),
			0xC8 => self.set(1,Self::set_b,Self::b),
			0xC9 => self.set(1,Self::set_c,Self::c),
			0xCA => self.set(1,Self::set_d,Self::d),
			0xCB => self.set(1,Self::set_e,Self::e),
			0xCC => self.set(1,Self::set_h,Self::h),
			0xCD => self.set(1,Self::set_l,Self::l),
			0xCE => self.set(1,Self::set_hl_indirect8,Self::hl_indirect8),
			0xCF => self.set(1,Self::set_a,Self::a),
			0xD0 => self.set(2,Self::set_b,Self::b),
			0xD1 => self.set(2,Self::set_c,Self::c),
			0xD2 => self.set(2,Self::set_d,Self::d),
			0xD3 => self.set(2,Self::set_e,Self::e),
			0xD4 => self.set(2,Self::set_h,Self::h),
			0xD5 => self.set(2,Self::set_l,Self::l),
			0xD6 => self.set(2,Self::set_hl_indirect8,Self::hl_indirect8),
			0xD7 => self.set(2,Self::set_a,Self::a),
			0xD8 => self.set(3,Self::set_b,Self::b),
			0xD9 => self.set(3,Self::set_c,Self::c),
			0xDA => self.set(3,Self::set_d,Self::d),
			0xDB => self.set(3,Self::set_e,Self::e),
			0xDC => self.set(3,Self::set_h,Self::h),
			0xDD => self.set(3,Self::set_l,Self::l),
			0xDE => self.set(3,Self::set_hl_indirect8,Self::hl_indirect8),
			0xDF => self.set(3,Self::set_a,Self::a),
			0xE0 => self.set(4,Self::set_b,Self::b),
			0xE1 => self.set(4,Self::set_c,Self::c),
			0xE2 => self.set(4,Self::set_d,Self::d),
			0xE3 => self.set(4,Self::set_e,Self::e),
			0xE4 => self.set(4,Self::set_h,Self::h),
			0xE5 => self.set(4,Self::set_l,Self::l),
			0xE6 => self.set(4,Self::set_hl_indirect8,Self::hl_indirect8),
			0xE7 => self.set(4,Self::set_a,Self::a),
			0xE8 => self.set(5,Self::set_b,Self::b),
			0xE9 => self.set(5,Self::set_c,Self::c),
			0xEA => self.set(5,Self::set_d,Self::d),
			0xEB => self.set(5,Self::set_e,Self::e),
			0xEC => self.set(5,Self::set_h,Self::h),
			0xED => self.set(5,Self::set_l,Self::l),
			0xEE => self.set(5,Self::set_hl_indirect8,Self::hl_indirect8),
			0xEF => self.set(5,Self::set_a,Self::a),
			0xF0 => self.set(6,Self::set_b,Self::b),
			0xF1 => self.set(6,Self::set_c,Self::c),
			0xF2 => self.set(6,Self::set_d,Self::d),
			0xF3 => self.set(6,Self::set_e,Self::e),
			0xF4 => self.set(6,Self::set_h,Self::h),
			0xF5 => self.set(6,Self::set_l,Self::l),
			0xF6 => self.set(6,Self::set_hl_indirect8,Self::hl_indirect8),
			0xF7 => self.set(6,Self::set_a,Self::a),
			0xF8 => self.set(7,Self::set_b,Self::b),
			0xF9 => self.set(7,Self::set_c,Self::c),
			0xFA => self.set(7,Self::set_d,Self::d),
			0xFB => self.set(7,Self::set_e,Self::e),
			0xFC => self.set(7,Self::set_h,Self::h),
			0xFD => self.set(7,Self::set_l,Self::l),
			0xFE => self.set(7,Self::set_hl_indirect8,Self::hl_indirect8),
			0xFF => self.set(7,Self::set_a,Self::a),
		}
	}
}
