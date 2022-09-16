use bit_field::BitField;
use byteorder::{ByteOrder, LittleEndian};

#[derive(Clone, Copy, Debug)]
enum SpuMode {
    Stop = 0,
    ManualWrite = 1,
    DMAwrite = 2,
    DMAread = 3,
}

enum DeltaMode {
    Linear,
    Exponential
}

enum DeltaDirection {
    Increase,
    Decrease
}

struct Voice {
    attack_mode: DeltaMode,
    attack_shift: u8,
    attack_step: u8,
    decay_shift: u8,
    sustain_level: u8,
    sustain_mode: DeltaMode,
    sustain_direction: DeltaDirection,
    sustain_shift: u8,
    sustain_step: u8,
    release_mode: DeltaMode,
    release_shift: u8,

    start_address: u16,
    current_address: u16,

}

pub struct SPU {
    main_volume: u32,
    reverb_volume: u32,
    spu_control: u16,
    voice0_volume: u32,
    current_mode: SpuMode,

    voice_registers: Vec<u8>,

    transfer_address_register: u16,
    internal_transfer_address: u32,

    memory: Vec<u8>,
    irq_addr: u32,
    pending_irq_acked: bool,

    cycle_count: usize,
}

impl SPU {
    pub fn new() -> Self {
        Self {
            main_volume: 0,
            reverb_volume: 0,
            spu_control: 0x8000, //Start with spu enabled
            voice0_volume: 0,
            current_mode: SpuMode::Stop,
            voice_registers: vec![0; 608],

            internal_transfer_address: 0,
            transfer_address_register: 0,
            irq_addr: 1,

            memory: vec![0; 0x800000],

            pending_irq_acked: true,


            cycle_count: 0,
        }
    }

    pub fn read_half_word(&mut self, addr: u32) -> u16 {
        
        let val  = match addr {
            0x1F801DAE => self.status_register(),
            0x1F801DAA => self.spu_control,
            0x1F801DAC => 0x4, //SPU transfer control
            0x1F801DA6 => self.transfer_address_register,
            0x1F801C00 ..= 0x1F801E5F => {
                let offset = addr - 0x1F801C00;
                LittleEndian::read_u16(&self.voice_registers[offset as usize..(offset + 2) as usize])
            },
            _ => 0, //{println!("Read unknown SPU address {:#X}", addr); 0}
        };
        //println!("Reading spu {:#X}  val {:#X}", addr, val);
        val
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        //println!("Writing spu {:#X} v {:#X}", addr, value);
        match addr {
            0x1F801DA4 => self.irq_addr = value as u32,
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
            0x1F801DA6 => self.set_transfer_address(value),

            0x1F801C00 ..= 0x1F801E5F => {
                let offset = addr - 0x1F801C00;
                LittleEndian::write_u16(&mut self.voice_registers[offset as usize..(offset + 2) as usize], value);
            },
            _ => println!("Wrote unknown SPU address {:#X} with {:#X}", addr, value)
        }
    }

    fn set_transfer_address(&mut self, addr: u16) {
        self.internal_transfer_address = (addr << 3) as u32;
        self.transfer_address_register = addr;
    }

    fn push_transfer_fifo(&mut self, value: u16) {
        //println!("SPU FIFO pushing value: {:#X} to addr {:#X}", value, self.internal_transfer_address);
        LittleEndian::write_u16(&mut self.memory[self.internal_transfer_address as usize..(self.internal_transfer_address + 2) as usize], value);
        self.internal_transfer_address += 2;
        if self.check_irq() {
            self.queue_irq();
        }
    }

    fn queue_irq(&mut self) {
        self.pending_irq_acked = false;
    }

    fn check_irq(&self) -> bool {
        //println!("addr {:#X} irq addr {:#X}", self.internal_transfer_address, self.irq_addr << 3);
        self.internal_transfer_address == self.irq_addr << 3
    }

    pub fn check_and_ack_irq(&mut self) -> bool {
        self.cycle_count += 1;

        if self.cycle_count % (340_220 / 2) == 0 && self.spu_control.get_bit(15) {
            self.queue_irq();
        }

        let result = !self.pending_irq_acked;
        self.pending_irq_acked = true;
        result
    }

    fn status_register(&self) -> u16 {
        //println!("Reading spu stat. mode is {:?}", self.current_mode);
        //let mut result: u16 = 0;

        //result |= self.current_mode.clone() as u16;

        //result

        self.spu_control & 0x3F
    }
}
