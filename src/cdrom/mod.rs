use bit_field::BitField;
use commands::*;
use disc::*;
use log::{trace, warn};

use crate::cpu::{InterruptSource, R3000};
use std::{borrow::{Borrow, BorrowMut}, collections::VecDeque};

mod commands;
pub mod disc;


#[derive(Debug, PartialEq, Copy, Clone)]
pub(super) enum DriveState {
    Play,
    Seek,
    Read,
    Idle,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub(super) enum MotorState {
    Off,
    SpinUp,
    On,
}

#[derive(Debug, Copy, Clone)]
pub enum SectorSize {
    DataOnly = 0x800,
    WholeSector = 0x924
}
#[derive(Debug, PartialEq)]
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

#[derive(Debug)]
pub(super) struct Packet {
    cause: IntCause,
    response: Vec<u8>,
    execution_cycles: u32,
    extra_response: Option<Box<Packet>>,
    command: u8,
}

#[derive(Debug)]
pub(super) struct Block {
    data: Vec<u8>
}


pub struct CDDrive {
    cycle_counter: u32,
    command_start_cycle: u32,
    pending_response: Option<Packet>,

    drive_state: DriveState,
    motor_state: MotorState,
    drive_mode: u8,

    disc: Option<Disc>,

    parameter_queue: VecDeque<u8>,
    data_queue: VecDeque<u8>,
    response_queue: VecDeque<u8>,

    want_data: bool,

    status_index: u8,

    seek_target: DiscIndex,
    seek_complete: bool,
    read_offset: usize,

    reg_interrupt_flag: u8,
    reg_interrupt_enable: u8,

    read_enabled: bool,

    //Probably useless registers
    reg_sound_map_data_out: u8,
}

impl CDDrive {
    pub fn new() -> Self {
        Self {
            cycle_counter: 0,
            command_start_cycle: 0,

            pending_response: None,

            parameter_queue: VecDeque::new(),
            data_queue: VecDeque::new(),
            response_queue: VecDeque::new(),

            status_index: 0,

            disc: None,

            want_data: false,
            drive_state: DriveState::Idle,
            motor_state: MotorState::On,
            drive_mode: 0,

            seek_target: DiscIndex::new(0, 0, 0),
            seek_complete: false,
            read_offset: 0,

            read_enabled: false,

            reg_interrupt_flag: 0,
            reg_interrupt_enable: 0,

            //Probably useless registers
            reg_sound_map_data_out: 0,
        }
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            0x1F801800 => self.status_index = val & 0x3, //Status
            0x1F801801 => match self.status_index {
                0 => self.execute_command(val),
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
                    self.want_data = val.get_bit(7); //Only handle want_data. This will probably bite me later
                    //self.data_queue.clear();
                },
                1 => self.write_interrupt_flag_register(val),
                2 => trace!("CD: Wrote Left-CD-Out Left SPU volume"),
                3 => (),
                _ => unreachable!(),
            },
            _ => panic!(
                "CD: Tried to write unknown byte. Address: {:#X} Value: {:#X} Index: {}",
                addr, val, self.status_index
            ),
        }
    }

    pub fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0x1F801800 => self.get_status_register(),
            0x1F801801 => match self.status_index {
                0 => panic!("CD: 0x1F801801 read byte unknown index 0"),
                1 => self.pop_response(),
                2 => panic!("CD: 0x1F801803 read byte unknown index 2"),
                3 => panic!("CD: 0x1F801801 read byte unknown index 3"),
                _ => unreachable!(),
            },
            0x1F801803 => {
                match self.status_index {
                    0 => self.pop_data(),
                    1 => self.reg_interrupt_flag,
                    2 => panic!("CD: 0x1F801803 read byte unknown index 2"),
                    3 => self.reg_interrupt_flag, //Register mirror
                    _ => unreachable!(),
                }
            }
            _ => panic!(
                "CD: Tried to read unknown byte. Address: {:#X} Index: {}",
                addr, self.status_index
            ),
        }
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

    fn execute_command(&mut self, command: u8) {
        // Make sure theres no pending command
        let is_readn = if let Some(res) = &self.pending_response {
            res.cause == IntCause::INT1
        } else {
            false
        };

        //println!("Attemping to execute command!");

        if self.pending_response.is_none() || is_readn {
            //println!("Executing");
            //Execute
            {
                let parameters: Vec<u8> = self.parameter_queue.iter().map(|v| v.clone()).collect();
                let response = match command {
                    0x1 => get_stat(self),
                    0x2 => set_loc(self, parameters[0], parameters[1], parameters[2]),
                    0x6 => read_with_retry(self),
                    0x9 => stop_read(self),
                    0xA => init(self),
                    0xE => set_mode(self, parameters[0]),
                    0x13 => get_tn(self),
                    0x14 => get_td(self, parameters[0]),
                    0x15 => seek_data(self),
                    0x16 => seek_data(self), //This should actually be seek_p, but I'm never using audio discs so we can reuse the data seek function
                    0x1A => get_id(self),
                    0xC => demute(self),
                    0x19 => {
                        //sub_function commands
                        match parameters[0] {
                            0x20 => commands::get_bios_date(),
                            _ => panic!("CD: Unknown sub_function command {:#X}", parameters[0]),
                        }
                    }
                    _ => panic!("CD: Unknown command {:#X}!", command),
                };
                self.pending_response = Some(response);
            }
        }

        //Clear out old parameters
        self.parameter_queue.clear();
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
        status |= (!self.data_queue.is_empty() as u8) << 6;
        // 7 BUSYSTS
        //TODO when I find out what it means to be busy
        //status |= 0 << 7;

        status
    }

    fn get_stat(&self) -> u8 {
        let mut status: u8 = 0;
        status |= match self.drive_state {
            DriveState::Play => 0x80,
            DriveState::Seek => 0x40,
            DriveState::Read => 0x20,
            DriveState::Idle => 0,
        };

        if self.motor_state == MotorState::On {
            status |= 0x2;
        };

        status
    }

    fn sector_size(&self) -> &SectorSize {
        match self.drive_mode.get_bit(5) {
            true => &SectorSize::WholeSector,
            false => &SectorSize::DataOnly
        }
    }

    fn push_parameter(&mut self, val: u8) {
        self.parameter_queue.push_back(val);
    }

    fn pop_response(&mut self) -> u8 {
        match self.response_queue.pop_front() {
            Some(val) => val,
            None => {
                warn!("CD: Tried to read response from empty response queue! Returning 0...");
                0
            }
        }
    }

    pub fn pop_data(&mut self) -> u8 {
        if self.want_data && self.data_queue.is_empty() {
            //Out of data, get some more
            //println!("Fetching more data!");
            let data = self.disc.as_ref().expect("Tried to read nonexistant disc!").read_sector(
                        self.seek_target.plus_sector_offset(self.read_offset),
                        self.sector_size()
                    );
        
            self.read_offset += 1;
            self.data_queue.extend(data.iter());
            //println!("Fetched {} bytes!", self.data_queue.len())
        }
        match self.data_queue.pop_front() {
            Some(val) => val,
            None => {
                warn!("CD: Tried to read data from empty data queue! Returning 0...");
                0
            }
        }
    }

    pub fn sector_data_take(&mut self) -> &[u8] {
        
        //println!("Fetching more data!");
        let data = self.disc.as_ref().expect("Tried to read nonexistant disc!").read_sector(
                    self.seek_target.plus_sector_offset(self.read_offset),
                    self.sector_size()
                );
    
        self.read_offset += 1;
        data
        
    }

    fn write_interrupt_flag_register(&mut self, val: u8) {
        self.reg_interrupt_flag &= !val;
        self.response_queue = VecDeque::new(); //Reset queue
        if self.reg_interrupt_flag.get_bit(6) {
            self.parameter_queue = VecDeque::new();
        }
    }

    fn write_interrupt_enable_register(&mut self, val: u8) {
        self.reg_interrupt_enable = val & 0x1f;
    }
}

pub fn step_cycle(cpu: &mut R3000) {
    if let Some(pending_response) = &mut cpu.main_bus.cd_drive.pending_response {
        pending_response.execution_cycles -= 1;
        //println!("{}", pending_response.execution_cycles);
        if pending_response.execution_cycles == 0 {
    
            let mut packet = cpu.main_bus.cd_drive.pending_response.take().unwrap();
           
            cpu.main_bus.cd_drive.response_queue = VecDeque::with_capacity(packet.response.len()); //Clear queue
            cpu.main_bus.cd_drive.response_queue.extend(packet.response.iter());
            cpu.main_bus.cd_drive.reg_interrupt_flag = packet.cause.bitflag();
        
    
            
            //Check if interrupt enabled. If so, fire interrupt
            //println!("Interrupts {:#X} cause {:#X} command {:#X}", cpu.main_bus.cd_drive.reg_interrupt_enable, packet.cause.bitflag(), packet.command);
            if cpu.main_bus.cd_drive.reg_interrupt_enable & packet.cause.bitflag()
            == packet.cause.bitflag()
            {
                cpu.fire_external_interrupt(InterruptSource::CDROM);
            }
    
            //If the response has an extra response, push that to the front of the line
            if let Some(ext_response) = packet.extra_response.take() {
                //println!("Extra response, filling. {:?}", ext_response);
                cpu.main_bus
                .cd_drive
                .pending_response = Some(*ext_response);
            };
          
    
            
            match packet.command {
                0x15 => {
                    //Make sure this is the second response
                    if packet.extra_response.is_some() {
                        //End seek and return drive to idle state
                        cpu.main_bus.cd_drive.read_offset = 0;
                        cpu.main_bus.cd_drive.drive_state = DriveState::Idle;
                    }
                }
    
                0x6 => {
                    //ReadN
                    //println!("Post ReadN");
                  
                    if cpu.main_bus.cd_drive.read_enabled {
                        let response_packet = Packet {
                            cause: IntCause::INT1,
                            response: vec![cpu.main_bus.cd_drive.get_stat()],
                            execution_cycles: 0x36cd2,
                            extra_response: None,
                            command: 0x6,
                        };
    
                        cpu.main_bus.cd_drive.pending_response = Some(response_packet);
                    }
                }
                _ => () //No actions for this command
            };
        }
    }
}
