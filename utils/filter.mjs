import * as fs from 'fs'

let file = fs.readFileSync('./instr.json')
let json = JSON.parse(file.toString())

const targets = ["n8", "n16", "e8", "a8", "a16"]

let set = new Set();
for (let [opcode, instr] of Object.entries(json.unprefixed)) {
  if (targets.includes(instr.operands[0]?.name) || (instr.operands[0]?.name === "HL" && !instr.operands[0]?.immediate)) {
    set.add(instr.mnemonic)
  }
}

console.log(set)