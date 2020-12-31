use crate::bios::Bios;
pub struct MainBus {
    bios: Bios,
}

impl MainBus {
    pub fn new(bios: Bios) -> MainBus {
        MainBus {
            bios
        }
    }
    // Only supports KSEG1
    pub fn read_word(&self, addr: u32) -> u32 {
        match addr {
            0xbfc0_0000..=0xbfc7_ffff => self.bios.read_word(addr - 0xbfc0_0000),
            _ => panic!("Invalid read at address {:#X}! This address is not mapped to any device", addr)
        }
    }

    pub fn write_word(&self, addr: u32, word: u32) {
        todo!()
    }
}