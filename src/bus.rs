use crate::bios::Bios;
use crate::memory::Memory;
pub struct MainBus {
    bios: Bios,
    memory: Memory,
}

impl MainBus {
    pub fn new(bios: Bios, memory: Memory) -> MainBus {
        MainBus {
            bios,
            memory
        }
    }
    // Only supports KSEG1
    pub fn read_word(&self, addr: u32) -> u32 {
        match addr {
            0x0001_0000..=0x001f_ffff => self.memory.read_word(addr - 0x0001_0000),
            0xbfc0_0000..=0xbfc7_ffff => self.bios.read_word(addr - 0xbfc0_0000),
            _ => panic!("Invalid read at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn write_word(&mut self, addr: u32, word: u32) {
        match addr {
            0x0001_0000..=0x001f_ffff => self.memory.write_word(addr - 0x0001_0000, word),
            0xbfc0_0000..=0xbfc7_ffff => panic!("Something tried to read the bios rom. This is not a valid action"),
            _ => panic!("Invalid write at address {:#X}! This address is not mapped to any device.", addr)
        }
    }
}