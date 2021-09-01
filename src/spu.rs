use bit_field::BitField;

#[derive(Clone, Copy, Debug)]
enum SpuMode {
    Stop = 0,
    ManualWrite = 1,
    DMAwrite = 2,
    DMAread = 3,
}

pub struct SPU {
    main_volume: u32,
    reverb_volume: u32,
    spu_control: u16,
    spu_status: u16,
    voice0_volume: u32,
    current_mode: SpuMode,
    transfer_address: u16,
}

impl SPU {
    pub fn new() -> Self {
        Self {
            main_volume: 0,
            reverb_volume: 0,
            spu_control: 0x8000, //Start with spu enabled
            spu_status: 0,
            voice0_volume: 0,
            current_mode: SpuMode::Stop,
            transfer_address: 0,
        }
    }

    pub fn read_half_word(&mut self, addr: u32) -> u16 {
        //println!("Reading spu {:#X}", addr);
        match addr {
            0x1F801DAE => self.status_register(),
            0x1F801DAA => {
                //println!("{:#X}", self.spu_control);
                self.spu_control
            },
            0x1F801DAC => 0x4, //SPU transfer control
            0x1F801C00 => (self.voice0_volume & 0xFFFF) as u16,
            0x1F801DA6 => self.transfer_address,
            _ => {
                //println!("Read unknown SPU address {:#X}", addr); 
                0
            }
        }
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        //println!("Writing spu {:#X} v {:#X}", addr, value);
        match addr {
            0x1F801D80 => self.main_volume = (value as u32) | (self.main_volume & 0xFFFF0000),
            0x1F801D82 => self.main_volume = ((value as u32) << 4) | (self.main_volume & 0xFFFF),
            0x1F801D84 => self.reverb_volume = (value as u32) | (self.reverb_volume & 0xFFFF0000),
            0x1F801D86 => {
                self.reverb_volume = ((value as u32) << 4) | (self.reverb_volume & 0xFFFF)
            }
            0x1F801DA8 => (), //SPU data transfer fifo
            0x1F801DAA => {
                self.spu_control = value;
                self.current_mode = match value.get_bits(4..5) {
                    0 => SpuMode::Stop,
                    1 => SpuMode::ManualWrite,
                    2 => SpuMode::DMAwrite,
                    3 => SpuMode::DMAread,
                    i => panic!("Unknown SPU mode {}", i)
                };
                //println!("spu_cnt <- {:#X}", value);
            },
            0x1F801C00 => self.voice0_volume = value as u32, //TODO implement real voice registers
            0x1F801DA6 => self.transfer_address = value,
            _ => (), //println!("Wrote unknown SPU address {:#X} with {:#X}", addr, value)
        }
    }

    fn status_register(&self) -> u16 {
        //println!("Reading spu stat. mode is {:?}", self.current_mode);
        let mut result: u16 = 0;

        result |= self.current_mode.clone() as u16;

        result
    }
}
