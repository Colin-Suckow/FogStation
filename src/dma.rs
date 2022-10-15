use crate::cpu::{InterruptSource, R3000};
use bit_field::BitField;
use log::{error, info, trace};
use crate::{MainBus, Scheduler};

const NUM_CHANNELS: usize = 7;

const DMA_CHANNEL_NAMES: [&str; 7] = ["MDECin", "MDECout", "GPU", "CDROM", "SPU", "PIO", "OTC"];

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
        self.control.get_bit(24)
            && if self.sync_mode() == 0 {
                self.control.get_bit(28)
            } else {
                true
            }
    }

    fn complete(&mut self) {
        self.control.set_bit(24, false);
        //self.control.set_bit(28, false);
    }

    fn print_stats(&self) {
        info!("");
        info!("Channel: {}", DMA_CHANNEL_NAMES[self.channel_num]);
        let sync_mode = match (self.control & 0x600) >> 9 {
            0 => "Immediate (0)",
            1 => "Sync (1)",
            2 => "Linked list (2)",
            3 => "Reserved (3)",
            _ => "Invalid sync mode",
        };

        info!("SyncMode: {}", sync_mode);
        info!("Base Address: {:#X}", self.base_addr);

        match (self.control & 0x600) >> 9 {
            0 => info!("BC: {} words", self.block & 0xFFFF),
            1 => info!(
                "BS: {} words per block  BA: {} blocks",
                self.block & 0xFFFF,
                (self.block >> 16) & 0xFFFF
            ),
            _ => (),
        };

        info!(
            "Direction: {} RAM",
            if self.control.get_bit(0) {
                "From"
            } else {
                "To"
            }
        );
        info!(
            "Address Step: {}",
            if self.control.get_bit(1) {
                "Backward"
            } else {
                "Forward"
            }
        );
        info!(
            "Chopping: {}",
            if self.control.get_bit(8) {
                "True"
            } else {
                "False"
            }
        );

        info!(
            "Chopping DMA Window Size: {} words",
            self.control.get_bits(16..18) << 1
        );
        info!(
            "Chopping CPU Window Size: {} cycles",
            self.control.get_bits(20..22) << 1
        );
        info!(
            "Start/Busy: {}",
            if self.control.get_bit(24) {
                "Start"
            } else {
                "Stopped"
            }
        );
        info!(
            "Start/Trigger: {}",
            if self.control.get_bit(28) {
                "Start"
            } else {
                "Stopped"
            }
        );
        info!("");
    }

    fn sync_mode(&self) -> usize {
        self.control.get_bits(9..=10) as usize
    }
}

pub struct DMAState {
    channels: [Channel; NUM_CHANNELS],
    control: u32,
    interrupt: u32,
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
        //println!("Write DMA word: addr {:#X} value {:#X}", addr, value);
        match addr {
            0x1F8010F0 => self.control = value,
            0x1F8010F4 => {
                self.interrupt = write_dicr(self.interrupt, value);
                self.update_master_flag();
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

    pub fn read_byte(&mut self, addr: u32) -> u8 {
        let _channel_num = (((addr & 0x000000F0) >> 4) - 0x8) as usize;
        match addr {
            0x1F8010F6 => ((self.interrupt >> 16) & 0xFF) as u8,
            _ => panic!("Unknown DMA read byte {:#X}", addr),
        }
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        let _channel_num = (((addr & 0x000000F0) >> 4) - 0x8) as usize;
        match addr {
            0x1F8010F6 => {
                self.interrupt &= !(0xFF << 16);
                self.interrupt |= (value as u32) << 16
            }
            _ => panic!("Unknown DMA read byte {:#X}", addr),
        }
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
        self.interrupt.get_bit(15)
            || self.interrupt.get_bit(16 + channel_num) && self.interrupt.get_bit(23)
        //  && !self.interrupt.get_bit(24 + channel_num)
    }
}

pub fn execute_dma_cycle(cpu: &mut R3000, main_bus: &mut MainBus, scheduler: &mut Scheduler) {
    //Populate list of running and enabled dma channels
    let mut channels_to_run: Vec<usize> = Vec::new();
    for i in 0..NUM_CHANNELS {
        let channel = &main_bus.dma.channels[i];
        if main_bus.dma.channel_enabled(i) && channel.enabled() {
            channels_to_run.push(i);
            //break; // Only try one channel per cycle
        }
    }
    //Execute dma copy for each channel
    for num in channels_to_run {
        main_bus.dma.channels[num].print_stats();
        //main_bus.dma.channels[num].control.set_bit(28, false); // Disable this channel's Start/Trigger bit because the transfer has begun
        match num {
            0 => {
                //MDEC_in

                let mut entries = (main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                let mut block_size = (main_bus.dma.channels[num].block) & 0xFFFF;
                let base_addr = main_bus.dma.channels[num].base_addr & 0xFFFFFF;

                if entries == 0 {
                    entries = 1
                };
                if block_size == 0 {
                    block_size = 1
                };

                match main_bus.dma.channels[num].control {
                    0x01000201 => {
                        for i in 0..entries {
                            for j in 0..block_size {
                                let word = main_bus
                                    .read_word(base_addr + ((i * block_size) * 4) + (j * 4));
                                main_bus.mdec.bus_write_word(0x1f801820, word);
                            }
                        }
                    }
                    control => panic!("Unknown MDEC DMA transfer! {:#X}", control),
                }

                main_bus.dma.channels[num].complete();
                main_bus.dma.raise_irq(num);
                if main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", main_bus.dma.interrupt);
                }
            }

            1 => {
                //MDEC_out

                let mut entries = (main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                let mut block_size = (main_bus.dma.channels[num].block) & 0xFFFF;
                let base_addr = main_bus.dma.channels[num].base_addr & 0xFFFFFF;

                if entries == 0 {
                    entries = 1
                };
                if block_size == 0 {
                    block_size = 1
                };

                match main_bus.dma.channels[num].control {
                    0x01000200 => {
                        for i in 0..entries {
                            for j in 0..block_size {
                                let word = main_bus.mdec.bus_read_word(0x1f801820);
                                //println!("MDEC_out DMA pushing word {:#X}", word);
                                main_bus
                                    .write_word(base_addr + ((i * block_size) * 4) + (j * 4), word, scheduler);
                            }
                        }
                        trace!("MDEC_out transfer done!")
                    }
                    control => println!("Unknown MDEC DMA transfer! {:#X}", control),
                }

                main_bus.dma.channels[num].complete();
                main_bus.dma.raise_irq(num);
                if main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                    trace!("IRQ fired");
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", main_bus.dma.interrupt);
                }
            }

            2 => {
                //GPU
                match main_bus.dma.channels[num].control {
                    0x01000401 => {
                        //Linked list mode. mem -> gpu
                        let mut addr = main_bus.dma.channels[num].base_addr;
                        trace!("Starting linked list transfer. addr {:#X}", addr);
                        let mut header = main_bus.read_word(addr);
                        trace!("base addr: {:#X}. base header: {:#X}", addr, header);
                        loop {
                            let num_words = (header >> 24) & 0xFF;
                            //trace!("addr {:#X}, header {:#X}, nw {}", addr, header, num_words);
                            for i in 0..num_words {
                                let packet = main_bus.read_word((addr + 4) + (i * 4));
                                main_bus.gpu.send_gp0_command(packet);
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
                            header = main_bus.read_word(addr);
                        }
                        main_bus.dma.channels[num].base_addr = 0xFFFFFF;
                        //println!("DMA2 linked list transfer done.");
                        main_bus.dma.channels[num].complete();

                        main_bus.dma.raise_irq(num);
                        if main_bus.dma.irq_channel_enabled(num) {
                            cpu.fire_external_interrupt(InterruptSource::DMA);
                        } else {
                            trace!("DMA IRQ Rejected");
                            trace!("DICR: {:#X}", main_bus.dma.interrupt);
                        }
                    }

                    0x01000201 => {
                        //VramWrite
                        trace!("DMA: Starting VramWrite");
                        let mut entries = (main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                        let mut block_size = (main_bus.dma.channels[num].block) & 0xFFFF;
                        let base_addr = main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                        if entries == 0 {
                            entries = 1
                        };
                        if block_size == 0 {
                            block_size = 1
                        };
                        trace!(
                            "Block size {} Num blocks {} base {:#X}",
                            block_size,
                            entries,
                            base_addr
                        );
                        for i in 0..entries {
                            for j in 0..block_size {
                                let packet = main_bus
                                    .read_word(base_addr + ((i * block_size) * 4) + (j * 4));
                                main_bus.gpu.send_gp0_command(packet);
                            }
                        }
                        trace!("DMA2 block transfer done.");
                        main_bus.dma.channels[num].base_addr += entries * block_size * 4;
                        main_bus.dma.channels[num].complete();
                        main_bus.dma.raise_irq(num);
                        if main_bus.dma.irq_channel_enabled(num) {
                            cpu.fire_external_interrupt(InterruptSource::DMA);
                        } else {
                            trace!("DMA IRQ Rejected");
                            trace!("DICR: {:#X}", main_bus.dma.interrupt);
                        }
                    }
                    0x1000200 => {
                        //VramRead
                        //TODO: Implement properly
                        trace!("VRAM read");

                        let mut entries = (main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                        let mut block_size = (main_bus.dma.channels[num].block) & 0xFFFF;
                        let base_addr = main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                        if entries == 0 {
                            entries = 1
                        };
                        if block_size == 0 {
                            block_size = 1
                        };
                        for i in 0..entries {
                            for j in 0..block_size {
                                let val = main_bus.gpu.read_word_gp0();
                                main_bus
                                    .write_word(base_addr + ((i * block_size) * 4) + (j * 4), val, scheduler);
                            }
                        }
                        main_bus.dma.channels[num].base_addr += entries * block_size * 4;
                        main_bus.dma.channels[num].complete();
                        main_bus.dma.raise_irq(num);
                        if main_bus.dma.irq_channel_enabled(num) {
                            cpu.fire_external_interrupt(InterruptSource::DMA);
                        } else {
                            trace!("DMA IRQ Rejected");
                            trace!("DICR: {:#X}", main_bus.dma.interrupt);
                        }
                    }
                    _ => {
                        panic!("Unknown gpu DMA mode. This must be a custom transfer. Control was {:#X}", main_bus.dma.channels[num].control)
                    }
                }
            }

            3 => {
                let mut words = (main_bus.dma.channels[num].block) & 0xFFFF;
                let base_addr = (main_bus.dma.channels[num].base_addr & 0xFFFFFF) as usize;
                let data = main_bus.cd_drive.data_queue();

                if words == 0 {
                    words = 0x10000;
                }

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

                for i in 0..(words * 4) {
                    main_bus.memory.data[(base_addr + i as usize)] = data[i as usize];
                }
                //main_bus.memory.data[base_addr..(base_addr + (words * 4) as usize)].copy_from_slice(data);
                data.drain(0..((words as usize) * 4));
                main_bus.dma.channels[num].complete();
                main_bus.dma.raise_irq(num);
                if main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", main_bus.dma.interrupt);
                }
            }

            4 => {
                //SPU

                let mut entries = (main_bus.dma.channels[num].block >> 16) & 0xFFFF;
                let mut block_size = (main_bus.dma.channels[num].block) & 0xFFFF;
                let _base_addr = main_bus.dma.channels[num].base_addr & 0xFFFFFF;

                if entries == 0 {
                    entries = 1
                };
                if block_size == 0 {
                    block_size = 1
                };

                match main_bus.dma.channels[num].control {
                    0x01000201 => {
                        for _ in 0..entries {
                            for _ in 0..block_size {
                                main_bus.spu.write_half_word(0x1F801DA8, 0);
                                main_bus.spu.write_half_word(0x1F801DA8, 0);
                            }
                        }
                    }
                    control => println!("Unknown SPU DMA transfer! {:#X}", control),
                }

                main_bus.dma.channels[num].complete();
                main_bus.dma.raise_irq(num);
                if main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", main_bus.dma.interrupt);
                }
            }

            6 => {
                //OTC
                //OTC is only used to reset the ordering table. So we can ignore a lot of the parameters
                let mut entries = main_bus.dma.channels[num].block & 0xFFFF;
                let base = main_bus.dma.channels[num].base_addr & 0xFFFFFF;
                trace!("Initializing {} entries ending at {:#X}", entries, base);

                if entries == 0 {
                    entries = 1;
                }

                for i in 0..entries {
                    let addr = base - (((entries - 1) - i) * 4);
                    if i == 0 {
                        //The first entry should point to the end of memory
                        main_bus.write_word(addr, 0xFFFFFF, scheduler);
                        //println!("Wrote DMA6 end at {:#X} val {:#X}", addr, 0xFFFFFF);
                    } else {
                        //All the others should point to the address below
                        main_bus.write_word(addr, (addr - 4) & 0xFFFFFF, scheduler);
                        //println!("Wrote DMA6 header at {:#X} val {:#X}", addr, (addr - 4) & 0xFFFFFF);
                    }
                }
                trace!("DMA6 done. Marking complete and raising irq");
                main_bus.dma.channels[num].complete();
                main_bus.dma.raise_irq(num);
                if main_bus.dma.irq_channel_enabled(num) {
                    cpu.fire_external_interrupt(InterruptSource::DMA);
                } else {
                    trace!("DMA IRQ Rejected");
                    trace!("DICR: {:#X}", main_bus.dma.interrupt);
                }
            }
            _ => panic!("Unable to transfer unknown DMA channel {}!", num),
        }
    }

    let old_flag = main_bus.dma.interrupt.get_bit(31);
    main_bus.dma.update_master_flag();
    if !old_flag && main_bus.dma.interrupt.get_bit(31) {
        cpu.fire_external_interrupt(InterruptSource::DMA);
    }
}

fn write_dicr(current_value: u32, value: u32) -> u32 {
    if value.get_bit(15) {
        error!("OH GOD BIT 15 IS SET")
    }
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
