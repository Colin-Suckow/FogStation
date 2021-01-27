pub struct SPU {
    main_volume: u32,
    reverb_volume: u32,
    spu_control: u16,
    spu_status: u16,
    voice0_volume: u32,
}

impl SPU {
    pub fn new() -> Self {
        Self {
            main_volume: 0,
            reverb_volume: 0,
            spu_control: 0x8000, //Start with spu enabled
            spu_status: 0,
            voice0_volume: 0,
        }
    }

    pub fn read_half_word(&mut self, addr: u32) -> u16 {
        match addr {
            0x1F801DAE => self.spu_status,
            0x1F801DAA => self.spu_control,
            0x1F801DAC => 0x4, //SPU transfer control
            0x1F801C00 => (self.voice0_volume & 0xFFFF) as u16,
            _ => 0//{println!("Read unknown SPU address {:#X}", addr); 0}
        }
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        match addr {
            0x1F801D80 => self.main_volume = (value as u32) | (self.main_volume & 0xFFFF0000),
            0x1F801D82 => self.main_volume = ((value as u32) << 4) | (self.main_volume & 0xFFFF),
            0x1F801D84 => self.reverb_volume = (value as u32) | (self.reverb_volume & 0xFFFF0000),
            0x1F801D86 => self.reverb_volume = ((value as u32) << 4) | (self.reverb_volume & 0xFFFF),
            0x1F801DA6 => (), //SPU data transfer address
            0x1F801DA8 => (), //SPU data transfer fifo
            0x1F801DAA => self.spu_control = value,
            0x1F801C00 => self.voice0_volume = value as u32, //TODO implement real voice registers
            _ => ()//println!("Wrote unknown SPU address {:#X} with {:#X}", addr, value)
        }
    }
}