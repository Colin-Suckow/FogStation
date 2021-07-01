use log::{error, info, warn};

use crate::bios::Bios;
use crate::cdrom::CDDrive;
use crate::controller::Controllers;
use crate::dma::DMAState;
use crate::gpu::Gpu;
use crate::memory::Memory;
use crate::spu::SPU;

pub struct MainBus {
    pub bios: Bios,
    pub memory: Memory,
    pub gpu: Gpu,
    pub dma: DMAState,
    spu: SPU,
    pub cd_drive: CDDrive,
    scratchpad: Memory,
    pub(super) controllers: Controllers,
}

impl MainBus {
    pub fn new(bios: Bios, memory: Memory, gpu: Gpu) -> MainBus {
        MainBus {
            bios,
            memory,
            gpu,
            dma: DMAState::new(),
            spu: SPU::new(),
            cd_drive: CDDrive::new(),
            scratchpad: Memory::new_scratchpad(),
            controllers: Controllers::new(),
        }
    }

    pub fn read_word(&mut self, og_addr: u32) -> u32 {
        let addr = og_addr & 0x1fffffff;
        let word = match addr {
            0x0..=0x001f_ffff => self.memory.read_word(addr),
            0x1f801810 => self.gpu.read_word_gp0(),
            0x1f801814 => self.gpu.read_status_register(),
            0x1F80101C => 0x00070777, //Expansion 2 delay/size
            0x1F801080..=0x1F8010F4 => self.dma.read_word(addr),
            0x1fc0_0000..=0x1fc7_ffff => self.bios.read_word(addr - 0x1fc0_0000),
            0x1F800000..=0x1F8003FF => self.scratchpad.read_word(addr - 0x1F800000),
            0x1F801014 => 0x200931E1, //SPU_DELAY
            0x1F801060 => 0x00000B88, //RAM_SIZE
            _ => panic!(
                "Invalid word read at address {:#X}! This address is not mapped to any device.",
                addr
            ),
        };
        //println!("Read {:#X} word from bus address {:#X}", word, addr);
        word
    }

    pub fn write_word(&mut self, og_addr: u32, word: u32) {
        let addr = og_addr & 0x1fffffff;
        //println!("Writing {:#X} to addr {:#X}", word, addr);
        if addr == 0xCAF50 {println!("Hit the thing {:#X}", word)};
        match addr & 0x1fffffff {
            0x1F802002 => info!("Serial: {}", word),
            0x1F802023 => info!("DUART A: {}", word),
            0x1F80202B => info!("DUART B: {}", word),
            0x1F801050 => info!("SIO: {}", word),
            0x0..=0x001f_ffff => self.memory.write_word(addr, word), //KUSEG
            0x1F801000 => info!("Expansion 1 base write"),
            0x1F801004 => info!("Expansion 2 base write"),
            0x1F801008 => info!("Expansion 1 delay/size write"),
            0x1F801010 => info!("BIOS ROM Control WORD write"),
            0x1F801060 => info!("RAM SIZE WORD write {:#X}", word),
            0x1F801020 => info!("COM_DELAY WORD write"),
            0x1F801014 => info!("SPU_DELAY size write"),
            0x1F801018 => info!("CDROM_DELAY size write"),
            0x1F80101C => info!("Expansion 2 delay/size write"),
            0x1F801080..=0x1F8010F4 => self.dma.write_word(addr, word),
            0x1F80100C => info!("Expansion 3 Delay/size write"),
            0x1F801810 => self.gpu.send_gp0_command(word),
            0x1F801814 => self.gpu.send_gp1_command(word),
            0x1F800000..=0x1F8003FF => self.scratchpad.write_word(addr - 0x1F800000, word),
            0x1f80_1000..=0x1f80_2fff => warn!("Something tried to write to the hardware control registers. These are not currently emulated. The address was {:#X}. Value {:#X}", addr, word),
            0x1FFE0000..=0x1FFE0200 => warn!("Something tried to write to the cache control registers. These are not currently emulated. The address was {:#X}", addr),
            _ => {
                panic!(
                    "Invalid word write at address {:#X}! This address is not mapped to any device.",
                    addr
                );
            }
        }
    }

    pub fn read_half_word(&mut self, og_addr: u32) -> u16 {
        let addr = og_addr & 0x1fffffff;


        match addr & 0x1fffffff {
            0x1F801070 => {
                panic!("Tried to read i_status half");
            },
            0x0..=0x001f_ffff => self.memory.read_half_word(addr),
            0x1F801C00..=0x1F801E80 => self.spu.read_half_word(addr),
            0x1F800000..=0x1F8003FF => self.scratchpad.read_half_word(addr - 0x1F800000),
            0x1F80_1040..=0x1F80_104E => self.controllers.read_half_word(addr),
            _ => panic!("Invalid half word read at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn write_half_word(&mut self, og_addr: u32, value: u16) {
        let addr = og_addr & 0x1fffffff;
        if addr == 0x7F10 {println!("Hit the thing half")};


        match addr & 0x1fffffff {
            0x1F802002 => info!("Serial: {}", value),
            0x1F802023 => info!("DUART A: {}", value),
            0x1F80202B => info!("DUART B: {}", value),
            0x1F801050 => info!("SIO: {}", value),
            0x0..=0x001f_ffff => self.memory.write_half_word(addr, value), //KUSEG
            0x1F801C00..=0x1F801E80 => self.spu.write_half_word(addr, value),
            0x1F800000..=0x1F8003FF => self.scratchpad.write_half_word(addr - 0x1F800000, value),
            0x1F80_1040..=0x1F80_104E => self.controllers.write_half_word(addr, value),
            0x1F80_1000..=0x1F80_2000 => warn!("Something tried to half word write to the I/O ports. This is not currently emulated. The address was {:#X}. value was {:#X}", addr, value),
            _ => panic!("Invalid half word write at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn read_byte(&mut self, og_addr: u32) -> u8 {
        let addr = og_addr & 0x1fffffff;
        match addr & 0x1fffffff {
            0x1F801070 => {
                warn!("Tried to read i_status word");
                0
            }
            0x1F801074 => {
                warn!("Tried to read i_mask byte");
                0
            }
            0x0..=0x001f_ffff => self.memory.read_byte(addr), //KUSEG
            0x1F00_0000..=0x1f00_FFFF => {
                //println!("Something tried to read the parallel port. This is not currently emulated, so a 0 was returned. The address was {:#X}", addr);
                0
            }
            0x1fc0_0000..=0x1fc7_ffff => self.bios.read_byte(addr - 0x1fc0_0000),
            0x1F801800..=0x1F801803 => self.cd_drive.read_byte(addr), //CDROM
            0x1F80_1040..=0x1F80_104E => self.controllers.read_byte(addr),
            0x1F800000..=0x1F8003FF => self.scratchpad.read_byte(addr - 0x1F800000),
            _ => {
                error!(
                    "Invalid byte read at address {:#X}! This address is not mapped to any device.",
                    addr
                );
                0
            }
        }
    }

    pub fn write_byte(&mut self, og_addr: u32, value: u8) {
        let addr = og_addr & 0x1fffffff;
        if addr == 0x7F10 {println!("Hit the thing byte")};

        match addr & 0x1fffffff {
            0x0..=0x001f_ffff => self.memory.write_byte(addr, value), //KUSEG
            0x1F801800..=0x1F801803 => self.cd_drive.write_byte(addr, value), //CDROM
            0x1F802002 => info!("Serial: {}", value),
            0x1F802023 => info!("DUART A: {}", value),
            0x1F80202B => info!("DUART B: {}", value),
            0x1F801050 => info!("SIO: {}", value),
            0x1F802000..=0x1F803000 => (), //Expansion port 2
            0x1F801040 => self.controllers.write_byte(addr, value),
            0x1F800000..=0x1F8003FF => self.scratchpad.write_byte(addr - 0x1F800000, value),
            _ => error!(
                "Invalid byte write at address {:#X}! This address is not mapped to any device.",
                addr
            ),
        }
    }
}
