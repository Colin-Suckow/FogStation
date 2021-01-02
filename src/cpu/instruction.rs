pub trait Instruction {
    fn opcode(&self) -> u8;
    fn rs(&self) -> u8;
    fn rt(&self) -> u8;
    fn rd(&self) -> u8;
    fn shamt(&self) -> u8;
    fn funct(&self) -> u8;
    fn immediate(&self) -> u16;
    fn address(&self) -> u32;
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

impl Instruction for u32 {
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
}



#[cfg(test)]
mod instruction_tests {
    use super::Instruction;
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
