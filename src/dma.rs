use crate::cpu::{InterruptSource, R3000};
use bit_field::BitField;
use log::{error, info, trace};

const NUM_CHANNELS: usize = 7;

const DMA_CHANNEL_NAMES: [&str; 7] = [
    "MDECin",
    "MDECout",
    "GPU",
    "CDROM",
    "SPU",
    "PIO",
    "OTC"
];

#[derive(Clone)]
struct Channel {
    channel_num: usize,
    base_addr: u32,
    block: u32,
    control: u32,
}

impl Channel {
    fn new(num: usize) -> Self {
        Self {
            base_addr: 0,
            block: 0,
            control: 0x0,
            channel_num: num,
        }
    }

    fn enabled(&self) -> bool {
        self.control.get_bit(24) && if self.channel_num == 0 || self.channel_num == 3 {
            self.control.get_bit(28)
        } else {
            true
        }
    }

    fn complete(&mut self) {
        self.control.set_bit(24, false);
        self.control.set_bit(28, false);
    }

    fn print_stats(&self) {
        info!("");
        info!("Channel: {}", DMA_CHANNEL_NAMES[self.channel_num]);
        let sync_mode = match (self.control & 0x600) >> 9 {
            0 => "Immediate (0)",
            1 => "Sync (1)",
            2 => "Linked list (2)",
            3 => "Reserved (3)",
            _ => "Invalid sync mode"
        };

        info!("SyncMode: {}", sync_mode);
        info!("Base Address: {:#X}", self.base_addr);

        match (self.control & 0x600) >> 9 {
            0 => info!("BC: {} words", self.block & 0xFFFF),
            1 => info!("BS: {} words per block  BA: {} blocks", self.block & 0xFFFF, (self.block >> 16) & 0xFFFF),
            _ => ()
        };
 
        info!("Direction: {} RAM", if self.control.get_bit(0) {"From"} else {"To"});
        info!("Address Step: {}", if self.control.get_bit(1) {"Backward"} else {"Forward"});
        info!("Chopping: {}", if self.control.get_bit(8) {"True"} else {"False"});

        info!("Chopping DMA Window Size: {} words", self.control.get_bits(16..18) << 1);
        info!("Chopping CPU Window Size: {} cycles", self.control.get_bits(20..22) << 1);
        info!("Start/Busy: {}", if self.control.get_bit(24) {"Start"} else {"Stopped"});
        info!("Start/Trigger: {}", if self.control.get_bit(28) {"Start"} else {"Stopped"});
        info!("");
    }
}

pub struct DMAState {
    channels: [Channel; NUM_CHANNELS],
    control: u32,
    interrupt: u32,
    cycles_to_wait: usize,
}

impl DMAState {
    pub fn new() -> Self {
        Self {
            channels: [
                Channel::new(0),
                Channel::new(1),
                Channel::new(2),
                Channel::new(3),
                Channel::new(4),
                Channel::new(5),
                Channel::new(6),
                ],
            control: 0x07654321, //Initial value on reset
            interrupt: 0,
            cycles_to_wait: 0,
        }
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        let channel_num = (((addr & 0x000000F0) >> 4) - 0x8) as usize;
        //println!("Reading DMA addr {:#X}", addr);
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
                        trace!("DMA ACCESS: Read base");
                        self.channels[channel_num].base_addr
                    }
                    0x1F801004 => {
                        //read block control
                        trace!("DMA ACCESS: Read block");
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
        //trace!("Write DMA word: addr {:#X} value {:#X}", addr, value);
        match addr {
            0x1F8010F0 => self.control = value,
            0x1F8010F4 => {
                self.interrupt = write_dicr(self.interrupt, value);
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
                        if value.get_bit(24) {
                            self.cycles_to_wait = 500;
                        }
                    }
                    _ => panic!("Unknown dma write {:#X}", addr),
                };
            }
        };
    }

    pub fn update_master_flag(&mut self) {
        let should_flag = self.interrupt.get_bit(15)
            || (self.interrupt.get_bit(23)
                && (self.interrupt.get_bits(16..=22) > 0 && self.interrupt.get_bits(24..=30) > 0));
        self.interrupt.set_bit(31, should_flag);

    }

    fn channel_enabled(&self, channel_num: usize) -> bool {
        self.control.get_bit((channel_num * 4) + 3)
    }

    fn raise_irq(&mut self, channel_num: usize) {
        if self.interrupt.get_bit(16 + channel_num) {
            self.interrupt.set_bit(24 + channel_num, true);
        }
    }

    fn irq_channel_enabled(&self, channel_num: usize) -> bool {
        self.interrupt.get_bit(15) || self.interrupt.get_bit(16 + channel_num) && self.interrupt.get_bit(23)//  && !self.interrupt.get_bit(24 + channel_num)
    }
}

pub fn execute_dma_cycle(cpu: &mut R3000) {
    
    // if cpu.main_bus.dma.cycles_to_wait > 0 {
    //     cpu.main_bus.dma.cycles_to_wait -= 1;
    //     return;
    // }

    //Populate list of running and enabled dma channels
    let mut channels_to_run: Vec<usize> = Vec::new();
    for i in 0..NUM_CHANNELS {
        let channel = &cpu.main_bus.dma.channels[i];
        if cpu.main_bus.dma.channel_enabled(i) && channel.enabled() {
            channels_to_run.push(i);
            //break; // Only try one channel per cycle
        }
    }
    //Execute dma copy for each channel
    for num in channels_to_run {
        //println!("Executing DMA {}", num);
        cpu.main_bus.dma.channels[num].print_stats();
        match num {
            2 => {
                //GPU
                match cpu.main_bus.dma.channels[num].control {
                    0x01000401 => {
                        //Linked list mode. mem -> gpu
                        let mut addr = cpu.main_bus.dma.channels[num].base_addr;
                        trace!("Starting linked list transfer. addr {:#X}", addr);
                        let mut header = cpu.main_bus.read_word(addr);
                        trace!("base addr: {:#X}. base header: {:#X}", addr, header);
                        loop {
                            let num_words = (header >> 24) & 0xFF;
                            //trace!("addr {:#X}, header {:#X}, nw {}", addr, header, num_words);
                            for i in 0..num_words {
                                let packet = cpu.main_bus.read_word((addr + 4) + (i * 4));
                                cpu.main_bus.gpu.send_gp0_command(packet);
                            }
                            if header & 0x800000 != 0 || header == 0x00FFFFFF {
                                break;
                            }

                            if addr == 0 {
                                trace!("Hit DMA infinite loop");
                                break;
                            }

                            //println!("Addr {:X}", addr);

                            addr = header & 0xFFFFFF;
                            header = cpu.main_bus.read_word(addr);
                        }
                        cpu.main_bus.dma.channels[num].base_addr = 0xFFFFFF;
                        //println!("DMA2 linked list transfer done.");
                        cpu.main_bus.dma.channels[num].complete();


                        cpu.main_bus.dma.raise_irq(num);
                        if cpu.main_bus.dma.irq_channel_enabled(num) {
                            cpu.fire_external_interrupt(InterruptSource::DMA);
                        } else {
                            trace!("DMA IRQ Rejected");
                            trace!("DICR: {:#X}", cpu.main_bus.dma.interrupt);
                        }
                    }

                    0x01000201 => {
                        //VramWrite
                        trace!("DMA: Starting VramWrite");
                        let entries = (cpu.main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                        let block_size = (cpu.main_bus.dma.channels[num].block) & 0xFFFF;
                        let base_addr = cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                        trace!("Block size {} Num blocks {} base {:#X}", block_size, entries, base_addr);
                        for i in 0..entries {
                            for j in 0..block_size {
                                let packet = cpu
                                    .main_bus
                                    .read_word(base_addr + ((i * block_size) * 4) + (j * 4));
                                cpu.main_bus.gpu.send_gp0_command(packet);
                            }
                        }
                        trace!("DMA2 block transfer done.");
                        cpu.main_bus.dma.channels[num].base_addr += entries * block_size * 4;
                        cpu.main_bus.dma.channels[num].complete();
                        cpu.main_bus.dma.raise_irq(num);
                        if cpu.main_bus.dma.irq_channel_enabled(num) {
                            cpu.fire_external_interrupt(InterruptSource::DMA);
                        } else {
                            trace!("DMA IRQ Rejected");
                            trace!("DICR: {:#X}", cpu.main_bus.dma.interrupt);
                        }
                    }
                    0x1000200 => {
                        //VramRead
                        //TODO: Implement properly
                        trace!("VRAM read");

                        let entries = (cpu.main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                        let block_size = (cpu.main_bus.dma.channels[num].block) & 0xFFFF;
                        let base_addr = cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                        for i in 0..entries {
                            for j in 0..block_size {
                                //Lets just write all zeros for now
                                cpu.main_bus.write_word(
                                    base_addr + ((i * block_size) * 4) + (j * 4),
                                    0xFFFFFFFF,
                                );
                            }
                        }
                        cpu.main_bus.dma.channels[num].base_addr += entries * block_size * 4;
                        cpu.main_bus.dma.channels[num].complete();
                        cpu.main_bus.dma.raise_irq(num);
                        if cpu.main_bus.dma.irq_channel_enabled(num) {
                            cpu.fire_external_interrupt(InterruptSource::DMA);
                        } else {
                            trace!("DMA IRQ Rejected");
                            trace!("DICR: {:#X}", cpu.main_bus.dma.interrupt);
                        }
                    }
                    _ => {
                        panic!("Unknown gpu DMA mode. This must be a custom transfer. Control was {:#X}", cpu.main_bus.dma.channels[num].control)
                    }
                }
            }

            3 => {
                let words = (cpu.main_bus.dma.channels[num].block) & 0xFFFF;
                let base_addr = (cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF) as usize;
                let data = cpu.main_bus.cd_drive.data_queue();

                if data.len() == 0 {
                    panic!("Tried to do dma on empty cd buffer");
                } else {
                    if data.len() < (words as usize) * 4 {
                        let diff = ((words as usize) * 4) - data.len();
                        for i in 0..diff {
                            data.push(data[i]);
                        }
                    }
                }


                trace!("Words {} base_addr {:#X}", words, base_addr);
                if base_addr <= 0x121CA8 && base_addr + (words * 4) as usize >= 0x121CA8 {
                    println!("CD DMA thing touched it");
                }
                for i in 0..(words * 4) {
                    cpu.main_bus.memory.data[(base_addr + i as usize)] = data[i as usize];
                }
                //cpu.main_bus.memory.data[base_addr..(base_addr + (words * 4) as usize)].copy_from_slice(data);
                data.drain(0..((words as usize) * 4));
                cpu.main_bus.dma.channels[num].complete();
                cpu.main_bus.dma.raise_irq(num);
                if cpu.main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", cpu.main_bus.dma.interrupt);
                }     
            }

            4 => {
                //SPU
                
                


                cpu.main_bus.dma.channels[num].complete();
                cpu.main_bus.dma.raise_irq(num);
                if cpu.main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", cpu.main_bus.dma.interrupt);
                }
            }

            6 => {
                //OTC
                //OTC is only used to reset the ordering table. So we can ignore a lot of the parameters
                let entries = cpu.main_bus.dma.channels[num].block & 0xFFFF;
                let base = cpu.main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                trace!("Initializing {} entries ending at {:#X}", entries, base);
                
                for i in 0..=entries {
                    let addr = base - ((entries - i) * 4);
                    if i == 0 {
                        //The first entry should point to the end of memory
                        cpu.main_bus.write_word(addr, 0xFFFFFF);
                        //trace!("Wrote DMA6 end at {:#X} val {:#X}", addr, 0xFFFFFF);
                    } else {
                        //All the others should point to the address below
                        cpu.main_bus.write_word(addr, (addr - 4) & 0xFFFFFF);
                        //trace!("Wrote DMA6 header at {:#X} val {:#X}", addr, (addr - 4) & 0xFFFFFF);
                    }
                }
                trace!("DMA6 done. Marking complete and raising irq");
                cpu.main_bus.dma.channels[num].complete();
                cpu.main_bus.dma.raise_irq(num);
                if cpu.main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", cpu.main_bus.dma.interrupt);
                }
            }
            _ => panic!("Unable to transfer unknown DMA channel {}!", num),
        }
    }
    cpu.main_bus.dma.update_master_flag();
    //cpu.main_bus.dma.cycles_to_wait = 200; // Lets give the cpu some time to see that the DMA is done
}

fn write_dicr(current_value: u32, value: u32) -> u32 {
    if value.get_bit(15) {error!("OH GOD BIT 15 IS SET")}
    let normal_bits = value & 0xFFFFFF; //These bits are written normally
    let ack_bits = (value >> 24) & 0x7F; //These bits are written as a one to clear. 0x7F0000
    let acked_bits = ((current_value >> 24) & 0x7F) & !ack_bits;
    normal_bits | (acked_bits << 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_dicr() {
        //test full irq clear
        assert_eq!(write_dicr(0xFFFFFFFF, 0x7F000000), 0x0);
        assert_eq!(write_dicr(0x7F000000, 0x7F000000), 0x0);
        assert_eq!(write_dicr(0x0, 0x7F000001), 0x1);
    }
}