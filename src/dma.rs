use bit_field::BitField;
use crate::cpu::R3000;
use crate::bus::MainBus;

pub enum Channel {
    MDECin,
    MDECout,
    GPU,
    CDROM,
    SPU,
    PIO,
    OTC
}
enum StepDirection {
    Forward,
    Backward,
}

enum TransferDirection {
    ToRAM,
    FromRAM,
}

enum SyncMode {
    Immediate,
    Block,
    LinkedList,
    Reserved,
}

struct Command {
    channel: Channel,
    step_direction: StepDirection,
    transfer_direction: TransferDirection,
    chopping_enabled: bool,
    sync_mode: SyncMode,
    chopping_dma_size: u8,
    chopping_cpu_size: u8,
    busy: bool,
    trigger: bool,
}

pub struct DMAState {
    pub control_register: u32,
    pub interrupt_register: u32,
}

impl DMAState {
    pub fn new() -> Self {
        Self {
            control_register: 0,
            interrupt_register: 0,
        }
    }

    pub fn execute_cycle(&mut self, cpu: &mut R3000) {

    }

    pub fn enable_channel(&mut self, channel: Channel) {
        self.control_register.set_bit(get_channel_enable_bit(&channel), true);
    }

    pub fn disable_channel(&mut self, channel: Channel) {
        self.control_register.set_bit(get_channel_enable_bit(&channel), false);
    }

    pub fn set_channel_priority(&mut self, channel: Channel, priority: u32) {
        match channel {
            Channel::MDECin => {
                self.control_register.set_bits(0..=2, priority & 0b111);
            }
            Channel::MDECout => {
                self.control_register.set_bits(4..=6, priority & 0b111);
            }
            Channel::GPU => {
                self.control_register.set_bits(8..=10, priority & 0b111);
            }
            Channel::CDROM => {
                self.control_register.set_bits(12..=14, priority & 0b111);
            }
            Channel::SPU => {
                self.control_register.set_bits(16..=18, priority & 0b111);
            }
            Channel::PIO => {
                self.control_register.set_bits(20..=22, priority & 0b111);
            }
            Channel::OTC => {
                self.control_register.set_bits(24..=26, priority & 0b111);
            }
        }
    }

    pub fn set_interrupt(&mut self, channel: Channel) {
        self.interrupt_register.set_bit(get_channel_irq_enable_bit(&channel), true);
    }

}

fn get_channel_enable_bit(channel: &Channel) -> usize {
    match channel {
        Channel::MDECin => 3,
        Channel::MDECout => 7,
        Channel::GPU => 11,
        Channel::CDROM => 15,
        Channel::SPU => 19,
        Channel::PIO => 23,
        Channel::OTC => 27,
    }
}

fn get_channel_irq_enable_bit(channel: &Channel) -> usize {
    match channel {
        Channel::MDECin => 16,
        Channel::MDECout => 17,
        Channel::GPU => 18,
        Channel::CDROM => 19,
        Channel::SPU => 20,
        Channel::PIO => 21,
        Channel::OTC => 22,
    }
}

