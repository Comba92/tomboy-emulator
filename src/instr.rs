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
  A, B, C, D, E, F, H, L, N, Z,
  AF, BC, DE, HL, SP, NZ, NC, NH,
  #[serde(
	alias = "$00", 
	alias = "$08", 
	alias = "$10",
	alias = "$18",
	alias = "$20",
	alias = "$28",
	alias = "$30",
	alias = "$38",
  )]
  RST,
  #[serde(
	alias = "0",
	alias = "1",
	alias = "2",
	alias = "3",
	alias = "4",
	alias = "5",
	alias = "6",
	alias = "7",
  )]
  Value
}

#[derive(Deserialize, Debug, Clone)]
pub struct InstrTarget {
  #[serde(alias = "name")]
  pub kind: TargetKind,
  pub immediate: bool,
  pub increment: bool,
  pub decrement: bool,
}

#[derive(Deserialize, Debug)]
struct InstrGroups {
  #[serde(borrow)]
  pub unprefixed: HashMap<&'static str, Instruction>,
  pub cbprefixed: HashMap<&'static str, Instruction>,
}

fn get_instructions() -> [Instruction; 256 * 2] {
	let json = include_str!("instr.json");
  let parsed: InstrGroups = serde_json
	::from_str(json)
	.unwrap();
  
  let mut flattened = Vec::new();
  let chained_iter = parsed.unprefixed.iter()
	.chain(parsed.cbprefixed.iter());

  for (opcode_str, instr) in chained_iter {
	let opcode = u8
	  ::from_str_radix(opcode_str.strip_prefix("0x").unwrap(), 16)
	  .unwrap();

	let instr = Instruction {
	  opcode,
	  name: instr.name,
	  bytes: instr.bytes,
	  cycles: instr.cycles.clone(),
	  immediate: instr.immediate,
	  prefix: parsed.cbprefixed.contains_key(opcode_str),
	  operands: instr.operands.clone(),
	};

	flattened.push(instr);
  }

  flattened.try_into().unwrap()
}

pub static INSTRUCTIONS: LazyLock<[Instruction; 256*2]> = LazyLock::new(get_instructions);

#[cfg(test)]
mod instr_tests {
  use super::*;

  #[test]
  fn parse_test() {
	let flattened = get_instructions();

	println!("{:?}", flattened);
  }
}