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
        LittleEndian::write_u32(&mut self.data[addr as usize..(addr + 4) as usize], word);
    }
}