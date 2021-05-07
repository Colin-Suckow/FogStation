use crate::cpu::{InterruptSource, R3000};
use bit_field::BitField;

const NUM_CHANNELS: usize = 7;

#[derive(Clone)]
struct Channel {
    base_addr: u32,
    block: u32,
    control: u32,
}

impl Channel {
    fn new() -> Self {
        Self {
            base_addr: 0,
            block: 0,
            control: 0x800000,
        }
    }

    fn enabled(&self) -> bool {
        self.control.get_bit(24)
    }

    fn complete(&mut self) {
        self.control.set_bit(24, false);
        self.control.set_bit(28, false);
    }
}

pub struct DMAState {
    channels: Vec<Channel>,
    control: u32,
    interrupt: u32,
}

impl DMAState {
    pub fn new() -> Self {
        Self {
            channels: vec![Channel::new(); NUM_CHANNELS],
            control: 0x07654321, //Initial value on reset
            interrupt: 0,
        }
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        let channel_num = (((addr & 0x000000F0) >> 4) - 0x8) as usize;
        match addr {
            0x1F8010F0 => self.control,
            0x1F8010F4 => {
                //println!("Reading DMA interrupt. val {:#X}", self.interrupt);
                self.interrupt
            }
            _ => {
                match addr & 0xFFFFFF0F {
                    0x1F801000 => {
                        //read base address
                        self.channels[channel_num].base_addr
                    }
                    0x1F801004 => {
                        //read block control
                        self.channels[channel_num].block
                    }
                    0x1F801008 => {
                        //read control
                        //println!("Reading dma control {} val {:#X}", channel_num, self.channels[channel_num].control);
                        self.channels[channel_num].control
                    }
                    _ => panic!("Unknown dma read {:#X}", addr),
                }
            }
        }
    }

    pub fn write_word(&mut self, addr: u32, value: u32) {
        let channel_num = (((addr & 0x000000F0) >> 4) - 0x8) as usize;
        match addr {
            0x1F8010F0 => self.control = value,
            0x1F8010F4 => {
                let normal_bits = value & 0x80FFFFFF; //These bits are written normally
                let ack_bits = (value >> 24) & 0x7F; //These bits are written as a one to clear. 0x7F0000
                let acked_bits = ((self.interrupt >> 24) & 0x7F) & !ack_bits;
                self.interrupt = normal_bits | (acked_bits << 24)
            }
            _ => {
                match addr & 0xFFFFFF0F {
                    0x1F801000 => {
                        //Set base address
                        //println!("Wrote DMA base {} with {:#X} addr {:#X}", channel_num, value, addr);
                        self.channels[channel_num].base_addr = value & 0xFFFFFF;
                    }
                    0x1F801004 => {
                        //Set block control
                        //println!("Wrote DMA block {} with {:#X}", channel_num, value);
                        self.channels[channel_num].block = value;
                    }
                    0x1F801008 => {
                        //Set control
                        //println!("Wrote DMA control {} with {:#X}", channel_num, value);
                        self.channels[channel_num].control = value;
                    }
                    _ => panic!("Unknown dma write {:#X}", addr),
                };
            }
        };
    }

    pub fn update(&mut self) {
        let should_flag = self.interrupt.get_bit(15)
            || (self.interrupt.get_bit(23)
                && (self.interrupt.get_bits(16..=22) > 0 && self.interrupt.get_bits(24..=30) > 0));
        self.interrupt.set_bit(31, should_flag);
    }

    fn channel_enabled(&self, channel_num: usize) -> bool {
        self.control.get_bit((channel_num * 4) + 3)
    }

    fn raise_irq(&mut self, channel_num: usize) {
        let irq_enabled = self.interrupt.get_bit(16 + channel_num);
        if irq_enabled {
            self.interrupt.set_bit(24 + channel_num, true);
        }
    }
}

pub fn execute_dma_cycle(cpu: &mut R3000) {
    //Populate list of running and enabled dma channels
    let mut channels_to_run: Vec<usize> = Vec::new();
    for i in 0..NUM_CHANNELS {
        let channel = &cpu.main_bus.dma.channels[i];
        if cpu.main_bus.dma.channel_enabled(i) && channel.enabled() {
            channels_to_run.push(i);
        }
    }

    //Execute dma copy for each channel
    for num in channels_to_run {
        match num {
            2 => {
                //GPU
                match cpu.main_bus.dma.channels[num].control {
                    0x01000401 => {
                        //Linked list mode. mem -> gpu
                        let mut addr = cpu.main_bus.dma.channels[num].base_addr;
                        //println!("Starting linked list transfer. addr {:#X}", addr);
                        let mut header = cpu.main_bus.read_word(addr);
                        //println!("base addr: {:#X}. base header: {:#X}", addr, header);
                        loop {
                            let num_words = (header >> 24) & 0xFF;
                            for i in 0..num_words {
                                let packet = cpu.main_bus.read_word((addr + 4) + (i * 4));
                                cpu.main_bus.gpu.send_gp0_command(packet);
                            }
                            //println!("addr {:#X}, header {:#X}, nw {}", addr, header, num_words);
                            if header & 0x800000 != 0 {
                                break;
                            }
                            addr = header & 0xFFFFFF;
                            header = cpu.main_bus.read_word(addr);
                        }
                        //println!("DMA2 linked list transfer done.");
                        cpu.main_bus.dma.channels[num].complete();
                        cpu.main_bus.dma.raise_irq(num);
                        cpu.fire_external_interrupt(InterruptSource::DMA);
                    }

                    0x01000201 => {
                        //VramWrite
                        let entries = (cpu.main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                        let block_size = (cpu.main_bus.dma.channels[num].block) & 0xFFFF;
                        let base_addr = cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                        for i in 0..entries {
                            for j in 0..block_size {
                                let packet = cpu
                                    .main_bus
                                    .read_word(base_addr + ((i * block_size) * 4) + (j * 4));
                                cpu.main_bus.gpu.send_gp0_command(packet);
                            }
                        }
                        //println!("DMA2 block transfer done.");
                        cpu.main_bus.dma.channels[num].complete();
                        cpu.main_bus.dma.raise_irq(num);
                        cpu.fire_external_interrupt(InterruptSource::DMA);
                    }
                    0x1000200 => {
                        //VramRead
                        //TODO: Implement properly

                        let entries = (cpu.main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                        let block_size = (cpu.main_bus.dma.channels[num].block) & 0xFFFF;
                        let base_addr = cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                        for i in 0..entries {
                            for j in 0..block_size {
                                //Lets just write all zeros for now
                                cpu.main_bus.write_word(
                                    base_addr + ((i * block_size) * 4) + (j * 4),
                                    0x50005000,
                                );
                            }
                        }

                        cpu.main_bus.dma.channels[num].complete();
                        cpu.main_bus.dma.raise_irq(num);
                        cpu.fire_external_interrupt(InterruptSource::DMA);
                    }
                    _ => {
                        panic!("Unknown gpu DMA mode. This must be a custom transfer. Control was {:#X}", cpu.main_bus.dma.channels[num].control)
                    }
                }
            }

            3 => {
                let words = (cpu.main_bus.dma.channels[num].block) & 0xFFFF;
                let base_addr = (cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF) as usize;
                let data = cpu.main_bus.cd_drive.sector_data_take();
                cpu.main_bus.memory.data[base_addr..(base_addr + (words * 4) as usize)].copy_from_slice(data);
                cpu.main_bus.dma.channels[num].complete();
                cpu.main_bus.dma.raise_irq(num);
                cpu.fire_external_interrupt(InterruptSource::DMA);               
            }

            6 => {
                //OTC
                //OTC is only used to reset the ordering table. So we can ignore a lot of the parameters
                let entries = cpu.main_bus.dma.channels[num].block & 0xFFFF;
                let base = cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                //println!("Initializing {} entries ending at {:#X}", entries, base);
                for i in 0..=entries {
                    let addr = base - ((entries - i) * 4);
                    if i == 0 {
                        //The first entry should point to the end of memory
                        cpu.main_bus.write_word(addr, 0x00FFFFFF);
                        //println!("Wrote DMA6 end at {:#X} pointing to {:#X}", addr, 0x00FFFFFF);
                    } else {
                        //All the others should point to the address below
                        cpu.main_bus.write_word(addr, addr - 4);
                        //println!("Wrote DMA6 header at {:#X} pointing to {:#X}", addr, addr - 4);
                    }
                }
                //println!("DMA6 done. Marking complete and raising irq");
                cpu.main_bus.dma.channels[num].complete();
                cpu.main_bus.dma.raise_irq(num);
                cpu.fire_external_interrupt(InterruptSource::DMA);
            }
            _ => panic!("Unable to transfer unknown DMA channel {}!", num),
        }
    }
    cpu.main_bus.dma.update();
}
