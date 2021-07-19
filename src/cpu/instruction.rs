use bit_field::BitField;

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
    fn sign_extended(&self) -> u32;
    fn zero_extended(&self) -> u32;
}

impl NumberHelpers for u8 {
    fn sign_extended(&self) -> u32 {
        (self.clone() as i8) as u32
    }

    fn zero_extended(&self) -> u32 {
        self.clone() as u32
    }
}

impl NumberHelpers for u16 {
    fn sign_extended(&self) -> u32 {
        (self.clone() as i16) as u32
    }

    fn zero_extended(&self) -> u32 {
        self.clone() as u32
    }
}

impl NumberHelpers for u32 {
    fn sign_extended(&self) -> u32 {
        (self.clone() as i32) as u32
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
        (self.clone() & 0xFFFF) as i16 as u32
    }
}


enum Instruction {
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

fn decode_opcode(inst: u32) -> Option<Instruction> {
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
