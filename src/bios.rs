use byteorder::{ByteOrder, LittleEndian};

pub struct Bios {
    data: Vec<u8>,
}

impl Bios {
    pub fn new(data: Vec<u8>) -> Bios {
        Bios { data }
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        LittleEndian::read_u32(&self.data[addr as usize..(addr + 4) as usize])
    }

    pub fn read_half_word(&self, addr: u32) -> u16 {
        LittleEndian::read_u16(&self.data[addr as usize..(addr + 2) as usize])
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.data[addr as usize]
    }

    pub fn get_data(&self) -> &Vec<u8> {
        &self.data
    }
}
