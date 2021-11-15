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
    voice0_volume: u32,
    current_mode: SpuMode,

    voice_registers: [u16; 192],

    transfer_address_register: u16,
    internal_transfer_address: u32,

    memory: [u8; 0x7FFFF],

    pending_irq_acked: bool,

}

impl SPU {
    pub fn new() -> Self {
        Self {
            main_volume: 0,
            reverb_volume: 0,
            spu_control: 0x8000, //Start with spu enabled
            voice0_volume: 0,
            current_mode: SpuMode::Stop,
            voice_registers: [0; 192],

            internal_transfer_address: 0,
            transfer_address_register: 0,

            memory: [0; 0x7FFFF],

            pending_irq_acked: true,
        }
    }

    pub fn read_half_word(&mut self, addr: u32) -> u16 {
        //println!("Reading spu {:#X}", addr);
        match addr {
            0x1F801C00 ..= 0x1F801D7F => {
                let addr = (addr - 0x1F801C00) / 2;
                self.voice_registers[addr as usize]
            },
            0x1F801DAE => self.status_register(),
            0x1F801DAA => {
                println!("{:#X}", self.spu_control);
                self.spu_control
            },
            0x1F801DAC => 0x4, //SPU transfer control
            0x1F801DA6 => self.transfer_address_register,
            _ => 0, //{println!("Read unknown SPU address {:#X}", addr); 0}
        }
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        //println!("Writing spu {:#X} v {:#X}", addr, value);
        match addr {
            0x1F801C00 ..= 0x1F801D7F => {
                let offset = (addr - 0x1F801C00) / 2;
                self.voice_registers[offset as usize] = value;
            },
            0x1F801D80 => self.main_volume = (value as u32) | (self.main_volume & 0xFFFF0000),
            0x1F801D82 => self.main_volume = ((value as u32) << 4) | (self.main_volume & 0xFFFF),
            0x1F801D84 => self.reverb_volume = (value as u32) | (self.reverb_volume & 0xFFFF0000),
            0x1F801D86 => {
                self.reverb_volume = ((value as u32) << 4) | (self.reverb_volume & 0xFFFF)
            }
            0x1F801DA8 => self.push_transfer_fifo(value), //SPU data transfer fifo
            0x1F801DAA => {
                self.spu_control = value;
                self.current_mode = match value.get_bits(4..5) {
                    0 => SpuMode::Stop,
                    1 => SpuMode::ManualWrite,
                    2 => SpuMode::DMAwrite,
                    3 => SpuMode::DMAread,
                    i => panic!("Unknown SPU mode {}", i)
                };
            },
            0x1F801C00 => self.voice0_volume = value as u32, //TODO implement real voice registers
            0x1F801DA6 => self.set_transfer_address(value),
            _ => (),//println!("Wrote unknown SPU address {:#X} with {:#X}", addr, value)
        }
    }

    fn set_transfer_address(&mut self, addr: u16) {
        self.internal_transfer_address = (addr as u32) * 8;
        self.transfer_address_register = addr;
    }

    fn push_transfer_fifo(&mut self, value: u16) {
        // TODO: Push data into spu memory. Right now we just increment the transfer address  for the SPU irq
        self.internal_transfer_address += 2;
    }

    fn check_irq(&self) {

    }

    fn pending_irq(&self) -> bool {
        !self.pending_irq_acked
    }


    fn status_register(&self) -> u16 {
        //println!("Reading spu stat. mode is {:?}", self.current_mode);
        //let mut result: u16 = 0;

        //result |= self.current_mode.clone() as u16;

        //result

        self.spu_control & 0x3F
    }
}
