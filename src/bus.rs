use crate::bios::Bios;
use crate::gpu::Gpu;
use crate::memory::Memory;
use crate::dma::DMAState;
use crate::spu::SPU;

pub struct MainBus {
    pub bios: Bios,
    memory: Memory,
    pub gpu: Gpu,
    pub dma: DMAState,
    spu: SPU,
}

impl MainBus {
    pub fn new(bios: Bios, memory: Memory, gpu: Gpu) -> MainBus {
        MainBus { bios, memory, gpu, dma: DMAState::new(), spu: SPU::new() }
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x1F801070 => {
                println!("Tried to read i_status word");
                0
            },
            0x0..=0x001f_ffff => self.memory.read_word(addr), //KUSEG
            //0x8001_0000..=0x8001_f000 => self.bios.read_word(addr - 0x8001_0000), for test roms
            0x8000_0000..=0x801f_ffff => self.memory.read_word(addr - 0x8000_0000), //KSEG0
            0x1f801810 => self.gpu.read_word_gp0(),
            0x1f801814 => self.gpu.read_status_register(),
            0x1F801080..=0x1F8010F4 => self.dma.read_word(addr),
            0x1f80_1000..=0x1f80_2fff => {
                println!("Something tried to read the hardware control registers. These are not currently emulated, so a 0 is being returned. The address was {:#X}", addr);
                0
            }
            0xA000_0000..=0xA01f_ffff => self.memory.read_word(addr - 0xA000_0000), //KSEG1
            0xbfc0_0000..=0xbfc7_ffff => self.bios.read_word(addr - 0xbfc0_0000),
            _ => panic!(
                "Invalid word read at address {:#X}! This address is not mapped to any device.",
                addr
            ),
        }
    }

    pub fn write_word(&mut self, addr: u32, word: u32) {
        match addr {
            0x0..=0x001f_ffff => self.memory.write_word(addr, word), //KUSEG
            0x1F801074 => println!("IRQ mask write {:#b}", word),
            0x8000_0000..=0x801f_ffff => self.memory.write_word(addr - 0x8000_0000, word), //KSEG0
            0xA000_0000..=0xA01f_ffff => self.memory.write_word(addr - 0xA000_0000, word), //KSEG1
            0x1F801000 => println!("Expansion 1 base write"),
            0x1F801004 => println!("Expansion 2 base write"),
            0x1F801008 => println!("Expansion 1 delay/size write"),
            0x1F801010 => println!("BIOS ROM Control WORD write"),
            0x1F801060 => println!("RAM SIZE WORD write"),
            0x1F801020 => println!("COM_DELAY WORD write"),
            0x1F801014 => println!("SPU_DELAY size write"),
            0x1F801018 => println!("CDROM_DELAY size write"),
            0x1F80101C => println!("Expansion 2 delay/size write"),
            0x1F801080..=0x1F8010F4 => self.dma.write_word(addr, word),
            0x1F80100C => println!("Expansion 3 Delay/size write"),
            0x1F801810 => self.gpu.send_gp0_command(word),
            0x1F801814 => self.gpu.send_gp1_command(word),
            0x1f80_1000..=0x1f80_2fff => println!("Something tried to write to the hardware control registers. These are not currently emulated. The address was {:#X}. Value {:#X}", addr, word),
            0xbfc0_0000..=0xbfc7_ffff => {
                panic!("Something tried to write to the bios rom. This is not a valid action")
            }
            0xFFFE0000..=0xFFFE0200 => (), //println!("Something tried to write to the cache control registers. These are not currently emulated. The address was {:#X}", addr),
            _ => {
                panic!(
                    "Invalid word write at address {:#X}! This address is not mapped to any device.",
                    addr
                );
            }
        }
    }

    pub fn read_half_word(&mut self, addr: u32) -> u16 {
        match addr {
            0x1F801070 => {
                panic!("Tried to read i_status half");
            },
            0x8000_0000..=0x801f_ffff => self.memory.read_half_word(addr - 0x8000_0000), //KSEG0
            0x1F801C00..=0x1F801E80 => self.spu.read_half_word(addr),
            0x1f80_1000..=0x1f80_2fff => {
                println!("Something tried to half word read an undefined IO address. The address was {:#X}", addr);
                0
            },
            _ => panic!("Invalid half word read at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        match addr {
            0x0..=0x001f_ffff => self.memory.write_half_word(addr, value), //KUSEG
            0x8000_0000..=0x801f_ffff => self.memory.write_half_word(addr - 0x8000_0000, value), //KSEG0
            0x1F801C00..=0x1F801E80 => self.spu.write_half_word(addr, value),
            0x1F80_1000..=0x1F80_2000 => println!("Something tried to half word write to the I/O ports. This is not currently emulated. The address was {:#X}. value was {:#X}", addr, value),
            _ => panic!("Invalid half word write at address {:#X}! This address is not mapped to any device.", addr)
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x1F801070 => {
                println!("Tried to read i_status word");
                0
            },
            0x1F801074 => {
                println!("Tried to read i_mask byte");
                0
            },
            0x0..=0x001f_ffff => self.memory.read_byte(addr), //KUSEG
            0x1F00_0000..=0x1f00_FFFF => {
                //println!("Something tried to read the parallel port. This is not currently emulated, so a 0 was returned. The address was {:#X}", addr);
                0
            }
            0x8000_0000..=0x801f_ffff => self.memory.read_byte(addr - 0x8000_0000), //KSEG0
            0xbfc0_0000..=0xbfc7_ffff => self.bios.read_byte(addr - 0xbfc0_0000),
            _ => panic!(
                "Invalid byte read at address {:#X}! This address is not mapped to any device.",
                addr
            ),
        }
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x1F80_2000..=0x1F80_3000 => (), //println!("Something tried to write to the second expansion port. This is not currently emulated. The address was {:#X}", addr),
            0x0..=0x001f_ffff => self.memory.write_byte(addr, value), //KUSEG
            0x8000_0000..=0x801f_ffff => self.memory.write_byte(addr - 0x8000_0000, value), //KSEG0
            0xA000_0000..=0xA01f_ffff => self.memory.write_byte(addr - 0xA000_0000, value), //KSEG1
            _ => panic!(
                "Invalid byte write at address {:#X}! This address is not mapped to any device.",
                addr
            ),
        }
    }
}
