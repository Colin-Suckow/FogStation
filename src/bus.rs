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
            0x0..=0x001f_ffff => self.memory.read_word(addr), //KUSEG
            0x8000_0000..=0x801f_ffff => self.memory.read_word(addr - 0x8000_0000), //KSEG1
            0xbfc0_0000..=0xbfc7_ffff => self.bios.read_word(addr - 0xbfc0_0000),
            _ => panic!("Invalid read at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn write_word(&mut self, addr: u32, word: u32) {
        match addr {
            0x0..=0x001f_ffff => self.memory.write_word(addr, word), //KUSEG
            0x8000_0000..=0x801f_ffff => self.memory.write_word(addr - 0x8000_0000, word), //KSEG1
            0x1f80_1000..=0x1f80_2fff => println!("Something tried to write to the hardware control registers. These are not currently emulated. The address was {:#X}", addr),
            0xbfc0_0000..=0xbfc7_ffff => panic!("Something tried to write to the bios rom. This is not a valid action"),
            0xFFFE0000..=0xFFFE0200 => println!("Something tried to write to the cache control registers. These are not currently emulated. The address was {:#X}", addr),
            _ => panic!("Invalid write at address {:#X}! This address is not mapped to any device.", addr)
        }
    }
}