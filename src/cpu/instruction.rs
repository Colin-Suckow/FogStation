use std::{convert::TryFrom, fmt::Display};

use bit_field::BitField;
use num_derive::FromPrimitive;    
use num_traits::FromPrimitive;

use super::R3000;

pub trait InstructionArgs {
    fn opcode(&self) -> u8;
    fn rs(&self) -> u8;
    fn rt(&self) -> u8;
    fn rd(&self) -> u8;
    fn shamt(&self) -> u8;
    fn funct(&self) -> u8;
    fn immediate(&self) -> u16;
    fn address(&self) -> u32;
    fn immediate_sign_extended(&self) -> u32;
}

pub trait NumberHelpers {
    fn sign_extended(&self) -> i32;
    fn zero_extended(&self) -> u32;
}

impl NumberHelpers for u8 {
    fn sign_extended(&self) -> i32 {
        (self.clone() as i8) as i32
    }

    fn zero_extended(&self) -> u32 {
        self.clone() as u32
    }
}

impl NumberHelpers for u16 {
    fn sign_extended(&self) -> i32 {
        (self.clone() as i16) as i32
    }

    fn zero_extended(&self) -> u32 {
        self.clone() as u32
    }
}

impl NumberHelpers for u32 {
    fn sign_extended(&self) -> i32 {
        (self.clone() as i32) as i32
    }

    fn zero_extended(&self) -> u32 {
        unimplemented!()
    }
}

impl InstructionArgs for u32 {
    fn opcode(&self) -> u8 {
        (self >> 26) as u8
    }

    fn rs(&self) -> u8 {
        ((self >> 21) & 0x1F) as u8
    }

    fn rt(&self) -> u8 {
        ((self >> 16) & 0x1F) as u8
    }

    fn rd(&self) -> u8 {
        ((self >> 11) & 0x1F) as u8
    }

    fn shamt(&self) -> u8 {
        ((self >> 6) & 0x1F) as u8
    }

    fn funct(&self) -> u8 {
        (self & 0x3F) as u8
    }

    fn immediate(&self) -> u16 {
        (self & 0xFFFF) as u16
    }

    fn address(&self) -> u32 {
        self & 0x3FFFFFF
    }

    fn immediate_sign_extended(&self) -> u32 {
        let val = (self.clone() & 0xFFFF) as i16 as i32 as u32;
        //println!("immse {:#X}", val);
        val
    }
}

#[derive(Debug)]
pub(super) enum Instruction {
    SLL{rt: u8, rd: u8, sa: u8},
    SRL{rt: u8, rd: u8, sa: u8},
    SRA{rt: u8, rd: u8, sa: u8},
    SLLV{rd: u8, rt: u8, rs: u8},
    SRLV{rd: u8, rt: u8, rs: u8},
    SRAV{rd: u8, rt: u8, rs: u8},
    JR{rs: u8},
    JALR{rd: u8, rs: u8},
    SYSCALL{code: u32},
    BREAK{code: u32},
    MFHI{rd: u8},
    MTHI{rs: u8},
    MFLO{rd: u8},
    MTLO{rs: u8},
    DIV{rs: u8, rt: u8},
    DIVU{rs: u8, rt: u8},
    ADD{rd: u8, rs: u8, rt: u8},
    SUB{rd: u8, rs: u8, rt: u8},
    SLTU{rd: u8, rs: u8, rt: u8},
    SUBU{rd: u8, rs: u8, rt: u8},
    AND{rd: u8, rs: u8, rt: u8},
    OR{rd: u8, rs: u8, rt: u8},
    XOR{rd: u8, rs: u8, rt: u8},
    NOR{rd: u8, rs: u8, rt: u8},
    ADDU{rd: u8, rs: u8, rt: u8},
    MULT{rs: u8, rt: u8},
    MULTU{rs: u8, rt: u8},
    SLT{rd: u8, rs: u8, rt: u8},
    BLTZ{rs: u8, offset: u16},
    BGEZ{rs: u8, offset: u16},
    BLTZAL{rs: u8, offset: u16},
    BGEZAL{rs: u8, offset: u16},
    J{target: u32},
    JAL{target: u32},
    BEQ{rs: u8, rt: u8, offset: u16},
    BNE{rs: u8, rt: u8, offset: u16},
    BLEZ{rs: u8, offset: u16},
    BGTZ{rs: u8, offset: u16},
    ADDI{rt: u8, rs: u8, immediate: u16},
    ADDIU{rt: u8, rs: u8, immediate: u16},
    SLTI{rt: u8, rs: u8, immediate: u16},
    SLTIU{rt: u8, rs: u8, immediate: u16},
    ANDI{rt: u8, rs: u8, immediate: u16},
    ORI{rt: u8, rs: u8, immediate: u16},
    XORI{rt: u8, rs: u8, immediate: u16},
    LUI{rt: u8, immediate: u16},
    MTC0{rt: u8, rd: u8},
    MFC0{rt: u8, rd: u8},
    RFE,
    MFC2{rt: u8, rd: u8},
    CTC2{rt: u8, rd: u8},
    MTC2{rt: u8, rd: u8},
    CFC2{rt: u8, rd: u8},
    IMM25{command: u32},
    LB{rt: u8, offset: u16, base: u8},
    LH{rt: u8, offset: u16, base: u8},
    LW{rt: u8, offset: u16, base: u8},
    LBU{rt: u8, offset: u16, base: u8},
    LHU{rt: u8, offset: u16, base: u8},
    SB{rt: u8, offset: u16, base: u8},
    SH{rt: u8, offset: u16, base: u8},
    LWL{rt: u8, offset: u16, base: u8},
    LWR{rt: u8, offset: u16, base: u8},
    SWL{rt: u8, offset: u16, base: u8},
    SWR{rt: u8, offset: u16, base: u8},
    SW{rt: u8, offset: u16, base: u8},
    LWC2{rt: u8, offset: u16, base: u8},
    SWC2{rt: u8, offset: u16, base: u8},
}

impl Instruction {

    #[allow(unused_variables)] // I should replace all these unused variables with underscores, but thats a lot of work
    pub fn mnemonic(&self) -> &str {
        match self {
            Instruction::SLL { rt, rd, sa } => "sll",
            Instruction::SRL { rt, rd, sa } => "srl",
            Instruction::SRA { rt, rd, sa } => "sra",
            Instruction::SLLV { rd, rt, rs } => "sllv",
            Instruction::SRLV { rd, rt, rs } => "srlv",
            Instruction::SRAV { rd, rt, rs } => "srav",
            Instruction::JR { rs } => "jr",
            Instruction::JALR { rd, rs } => "jalr",
            Instruction::SYSCALL { code } => "syscall",
            Instruction::BREAK { code } => "break",
            Instruction::MFHI { rd } => "mfhi",
            Instruction::MTHI { rs } => "mthi",
            Instruction::MFLO { rd } => "mflo",
            Instruction::MTLO { rs } => "mtlo",
            Instruction::DIV { rs, rt } => "div",
            Instruction::DIVU { rs, rt } => "divu",
            Instruction::ADD { rd, rs, rt } => "add",
            Instruction::SUB { rd, rs, rt } => "sub",
            Instruction::SLTU { rd, rs, rt } => "sltu",
            Instruction::SUBU { rd, rs, rt } => "subu",
            Instruction::AND { rd, rs, rt } => "and",
            Instruction::OR { rd, rs, rt } => "or",
            Instruction::XOR { rd, rs, rt } => "xor",
            Instruction::NOR { rd, rs, rt } => "nor",
            Instruction::ADDU { rd, rs, rt } => "addu",
            Instruction::MULT { rs, rt } => "mult",
            Instruction::MULTU { rs, rt } => "multu",
            Instruction::SLT { rd, rs, rt } => "slt",
            Instruction::BLTZ { rs, offset } => "bltz",
            Instruction::BGEZ { rs, offset } => "bgez",
            Instruction::BLTZAL { rs, offset } => "bltzal",
            Instruction::BGEZAL { rs, offset } => "bgezal",
            Instruction::J { target } => "j",
            Instruction::JAL { target } => "jal",
            Instruction::BEQ { rs, rt, offset } => "beq",
            Instruction::BNE { rs, rt, offset } => "bne",
            Instruction::BLEZ { rs, offset } => "blez",
            Instruction::BGTZ { rs, offset } => "bgtz",
            Instruction::ADDI { rt, rs, immediate } => "addi",
            Instruction::ADDIU { rt, rs, immediate } => "addiu",
            Instruction::SLTI { rt, rs, immediate } => "slti",
            Instruction::SLTIU { rt, rs, immediate } => "sltiu",
            Instruction::ANDI { rt, rs, immediate } => "andi",
            Instruction::ORI { rt, rs, immediate } => "ori",
            Instruction::XORI { rt, rs, immediate } => "xori",
            Instruction::LUI { rt, immediate } => "lui",
            Instruction::MTC0 { rt, rd } => "mtc0",
            Instruction::MFC0 { rt, rd } => "mfc0",
            Instruction::RFE => "rfe",
            Instruction::MFC2 { rt, rd } => "mfc2",
            Instruction::CTC2 { rt, rd } => "ctc2",
            Instruction::MTC2 { rt, rd } => "mtc2",
            Instruction::CFC2 { rt, rd } => "cfc2",
            Instruction::IMM25 { command } => "imm25",
            Instruction::LB { rt, offset, base } => "lb",
            Instruction::LH { rt, offset, base } => "lh",
            Instruction::LW { rt, offset, base } => "lw",
            Instruction::LBU { rt, offset, base } => "lbu",
            Instruction::LHU { rt, offset, base } => "lhu",
            Instruction::SB { rt, offset, base } => "sb",
            Instruction::SH { rt, offset, base } => "sh",
            Instruction::LWL { rt, offset, base } => "lwl",
            Instruction::LWR { rt, offset, base } => "lwr",
            Instruction::SWL { rt, offset, base } => "swl",
            Instruction::SWR { rt, offset, base } => "swr",
            Instruction::SW { rt, offset, base } => "sw",
            Instruction::LWC2 { rt, offset, base } => "lwc2",
            Instruction::SWC2 { rt, offset, base } => "swc2",
        }
    }

    #[allow(unused_variables)]
    pub fn arguments(&self, cpu: &R3000) -> String {
        match self {
            Instruction::SLL { rt, rd, sa } |
            Instruction::SRL { rt, rd, sa } |
            Instruction::SRA { rt, rd, sa } => format!("${}({:08x}), {:#x}", RegisterNames::from_u8(*rt).unwrap(), cpu.gen_registers[*rt as usize], sa),

            Instruction::JR { rs } =>  format!("${}({:08x}", RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize]),
            
            Instruction::JALR { rd, rs } => format!("${}({:08x}", RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize]),

            Instruction::SYSCALL { code } |
            Instruction::BREAK { code } => format!("{:#08x}", code),

            Instruction::MFHI { rd } => format!("${}({:08x}, $hi({:08x})", RegisterNames::from_u8(*rd).unwrap(), cpu.gen_registers[*rd as usize], cpu.hi),
            Instruction::MFLO { rd } => format!("${}({:08x}, $lo({:08x})", RegisterNames::from_u8(*rd).unwrap(), cpu.gen_registers[*rd as usize], cpu.lo),
            
            Instruction::MTHI { rs } => format!("$hi({:08x}), ${}({:08x}",  cpu.hi, RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize]),
            Instruction::MTLO { rs } => format!("$lo({:08x}), ${}({:08x}",  cpu.lo, RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize]),

            Instruction::DIV { rs, rt } |
            Instruction::DIVU { rs, rt } |
            Instruction::MULT { rs, rt } |
            Instruction::MULTU { rs, rt } => format!("${}({:08x}, ${}({:08x})", RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize], RegisterNames::from_u8(*rt).unwrap(), cpu.gen_registers[*rt as usize]),

            Instruction::SLLV { rd, rt, rs } |
            Instruction::SRLV { rd, rt, rs } |
            Instruction::SRAV { rd, rt, rs } |
            Instruction::ADD { rd, rs, rt } |
            Instruction::SUB { rd, rs, rt } |
            Instruction::SLTU { rd, rs, rt } |
            Instruction::SUBU { rd, rs, rt } |
            Instruction::AND { rd, rs, rt } |
            Instruction::OR { rd, rs, rt } |
            Instruction::XOR { rd, rs, rt } |
            Instruction::NOR { rd, rs, rt } |
            Instruction::ADDU { rd, rs, rt } |           
            Instruction::SLT { rd, rs, rt } => format!("${}({:08x}, ${}({:08x}, ${}({:08x})", RegisterNames::from_u8(*rd).unwrap(), cpu.gen_registers[*rd as usize], RegisterNames::from_u8(*rt).unwrap(), cpu.gen_registers[*rt as usize], RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize]),

            Instruction::BLTZ { rs, offset } |
            Instruction::BGEZ { rs, offset } |
            Instruction::BLTZAL { rs, offset } |
            Instruction::BLEZ { rs, offset } |
            Instruction::BGTZ { rs, offset } |
            Instruction::BGEZAL { rs, offset } => format!("${}({:08x}), {:#x}", RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize], offset),

            Instruction::J { target } |
            Instruction::JAL { target } => format!("{:#08x}", target),

            Instruction::BEQ { rs, rt, offset } |
            Instruction::BNE { rs, rt, offset } => format!("${}({:08x}, ${}({:08x}), {:#08x}", RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize], RegisterNames::from_u8(*rt).unwrap(), cpu.gen_registers[*rt as usize], offset),


            Instruction::ADDI { rt, rs, immediate } |
            Instruction::ADDIU { rt, rs, immediate } |
            Instruction::SLTI { rt, rs, immediate } |
            Instruction::SLTIU { rt, rs, immediate } |
            Instruction::ANDI { rt, rs, immediate } |
            Instruction::ORI { rt, rs, immediate } |
            Instruction::XORI { rt, rs, immediate } => format!("${}({:08x}, ${}({:08x}), {:#04x}", RegisterNames::from_u8(*rt).unwrap(), cpu.gen_registers[*rt as usize], RegisterNames::from_u8(*rs).unwrap(), cpu.gen_registers[*rs as usize], immediate),

            Instruction::LUI { rt, immediate } => format!("${}({:08x}, {:#04x}", RegisterNames::from_u8(*rt).unwrap(), cpu.gen_registers[*rt as usize], immediate),

            Instruction::RFE => "".to_string(),

            Instruction::MFC0 { rt, rd } |
            Instruction::MFC2 { rt, rd } |
            Instruction::CFC2 { rt, rd } => format!("${}({:08x}, ${}({:08x})", RegisterNames::from_u8(*rd).unwrap(), cpu.gen_registers[*rd as usize], rt, cpu.cop0.read_reg(*rt as u8)),

            Instruction::MTC0 { rt, rd } |
            Instruction::MTC2 { rt, rd } |
            Instruction::CTC2 { rt, rd } => format!("${}({:08x}, ${}({:08x})",rt, cpu.cop0.read_reg(*rt as u8), RegisterNames::from_u8(*rd).unwrap(), cpu.gen_registers[*rd as usize]),

            Instruction::IMM25 { command } => format!("{:08x}", command),

            Instruction::LB { rt, offset, base } |
            Instruction::LH { rt, offset, base } |
            Instruction::LW { rt, offset, base } |
            Instruction::LBU { rt, offset, base } |
            Instruction::LHU { rt, offset, base } |
            Instruction::SB { rt, offset, base } |
            Instruction::SH { rt, offset, base } |
            Instruction::LWL { rt, offset, base } |
            Instruction::LWR { rt, offset, base } |
            Instruction::SWL { rt, offset, base } |
            Instruction::SWR { rt, offset, base } |
            Instruction::SW { rt, offset, base } |
            Instruction::LWC2 { rt, offset, base } |
            Instruction::SWC2 { rt, offset, base } => format!("${}({:08x}), {:#04x}({})([{:08x}] = {:08x})", RegisterNames::from_u8(*rt).unwrap(), cpu.read_reg(*rt as u8), offset, RegisterNames::from_u8(*base).unwrap(), cpu.gen_registers[*base as usize] + *offset as u32, cpu.main_bus.peek_word((cpu.gen_registers[*base as usize] as i32 + (*offset  as i16)as i32) as u32)),
        }
    }

    pub fn execute(&self, cpu: &mut R3000) {
        match self {
            
        }
    }

}

pub(super) fn decode_opcode(inst: u32) -> Option<Instruction> {
    match inst.opcode() {
        0x0 => {
            //SPECIAL INSTRUCTIONS
            match inst.funct() {
                0x0 => Some(Instruction::SLL {rt: inst.rt(), rd: inst.rd(), sa: inst.shamt()}),
                0x2 => Some(Instruction::SRL {rt: inst.rt(), rd: inst.rd(), sa: inst.shamt()}),
                0x3 => Some(Instruction::SRA {rt: inst.rt(), rd: inst.rd(), sa: inst.shamt()}),
                0x4 => Some(Instruction::SLLV {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x6 => Some(Instruction::SRLV {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x7 => Some(Instruction::SRAV {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x8 => Some(Instruction::JR {rs: inst.rs()}),
                0x9 => Some(Instruction::JALR {rd: inst.rd(), rs: inst.rs()}),
                0xC => Some(Instruction::SYSCALL {code: (inst >> 5) & 0x1FFFFF}),
                0xD => Some(Instruction::BREAK {code: (inst >> 5) & 0x1FFFFF}),
                0x10 => Some(Instruction::MFHI {rd: inst.rd()}),
                0x11 => Some(Instruction::MTHI {rs: inst.rs()}),
                0x12 => Some(Instruction::MFLO {rd: inst.rd()}),
                0x13 => Some(Instruction::MTLO {rs: inst.rs()}),
                0x1A => Some(Instruction::DIV {rs: inst.rs(), rt: inst.rt()}),
                0x1B => Some(Instruction::DIVU {rs: inst.rs(), rt: inst.rt()}),
                0x20 => Some(Instruction::ADD {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x22 => Some(Instruction::SUB {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x2B => Some(Instruction::SLTU {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x23 => Some(Instruction::SUBU {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x24 => Some(Instruction::AND {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x25 => Some(Instruction::OR {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x26 => Some(Instruction::XOR {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x27 => Some(Instruction::NOR {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x21 => Some(Instruction::ADDU {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                0x18 => Some(Instruction::MULT {rt: inst.rt(), rs: inst.rs()}),
                0x19 => Some(Instruction::MULTU {rt: inst.rt(), rs: inst.rs()}),
                0x2A => Some(Instruction::SLT {rd: inst.rd(), rt: inst.rt(), rs: inst.rs()}),
                _ => None
            }
        }

        0x1 => {
            //"PC-relative" test and branch instructions
            match inst.rt() {
                0x0 => Some(Instruction::BLTZ{rs: inst.rs(), offset: inst.immediate()}),
                0x1 => Some(Instruction::BGEZ{rs: inst.rs(), offset: inst.immediate()}),
                0x10 => Some(Instruction::BLTZAL{rs: inst.rs(), offset: inst.immediate()}),
                0x11 => Some(Instruction::BGEZAL{rs: inst.rs(), offset: inst.immediate()}),
                _ => None
            }
        }

        0x2 => Some(Instruction::J{target: inst.address()}),
        0x3 => Some(Instruction::JAL{target: inst.address()}),
        0x4 => Some(Instruction::BEQ{rs: inst.rs(), rt: inst.rt(), offset: inst.immediate()}),
        0x5 => Some(Instruction::BNE{rs: inst.rs(), rt: inst.rt(), offset: inst.immediate()}),
        0x6 => Some(Instruction::BLEZ{rs: inst.rs(), offset: inst.immediate()}),
        0x7 => Some(Instruction::BGTZ{rs: inst.rs(), offset: inst.immediate()}),
        0x8 => Some(Instruction::ADDI{rt: inst.rt(), rs: inst.rs(), immediate: inst.immediate()}),
        0x9 => Some(Instruction::ADDIU{rt: inst.rt(), rs: inst.rs(), immediate: inst.immediate()}),
        0xA => Some(Instruction::SLTI{rt: inst.rt(), rs: inst.rs(), immediate: inst.immediate()}),
        0xB => Some(Instruction::SLTIU{rt: inst.rt(), rs: inst.rs(), immediate: inst.immediate()}),
        0xC => Some(Instruction::ANDI{rt: inst.rt(), rs: inst.rs(), immediate: inst.immediate()}),
        0xD => Some(Instruction::ORI{rt: inst.rt(), rs: inst.rs(), immediate: inst.immediate()}),
        0xE => Some(Instruction::XORI{rt: inst.rt(), rs: inst.rs(), immediate: inst.immediate()}),
        0xF => Some(Instruction::LUI{rt: inst.rt(), immediate: inst.immediate()}),
        0x10 => {
            //COP0 instructions
            match inst.rs() {
                0x4 => Some(Instruction::MTC0{rt: inst.rt(), rd: inst.rd()}),
                0x0 => Some(Instruction::MFC0{rt: inst.rt(), rd: inst.rd()}),

                0x10 => Some(Instruction::RFE),
                _ => None,
            }
        }

        0x12 => {
            //COP2 (GTE) instructions
            if inst.get_bit(25) {
                Some(Instruction::IMM25{command: inst & 0x1FFFFFF})
            } else {
                match inst.rs() {
                    0x0 => Some(Instruction::MFC2{rt: inst.rt(), rd: inst.rd()}),
                    0x6 => Some(Instruction::CTC2{rt: inst.rt(), rd: inst.rd()}),
                    0x4 => Some(Instruction::MTC2{rt: inst.rt(), rd: inst.rd()}),
                    0x2 => Some(Instruction::CFC2{rt: inst.rt(), rd: inst.rd()}),
                    _ => None
                }
            }
        }

        0x20 => Some(Instruction::LB{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x21 => Some(Instruction::LH{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x23 => Some(Instruction::LW{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x24 => Some(Instruction::LBU{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x25 => Some(Instruction::LHU{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x28 => Some(Instruction::SB{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x29 => Some(Instruction::SH{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x22 => Some(Instruction::LWL{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x26 => Some(Instruction::LWR{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x2A => Some(Instruction::SWL{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x2E => Some(Instruction::SWR{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x2B => Some(Instruction::SW{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x32 => Some(Instruction::LWC2{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        0x3A => Some(Instruction::SWC2{rt: inst.rt(), offset: inst.immediate(), base: inst.rs()}),
        _ => None,
    }

}

#[derive(FromPrimitive)]
#[allow(non_camel_case_types)]
pub enum RegisterNames {
    zero = 0,
    at = 1,
    v0 = 2,
    v1 = 3,
    a0 = 4,
    a1 = 5,
    a2 = 6,
    a3 = 7,
    t0 = 8,
    t1 = 9,
    t2 = 10,
    t3 = 11,
    t4 = 12,
    t5 = 13,
    t6 = 14,
    t7 = 15,
    s0 = 16,
    s1 = 17,
    s2 = 18,
    s3 = 19,
    s4 = 20,
    s5 = 21,
    s6 = 22,
    s7 = 23,
    t8 = 24,
    t9 = 25,
    k0 = 26,
    k1 = 27,
    gp = 28,
    sp = 29,
    fp = 30,
    ra = 31,

}

impl Display for RegisterNames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterNames::zero => write!(f, "ze"),
            RegisterNames::at => write!(f,"at"),
            RegisterNames::v0 => write!(f,"v0"),
            RegisterNames::v1 => write!(f,"v1"),
            RegisterNames::a0 => write!(f,"a0"),
            RegisterNames::a1 => write!(f,"a1"),
            RegisterNames::a2 => write!(f,"a2"),
            RegisterNames::a3 => write!(f,"a3"),
            RegisterNames::t0 => write!(f,"t0"),
            RegisterNames::t1 => write!(f,"t1"),
            RegisterNames::t2 => write!(f,"t2"),
            RegisterNames::t3 => write!(f,"t3"),
            RegisterNames::t4 => write!(f,"t4"),
            RegisterNames::t5 => write!(f,"t5"),
            RegisterNames::t6 => write!(f,"t6"),
            RegisterNames::t7 => write!(f,"t7"),
            RegisterNames::s0 => write!(f,"s0"),
            RegisterNames::s1 => write!(f,"s1"),
            RegisterNames::s2 => write!(f,"s2"),
            RegisterNames::s3 => write!(f,"s3"),
            RegisterNames::s4 => write!(f,"s4"),
            RegisterNames::s5 => write!(f,"s5"),
            RegisterNames::s6 => write!(f,"s6"),
            RegisterNames::s7 => write!(f,"s7"),
            RegisterNames::t8 => write!(f,"t8"),
            RegisterNames::t9 => write!(f,"t9"),
            RegisterNames::k0 => write!(f,"k0"),
            RegisterNames::k1 => write!(f,"k1"),
            RegisterNames::gp => write!(f,"gp"),
            RegisterNames::sp => write!(f,"sp"),
            RegisterNames::fp => write!(f,"fp"),
            RegisterNames::ra => write!(f,"ra"),
        }
    }
}

impl TryFrom<usize> for RegisterNames {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            x if x == 0 => Ok(RegisterNames::zero),
            x if x == 1 => Ok(RegisterNames::at),
            x if x == 2 => Ok(RegisterNames::v0),
            x if x == 3 => Ok(RegisterNames::v1),
            x if x == 4 => Ok(RegisterNames::a0),
            x if x == 5 => Ok(RegisterNames::a1),
            x if x == 6 => Ok(RegisterNames::a2),
            x if x == 7 => Ok(RegisterNames::a3),
            x if x == 8 => Ok(RegisterNames::t0),
            x if x == 9 => Ok(RegisterNames::t1),
            x if x == 10 => Ok(RegisterNames::t2),
            x if x == 11 => Ok(RegisterNames::t3),
            x if x == 12 => Ok(RegisterNames::t4),
            x if x == 13 => Ok(RegisterNames::t5),
            x if x == 14 => Ok(RegisterNames::t6),
            x if x == 15 => Ok(RegisterNames::t7),
            x if x == 16 => Ok(RegisterNames::s0),
            x if x == 17 => Ok(RegisterNames::s1),
            x if x == 18 => Ok(RegisterNames::s2),
            x if x == 19 => Ok(RegisterNames::s3),
            x if x == 20 => Ok(RegisterNames::s4),
            x if x == 21 => Ok(RegisterNames::s5),
            x if x == 22 => Ok(RegisterNames::s6),
            x if x == 23 => Ok(RegisterNames::s7),
            x if x == 24 => Ok(RegisterNames::t8),
            x if x == 25 => Ok(RegisterNames::t9),
            x if x == 26 => Ok(RegisterNames::k0),
            x if x == 27 => Ok(RegisterNames::k1),
            x if x == 28 => Ok(RegisterNames::gp),
            x if x == 29 => Ok(RegisterNames::sp),
            x if x == 30 => Ok(RegisterNames::fp),
            x if x == 31 => Ok(RegisterNames::ra),
            _ => Err(()),
        }
    }

    
}



#[cfg(test)]
mod instruction_tests {
    use super::InstructionArgs;
    #[test]
    fn test_opcode() {
        let test: u32 = 0b11111100000000000000000000000000;
        assert_eq!(test.opcode(), 0b00111111);
    }

    #[test]
    fn test_rs() {
        let test: u32 = 0b11111000000000000000000000;
        assert_eq!(test.rs(), 0b00011111);
    }

    #[test]
    fn test_rt() {
        let test: u32 = 0b111110000000000000000;
        assert_eq!(test.rt(), 0b00011111);
    }

    #[test]
    fn test_rd() {
        let test: u32 = 0b1111100000000000;
        assert_eq!(test.rd(), 0b00011111);
    }

    #[test]
    fn test_shamt() {
        let test: u32 = 0b11111000000;
        assert_eq!(test.shamt(), 0b00011111);
    }

    #[test]
    fn test_funct() {
        let test: u32 = 0b11111;
        assert_eq!(test.funct(), 0b00011111);
    }

    #[test]
    fn test_immediate() {
        let test: u32 = 0xFFFFFF;
        assert_eq!(test.immediate(), 0xFFFF);
    }

    #[test]
    fn test_address() {
        let test: u32 = 0xFFFFFFF;
        assert_eq!(test.address(), 0x3FFFFFF);
    }
}
