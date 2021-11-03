use bit_field::BitField;

use crate::cpu::Exception;

#[derive(Debug)]
pub struct Cop0 {
    gen_registers: [u32; 32],
}

impl Cop0 {
    pub fn new() -> Cop0 {
        let mut regs = [0; 32];
        regs[12] = 1; //Initially enable interrupts
        Cop0 {
            gen_registers: regs,
        }
    }

    /// Returns the value stored within the given register. Will panic if register_number > 31
    pub fn read_reg(&self, register_number: u8) -> u32 {
        self.gen_registers[register_number as usize]
    }

    /// Sets register to given value. Prevents setting R0, which should always be zero. Will panic if register_number > 31
    pub fn write_reg(&mut self, register_number: u8, value: u32) {
        self.gen_registers[register_number as usize] = value;
    }

    pub fn cache_isolated(&self) -> bool {
        ((self.gen_registers[12] >> 16) & 0x1) == 1
    }

    pub fn set_cause_execode(&mut self, exception: &Exception) {
        self.gen_registers[13] =
            (!((0x1F as u32) << 2) & self.gen_registers[13]) | ((exception.clone() as u32) << 2);
    }

    pub fn interrupts_enabled(&self) -> bool {
        self.gen_registers[12].get_bit(0)
    }

    pub fn interrupt_mask(&self) -> u8 {
        ((self.gen_registers[12] << 8) & 0xFF) as u8
    }
}

#[cfg(test)]
mod cop0_tests {
    use super::*;
    #[test]
    fn test_cache_isolated() {
        let mut cop0 = Cop0::new();
        cop0.write_reg(12, 65536);
        assert_eq!(cop0.cache_isolated(), true);

        cop0.write_reg(12, 0);
        assert_eq!(cop0.cache_isolated(), false);
    }
}
