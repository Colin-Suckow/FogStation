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

    pub fn read_word(&self, addr: u32) -> u32 {
        match addr {
            0x0..=0x001f_ffff => self.memory.read_word(addr), //KUSEG
            0x8000_0000..=0x801f_ffff => self.memory.read_word(addr - 0x8000_0000), //KSEG0
            0xA000_0000..=0xA01f_ffff => self.memory.read_word(addr - 0xA000_0000), //KSEG1
            0xbfc0_0000..=0xbfc7_ffff => self.bios.read_word(addr - 0xbfc0_0000),
            _ => panic!("Invalid word read at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn write_word(&mut self, addr: u32, word: u32) {
        match addr {
            0x0..=0x001f_ffff => self.memory.write_word(addr, word), //KUSEG
            0x8000_0000..=0x801f_ffff => self.memory.write_word(addr - 0x8000_0000, word), //KSEG0
            0xA000_0000..=0xA01f_ffff => self.memory.write_word(addr - 0xA000_0000, word), //KSEG1
            0x1f80_1000..=0x1f80_2fff => println!("Something tried to write to the hardware control registers. These are not currently emulated. The address was {:#X}", addr),
            0xbfc0_0000..=0xbfc7_ffff => panic!("Something tried to write to the bios rom. This is not a valid action"),
            0xFFFE0000..=0xFFFE0200 => println!("Something tried to write to the cache control registers. These are not currently emulated. The address was {:#X}", addr),
            _ => panic!("Invalid word write at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn read_half_word(&self, addr: u32) -> u16 {
        match addr {
            _ => panic!("Invalid half word read at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        match addr {
            0x0..=0x001f_ffff => self.memory.write_half_word(addr, value), //KUSEG
            0x1F80_1000..=0x1F80_2000 => println!("Something tried to write to the I/O ports. This is not currently emulated. The address was {:#X}", addr),
            _ => panic!("Invalid half word write at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x1F00_0000..=0x1f00_FFFF => {
                println!("Something tried to read the parallel port. This is not currently emulated, so a 0 was returned. The address was {:#X}", addr);
                0
            },
            0x8000_0000..=0x801f_ffff => self.memory.read_byte(addr - 0x8000_0000), //KSEG0
            0xbfc0_0000..=0xbfc7_ffff => self.bios.read_byte(addr - 0xbfc0_0000),
            _ => panic!("Invalid byte read at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x1F80_2000..=0x1F80_3000 => println!("Something tried to write to the second expansion port. This is not currently emulated. The address was {:#X}", addr),
            0x0..=0x001f_ffff => self.memory.write_byte(addr, value), //KUSEG
            0x8000_0000..=0x801f_ffff => self.memory.write_byte(addr - 0x8000_0000, value), //KSEG0
            _ => panic!("Invalid byte write at address {:#X}! This address is not mapped to any device.", addr)
        }
    }
}