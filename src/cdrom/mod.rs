use bit_field::BitField;
use commands::*;
use disc::*;
use log::{trace, warn};

use crate::cpu::{InterruptSource, R3000};
use std::collections::VecDeque;
use crate::{CpuCycles, MainBus, Scheduler};
use crate::ScheduleTarget::{CDIrq, CDPacket};

mod commands;
pub mod disc;

#[derive(Debug, PartialEq, Copy, Clone)]
#[allow(dead_code)]
pub(super) enum DriveState {
    Play,
    Seek,
    Read,
    Idle,
    Pause,
}

#[derive(Debug, PartialEq, Copy, Clone)]
#[allow(dead_code)]
pub(super) enum MotorState {
    Off,
    SpinUp,
    On,
}

#[derive(Debug, Copy, Clone)]
pub enum SectorSize {
    DataOnly = 0x800,
    WholeSector = 0x924,
}

enum DriveSpeed {
    Single,
    Double,
}
#[derive(Debug, PartialEq, Clone, Copy)]
#[allow(dead_code)]
pub(super) enum IntCause {
    INT1,
    INT2,
    INT3,
    INT4,
    INT5,
    INT6,
    INT7,
    INT8,
    INT10h,
}

impl IntCause {
    fn bitflag(&self) -> u8 {
        match self {
            IntCause::INT1 => 1,
            IntCause::INT2 => 2,
            IntCause::INT3 => 3,
            IntCause::INT4 => 4,
            IntCause::INT5 => 5,
            IntCause::INT6 => 6,
            IntCause::INT7 => 7,
            IntCause::INT8 => 8,
            IntCause::INT10h => 0x10,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Packet {
    internal_id: u32,
    cause: IntCause,
    response: Vec<u8>,
    execution_cycles: u32,
    extra_response: Option<Box<Packet>>,
    command: u8,
    need_irq: bool,
}

#[derive(Debug)]
pub(super) struct Block {
    _data: Vec<u8>,
}
#[allow(dead_code)]

pub struct CDDrive {
    cycle_counter: u32,
    next_id: u32,
    command_start_cycle: u32,
    running_commands: Vec<Packet>,

    drive_state: DriveState,
    motor_state: MotorState,
    drive_mode: u8,

    disc: Option<Disc>,

    parameter_queue: VecDeque<u8>,
    response_queue: VecDeque<u8>,
    data_queue: Vec<Sector>,
    response_data_queue: Vec<u8>,
    ready_packets: Vec<Packet>, // List of packets that have been run and are ready to be delivered upon ack

    want_data: bool,

    status_index: u8,

    current_seek_target: DiscIndex,
    next_seek_target: DiscIndex,
    seek_complete: bool,
    read_offset: usize,

    reg_interrupt_flag: u8,
    reg_interrupt_enable: u8,

    read_enabled: bool,
    sector_awaiting_delivery: bool,
    irq_request: bool,

    //Probably useless registers
    reg_sound_map_data_out: u8,
}

impl CDDrive {
    pub fn new() -> Self {
        Self {
            cycle_counter: 0,
            next_id: 0,
            command_start_cycle: 0,

            running_commands: Vec::new(),

            parameter_queue: VecDeque::new(),
            data_queue: Vec::new(),
            response_queue: VecDeque::new(),
            response_data_queue: Vec::new(),
            ready_packets: Vec::new(),

            status_index: 0,

            disc: None,

            want_data: false,
            drive_state: DriveState::Idle,
            motor_state: MotorState::On,
            drive_mode: 0,

            next_seek_target: DiscIndex::new_dec(0, 0, 0),
            current_seek_target: DiscIndex::new_dec(0, 0, 0),
            seek_complete: false,
            read_offset: 0,

            read_enabled: false,
            sector_awaiting_delivery: false,
            irq_request: false,

            reg_interrupt_flag: 0,
            reg_interrupt_enable: 0,

            //Probably useless registers
            reg_sound_map_data_out: 0,
        }
    }

    pub fn write_byte(&mut self, addr: u32, val: u8, scheduler: &mut Scheduler) {
        ////println!("CDROM writing {:#X}.Index({}) val {:#X}", addr, self.status_index & 0x3, val);
        match addr {
            0x1F801800 => self.status_index = val & 0x3, //Status
            0x1F801801 => match self.status_index {
                0 => self.execute_command(val, scheduler),
                1 => self.reg_sound_map_data_out = val,
                2 => panic!("CD: 0x1F801801 write byte unknown index 2"),
                3 => trace!("CD: Wrote Right-CD-Out Right SPU volume"),
                _ => unreachable!(),
            },
            0x1F801802 => match self.status_index {
                0 => self.push_parameter(val),
                1 => self.write_interrupt_enable_register(val),
                2 => trace!("CD: Wrote Left-CD-Out Right SPU volume"),
                3 => trace!("CD: Wrote Right-CD-Out Left SPU volume"),
                _ => unreachable!(),
            },
            0x1F801803 => match self.status_index {
                0 => {
                    if val.get_bit(5) {
                        panic!("CD INT10 requested");
                    }
                    if val.get_bit(7) {
                        // Try to load latest sector from buffer
                        let _sector_size = *self.sector_size() as usize;
                        if self.data_queue.len() > 0 {
                            let sector = self.data_queue.remove(0);
                            ////println!("Loaded a sector!");
                            ////println!("Loaded sector. Index {}, sector # {}", sector.index(), sector.index().sector_number());
                            ////println!("Filling buffer with sector size {:?}", self.sector_size());
                            self.response_data_queue
                                .extend(sector.consume(self.sector_size()));
                        } else {
                            ////println!("Game requested sector load, but the input buffer was empty!");
                        }
                    } else {
                        self.response_data_queue.clear();
                    }
                }
                1 => self.write_interrupt_flag_register(val, scheduler),
                2 => trace!("CD: Wrote Left-CD-Out Left SPU volume"),
                3 => (), // Apply audio changes
                _ => unreachable!(),
            },
            _ => panic!(
                "CD: Tried to write unknown byte. Address: {:#X} Value: {:#X} Index: {}",
                addr, val, self.status_index
            ),
        }
    }

    pub fn read_byte(&mut self, addr: u32) -> u8 {
        let v = match addr {
            0x1F801800 => self.get_status_register(),
            0x1F801801 => match self.status_index {
                0 => self.pop_response(), // mirror
                1 => self.pop_response(),
                2 => panic!("CD: 0x1F801801 read byte unknown index 2"),
                3 => panic!("CD: 0x1F801801 read byte unknown index 3"),
                _ => unreachable!(),
            },
            0x1F801802 => match self.status_index {
                0 => self.pop_data(),
                1 => panic!("CD: 0x1F801802 read byte unknown index 1"),
                2 => panic!("CD: 0x1F801802 read byte unknown index 2"),
                3 => panic!("CD: 0x1F801802 read byte unknown index 3"),
                _ => unreachable!(),
            },
            0x1F801803 => {
                match self.status_index {
                    0 => self.reg_interrupt_enable,
                    1 => self.reg_interrupt_flag | 0xE0,
                    2 => panic!("CD: 0x1F801803 read byte unknown index 2"),
                    3 => self.reg_interrupt_flag | 0xE0, //Register mirror
                    _ => unreachable!(),
                }
            }
            _ => panic!(
                "CD: Tried to read unknown byte. Address: {:#X} Index: {}",
                addr, self.status_index
            ),
        };
        // //println!(
        //     "CDROM reading {:#X}.Index({}) = {:#X}",
        //     addr,
        //     self.status_index & 0x3,
        //     v
        // );
        v
    }

    pub fn load_disc(&mut self, disc: Disc) {
        self.disc = Some(disc);
    }

    pub fn remove_disc(&mut self) {
        self.disc = None;
    }

    pub fn disc(&self) -> &Option<Disc> {
        &self.disc
    }

    fn execute_command(&mut self, command: u8, scheduler: &mut Scheduler) {
        //println!("Received command {:#X}", command);

        //Execute
        {
            let parameters: Vec<u8> = self.parameter_queue.iter().map(|v| v.clone()).collect();
            let response = match command {
                0x1 => get_stat(self),
                0x2 => set_loc(self, parameters[0], parameters[1], parameters[2]),
                0x3 => play(self),
                0x6 => read_with_retry(self),
                0x8 => stop(self),
                0x9 => pause_read(self),
                0xA => init(self),
                0xB => mute(self),
                0xD => set_filter(self),
                0xE => set_mode(self, parameters[0]),
                0x10 => set_filter(self), //This is actually GetlocL. But I'm lazy right now. TODO: Implement this
                0x11 => set_filter(self), //This is actually GetlocP. But I'm lazy right now. TODO: Implement this
                0x13 => get_tn(self),
                0x14 => get_td(self, parameters[0]),
                0x15 => seek_data(self),
                0x16 => seek_data(self), //This should actually be seek_p, but I'm never using audio discs so we can reuse the data seek function
                0x1A => get_id(self),
                0x1B => read_with_retry(self), // This is actually ReadS (read without retry), but it behaves the same as ReadN, so I'm just using that
                0x1E => get_toc(self),
                0xC => demute(self),
                0x19 => {
                    //sub_function commands
                    match parameters[0] {
                        0x20 => commands::get_bios_date(self),
                        0x4 => start_sce(self),
                        0x5 => end_sce(self),
                        _ => panic!("CD: Unknown sub_function command {:#X}", parameters[0]),
                    }
                }
                _ => panic!("CD: Unknown command {:#X}!", command),
            };
            scheduler.schedule_event(CDPacket(response.internal_id), CpuCycles(response.execution_cycles));
            self.running_commands.push(response);
        }

        //Clear out old parameters
        self.parameter_queue.clear();
    }

    fn busy(&self) -> bool {
        false //self.reg_interrupt_flag != 0
    }

    fn get_status_register(&self) -> u8 {
        let mut status: u8 = 0;
        //0-1 index
        status |= self.status_index;
        //3 prmempt
        status |= (self.parameter_queue.is_empty() as u8) << 3;
        //4 prmrdy
        status |= (!(self.parameter_queue.len() >= 16) as u8) << 4;
        //5 RSLRRDY
        status |= (!self.response_queue.is_empty() as u8) << 5;
        //6 DRQSTS
        status |= (!self.response_data_queue.is_empty() as u8) << 6;
        // 7 BUSYSTS
        status |= (self.busy() as u8) << 7;
        //println!("Status: {:#X}", status);
        status
    }

    fn drive_speed(&self) -> DriveSpeed {
        match self.drive_mode.get_bit(7) {
            true => DriveSpeed::Double,
            false => DriveSpeed::Single,
        }
    }

    fn get_stat(&self) -> u8 {
        let mut status: u8 = 0;
        status |= match self.drive_state {
            DriveState::Play => 0x80,
            DriveState::Seek => 0x40,
            DriveState::Read => 0x20,
            _ => 0,
        };

        if self.motor_state == MotorState::On {
            status |= 0x2;
        };

        status
    }

    fn sector_size(&self) -> &SectorSize {
        match self.drive_mode.get_bit(5) {
            true => &SectorSize::WholeSector,
            false => &SectorSize::DataOnly,
        }
    }

    fn next_packet_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        return id;
    }

    fn push_parameter(&mut self, val: u8) {
        self.parameter_queue.push_back(val);
    }

    fn pop_response(&mut self) -> u8 {
        //println!("Popping response");
        match self.response_queue.pop_front() {
            Some(val) => val,
            None => {
                println!("CD: Tried to read response from empty response queue! Returning 0...");
                0
            }
        }
    }

    pub fn data_queue(&mut self) -> &mut Vec<u8> {
        &mut self.response_data_queue
    }

    pub fn pop_data(&mut self) -> u8 {
        self.response_data_queue.remove(0) // This is slow, but whatever for now. Using a proper deque is a bit difficult here
    }

    fn write_interrupt_flag_register(&mut self, val: u8, scheduler: &mut Scheduler) {
        //println!("Writing flag with val {:#X}   pre flag val {:#X}", val, self.reg_interrupt_flag);
        self.reg_interrupt_flag &= !(val & 0x1F);

        ////println!("Post flag {:#X}", self.reg_interrupt_flag);
        self.response_queue = VecDeque::new(); //Reset queue
        if val.get_bit(6) {
            ////println!("Clearing parameters");
            self.parameter_queue = VecDeque::new();
        }
        
        // Now that the command has been acked we insert the data
        if let Some(packet) = self.get_next_ready_packet() {
            //println!("Presenting because get was true");
            self.present_packet(packet, scheduler);
        } else {
            //println!("Not presenting because no ready packet");
        }
    }

    fn write_interrupt_enable_register(&mut self, val: u8) {
        self.reg_interrupt_enable = val & 0x1f;
    }

    pub fn get_enable(&self) -> u8 {
        self.reg_interrupt_enable
    }

    pub fn get_flag(&self) -> u8 {
        self.reg_interrupt_flag
    }

    fn queue_irq(&self, scheduler: &mut Scheduler) {
        // Wait 25k cycles before sending IRQ to simulate mechacon -> cpu communication delay
        scheduler.schedule_event(CDIrq, CpuCycles(1));
    }

    fn take_packet_by_id(&mut self, packet_id: u32) -> Option<Packet> {
        for i in 0..self.running_commands.len() {
            if self.running_commands[i].internal_id == packet_id {
                return Some(self.running_commands.remove(i));
            }
        }
        return None;
    }

    fn queue_ready_packet(&mut self, packet: Packet) {
        self.ready_packets.push(packet);
        //println!("Queing ready packet");
    }

    fn get_next_ready_packet(&mut self) -> Option<Packet> {
        //println!("Getting ready packet");
        self.ready_packets.pop()
    }

    fn present_packet(&mut self, packet: Packet, scheduler: &mut Scheduler) {
        //println!("Presenting packet with cause {:#X}", packet.cause.bitflag());
        self.response_queue = VecDeque::with_capacity(packet.response.len()); //Clear queue
        self
            .response_queue
            .extend(packet.response.iter());

        // This packet still needs to raise it's IRQ. Do it now
        if packet.need_irq {
            self.reg_interrupt_flag = packet.cause.bitflag();
            //println!("Raising IRQ because it wasn't done earlier");
            if self.reg_interrupt_enable & packet.cause.bitflag() == packet.cause.bitflag()
            {
                //println!("Firing IRQ");
                self.queue_irq(scheduler);
            }
            
        } else {
            //println!("Not need irq. Not queueing anything");
        }
        ////println!("All done!");
    }
}

pub fn cdpacket_event(cpu: &mut R3000, main_bus: &mut MainBus, scheduler: &mut Scheduler, packet_id: u32) {

    let mut packet = match main_bus.cd_drive.take_packet_by_id(packet_id) {
        Some(p) => p,
        None => {
            //No response ready right now
            //println!("Failed to find cd packet by id {}", packet_id);
            return;
        }
    };

    //println!("packet_event cause = {:#X} command = {:#X}", packet.cause.bitflag(), packet.command);

    if packet.cause == IntCause::INT1 && !main_bus.cd_drive.read_enabled {
        // Received completed ReadN but reads are disabled. Dropping...
        return;
    }

    //If the response has an extra response, push that to the in progress commands
    if let Some(mut ext_response) = packet.extra_response.clone() {
        ////println!("Extra response, filling. {:?}", ext_response);
        let next_id = main_bus.cd_drive.next_packet_id();
        ext_response.internal_id = next_id;
        scheduler.schedule_event(CDPacket(next_id), CpuCycles(ext_response.execution_cycles));
        main_bus.cd_drive.running_commands.push(*ext_response);
    };

    // Packet post conditions
    match packet.command {
        0x15 => {
            //Make sure this is the second response
            if packet.extra_response.is_none() {
                //End seek and return drive to idle state
                main_bus.cd_drive.read_offset = 0;
                main_bus.cd_drive.drive_state = DriveState::Idle;
            }
        }

        0x9 => {
            //pause
            if packet.extra_response.is_none() {
                main_bus.cd_drive.read_enabled = false;
            }
        }

        0x6 => {
            //ReadN
            if packet.cause == IntCause::INT1 {
                let new_sector = main_bus.cd_drive
                    .disc
                    .as_ref()
                    .expect("Tried to read nonexistent disc!")
                    .read_sector(
                        main_bus.cd_drive.next_seek_target
                            .plus_sector_offset(main_bus.cd_drive.read_offset),
                    );

                //println!("Read {} from disc. Read offset {}", new_sector.index(), main_bus.cd_drive.read_offset);

                main_bus.cd_drive.read_offset += 1;

                if main_bus.cd_drive.data_queue.len() >= 2 {
                    ////println!("DROPPED SECTOR");
                }

                // Get rid of all the middle sectors, leave only the oldest

                // if main_bus.cd_drive.data_queue.len() > 1 {
                //     main_bus.cd_drive
                //         .data_queue
                //         .drain(1..main_bus.cd_drive.data_queue.len());

                // }

                //main_bus.cd_drive.data_queue.clear();
                main_bus.cd_drive.data_queue.push(new_sector);

                if main_bus.cd_drive.read_enabled {
                    //println!("Inserting next ReadN");
                    let cycles = match main_bus.cd_drive.drive_speed() {
                        DriveSpeed::Single => 0x686da,
                        DriveSpeed::Double => 0x322df,
                    };
                    let response_packet = Packet {
                        internal_id: main_bus.cd_drive.next_packet_id(),
                        cause: IntCause::INT1,
                        response: vec![main_bus.cd_drive.get_stat()],
                        execution_cycles: cycles,
                        extra_response: None,
                        command: 0x6,
                        need_irq: false
                    };
                    scheduler.schedule_event(CDPacket(response_packet.internal_id), CpuCycles(response_packet.execution_cycles));
                    main_bus.cd_drive.running_commands.push(response_packet);
                }
            }
        }
        _ => (), //No actions for this command
    };

    if main_bus.cd_drive.reg_interrupt_flag != 0 {
        //println!("need_irq branch");
        // There is a pending IRQ. Save this info so we know to raise the IRQ later
        packet.need_irq = true;
    } else {
        //println!("Immediate IRQ branch");
        // There is no pending IRQ. Raise the IRQ and present it now
        main_bus.cd_drive.reg_interrupt_flag = packet.cause.bitflag();
            
        if main_bus.cd_drive.reg_interrupt_enable & packet.cause.bitflag() == packet.cause.bitflag()
        {
            main_bus.cd_drive.queue_irq(scheduler);
        }

        // Present the packet anyways since the ps1 is dumb
        main_bus.cd_drive.present_packet(packet.clone(), scheduler);
    }

    // Insert this packet into the queue
    main_bus.cd_drive.queue_ready_packet(packet);
}
