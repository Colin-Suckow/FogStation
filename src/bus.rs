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

    pub fn read_word(&self, addr: u32) -> u32 {
        todo!()
    }

    pub fn write_word(&self, addr: u32, word: u32) {
        todo!()
    }
}