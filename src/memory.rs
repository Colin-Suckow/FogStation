use byteorder::{ByteOrder, LittleEndian};

pub struct Memory {
    data: Vec<u8>
}

impl Memory {
    /// Initializes 2MiB of system memory
    pub fn new() -> Memory {
        Memory {
            data: vec![0; 16_000_000]
        }
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        LittleEndian::read_u32(&self.data[addr as usize..(addr + 4) as usize])
    }

    pub fn write_word(&mut self, addr: u32, word: u32) {
        if addr >= 0xB0 && addr < 0xC0 {
            println!("B function maybe have been written. Value was {}", word);
        }
        LittleEndian::write_u32(&mut self.data[addr as usize..(addr + 4) as usize], word);
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        LittleEndian::write_u16(&mut self.data[addr as usize..(addr + 2) as usize], value);
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.data[addr as usize]
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        self.data[addr as usize] = value;
    }
}