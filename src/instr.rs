use std::{collections::HashMap, sync::LazyLock};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Instruction {
  #[serde(skip)]
  pub opcode: u8,
  #[serde(alias = "mnemonic")]
  pub name: &'static str,
  pub bytes: usize,
  pub cycles: Vec<usize>,
  pub immediate: bool,
  #[serde(skip)]
  pub prefix: bool,
  pub operands: Vec<InstrTarget>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum TargetKind {
  #[serde(alias = "n8")]
  Immediate8,
  #[serde(alias = "n16")]
  Immediate16,
  #[serde(alias = "a8")]
  Address8,
  #[serde(alias = "a16")]
  Address16,
  #[serde(alias = "e8")]
  Signed8,
  A, B, C, D, E, F, H, L,
  AF, BC, DE, HL, SP,
  N, Z,
  NZ, NC, NH,

	#[serde(alias = "$00")]
  RST00,
	#[serde(alias = "$08")]
  RST08,
	#[serde(alias = "$10")]
  RST10,
	#[serde(alias = "$18")]
  RST18,
	#[serde(alias = "$20")]
  RST20,
	#[serde(alias = "$28")]
  RST28,
	#[serde(alias = "$30")]
  RST30,
	#[serde(alias = "$38")]
  RST38,

	#[serde(alias = "0")]
  Bit0,
	#[serde(alias = "1")]
  Bit1,
	#[serde(alias = "2")]
  Bit2,
	#[serde(alias = "3")]
  Bit3,
	#[serde(alias = "4")]
  Bit4,
	#[serde(alias = "5")]
  Bit5,
	#[serde(alias = "6")]
  Bit6,
	#[serde(alias = "7")]
  Bit7,
}

#[derive(Deserialize, Debug, Clone)]
pub struct InstrTarget {
  #[serde(alias = "name")]
  pub kind: TargetKind,
  pub immediate: bool,
  #[serde(default)]
  pub increment: bool,
  #[serde(default)]
  pub decrement: bool,
}

#[derive(Deserialize, Debug)]
struct InstrGroups {
  #[serde(borrow)]
  pub unprefixed: HashMap<&'static str, Instruction>,
  pub cbprefixed: HashMap<&'static str, Instruction>,
}

fn get_instructions() -> [Instruction; 256 * 2] {
	let json = include_str!("../utils/instr.json");
  let parsed: InstrGroups = serde_json
	  ::from_str(json)
	  .unwrap();
  
  let mut unprefixed = Vec::new();
  let mut cbprefixed = Vec::new();

  for (opcode_str, instr) in parsed.unprefixed {
    let opcode = u8
      ::from_str_radix(opcode_str.strip_prefix("0x").unwrap(), 16)
      .unwrap();

    let instr = Instruction {
      opcode,
      name: instr.name,
      bytes: instr.bytes,
      cycles: instr.cycles.clone(),
      immediate: instr.immediate,
      prefix: false,
      operands: instr.operands.clone(),
    };
    unprefixed.push(instr);
  }

  for (opcode_str, instr) in parsed.cbprefixed {
    let opcode = u8
      ::from_str_radix(opcode_str.strip_prefix("0x").unwrap(), 16)
      .unwrap();

    let instr = Instruction {
      opcode,
      name: instr.name,
      bytes: instr.bytes,
      cycles: instr.cycles.clone(),
      immediate: instr.immediate,
      prefix: true,
      operands: instr.operands.clone(),
    };

    cbprefixed.push(instr);
  }
  
  unprefixed.sort_by(|a, b| a.opcode.cmp(&b.opcode));
  cbprefixed.sort_by(|a, b| a.opcode.cmp(&b.opcode));

  unprefixed.append(&mut cbprefixed);
  unprefixed.try_into().unwrap()
}

pub static INSTRUCTIONS: LazyLock<[Instruction; 256*2]> = LazyLock::new(get_instructions);

#[cfg(test)]
mod instr_tests {
  use super::*;

  #[test]
  fn parse_test() {
	let flattened = get_instructions();

	println!("{:#?}", flattened);
  }
}