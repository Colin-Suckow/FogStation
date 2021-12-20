use std::{cmp::min, mem::size_of_val};

use bit_field::BitField;
use log::warn;
use nalgebra::clamp;

use crate::gpu;

#[derive(Clone, Copy)]
struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub c: u8,
}

impl Color {
    fn new(r: u8, g: u8, b: u8, c: u8) -> Self {
        Self {
            r: r,
            g: g,
            b: b,
            c: c,
        }
    }

    fn new_empty() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            c: 0,
        }
    }

    fn set_word(&mut self, val: u32) {
        self.r = (val & 0xFF) as u8;
        self.g = ((val >> 8) & 0xFF) as u8;
        self.b = ((val >> 16) & 0xFF) as u8;
        self.c = ((val >> 24) & 0xFF) as u8;
    }

    fn word(&self) -> u32 {
        (self.r as u32) | ((self.g as u32) << 8) | ((self.b as u32) << 16) | ((self.c as u32) << 24)
    }
}

#[allow(non_snake_case)]
pub(super) struct GTE {
    // Control Registers
    ZSF3: i16,
    ZSF4: i16,
    H: u16,
    DQA: i16,
    DQB: i32,
    OFX: i32,
    OFY: i32,
    RBK: i32,
    BBK: i32,
    GBK: i32,
    RFC: i32,
    GFC: i32,
    BFC: i32,
    LR1: i16,
    LR2: i16,
    LR3: i16,
    LG1: i16,
    LG2: i16,
    LG3: i16,
    LB1: i16,
    LB2: i16,
    LB3: i16,
    L11: i16,
    L12: i16,
    L13: i16,
    L21: i16,
    L22: i16,
    L23: i16,
    L31: i16,
    L32: i16,
    L33: i16,
    RT11: i16,
    RT12: i16,
    RT13: i16,
    RT21: i16,
    RT22: i16,
    RT23: i16,
    RT31: i16,
    RT32: i16,
    RT33: i16,
    TRX: i32,
    TRY: i32,
    TRZ: i32,
    FLAG: u32,
    LZCS: i32,

    // Data registers
    VX0: i16,
    VY0: i16,
    VZ0: i16,
    VX1: i16,
    VY1: i16,
    VZ1: i16,
    VX2: i16,
    VY2: i16,
    VZ2: i16,
    IR0: i16,
    IR1: i16,
    IR2: i16,
    IR3: i16,
    MAC0: i32,
    MAC1: i32,
    MAC2: i32,
    MAC3: i32,
    SZ0: u16,
    SZ1: u16,
    SZ2: u16,
    SZ3: u16,
    SX0: i16,
    SX1: i16,
    SX2: i16,
    SY0: i16,
    SY1: i16,
    SY2: i16,
    RGBC: Color,
    RGB0: Color,
    RGB1: Color,
    RGB2: Color,
    RES1: u32,
    OTZ: u16,
    IRGB: u32,
}

// Interface
impl GTE {
    pub(super) fn new() -> Self {
        Self {
            // Control Registers
            ZSF3: 0,
            ZSF4: 0,
            H: 0,
            DQA: 0,
            DQB: 0,
            OFX: 0,
            OFY: 0,
            RBK: 0,
            BBK: 0,
            GBK: 0,
            RFC: 0,
            GFC: 0,
            BFC: 0,
            LR1: 0,
            LR2: 0,
            LR3: 0,
            LG1: 0,
            LG2: 0,
            LG3: 0,
            LB1: 0,
            LB2: 0,
            LB3: 0,
            L11: 0,
            L12: 0,
            L13: 0,
            L21: 0,
            L22: 0,
            L23: 0,
            L31: 0,
            L32: 0,
            L33: 0,
            RT11: 0,
            RT12: 0,
            RT13: 0,
            RT21: 0,
            RT22: 0,
            RT23: 0,
            RT31: 0,
            RT32: 0,
            RT33: 0,
            TRX: 0,
            TRY: 0,
            TRZ: 0,
            FLAG: 0,
            LZCS: 0,

            // Data Registers
            VX0: 0,
            VY0: 0,
            VZ0: 0,
            VX1: 0,
            VY1: 0,
            VZ1: 0,
            VX2: 0,
            VY2: 0,
            VZ2: 0,
            IR0: 0,
            IR1: 0,
            IR2: 0,
            IR3: 0,
            MAC0: 0,
            MAC1: 0,
            MAC2: 0,
            MAC3: 0,
            SZ0: 0,
            SZ1: 0,
            SZ2: 0,
            SZ3: 0,
            SX0: 0,
            SX1: 0,
            SX2: 0,
            SY0: 0,
            SY1: 0,
            SY2: 0,
            RGBC: Color::new_empty(),
            RGB0: Color::new_empty(),
            RGB1: Color::new_empty(),
            RGB2: Color::new_empty(),
            RES1: 0,
            OTZ: 0,
            IRGB: 0,
        }
    }

    pub(super) fn set_control_register(&mut self, reg: usize, val: u32) {
        // println!(
        //     "Writing control reg {} (raw {}) with val {:#X}",
        //     ctrl_reg_name[reg],
        //     reg,
        //     val
        // );
        match reg {
            0 => {
                self.RT11 = val as i16;
                self.RT12 = (val >> 16) as u16 as i16;
            }
            1 => {
                self.RT13 = val as i16;
                self.RT21 = (val >> 16) as i16;
            }
            2 => {
                self.RT22 = val as i16;
                self.RT23 = (val >> 16) as i16;
            }
            3 => {
                self.RT31 = val as i16;
                self.RT32 = (val >> 16) as i16;
            }
            4 => self.RT33 = val as i16,
            5 => self.TRX = val as i32,
            6 => self.TRY = val as i32,
            7 => self.TRZ = val as i32,
            8 => {
                self.L11 = (val & 0xFFFF) as i16;
                self.L12 = ((val >> 16) & 0xFFFF) as i16;
            }
            9 => {
                self.L13 = (val & 0xFFFF) as i16;
                self.L21 = ((val >> 16) & 0xFFFF) as i16;
            }
            10 => {
                self.L22 = (val & 0xFFFF) as i16;
                self.L23 = ((val >> 16) & 0xFFFF) as i16;
            }
            11 => {
                self.L31 = (val & 0xFFFF) as i16;
                self.L32 = ((val >> 16) & 0xFFFF) as i16;
            }
            12 => self.L33 = val as i16,
            13 => self.RBK = val as i32,
            14 => self.GBK = val as i32,
            15 => self.BBK = val as i32,
            16 => {
                self.LR1 = (val & 0xFFFF) as i16;
                self.LR2 = ((val >> 16) & 0xFFFF) as i16;
            }
            17 => {
                self.LR3 = (val & 0xFFFF) as i16;
                self.LG1 = ((val >> 16) & 0xFFFF) as i16;
            }
            18 => {
                self.LG2 = (val & 0xFFFF) as i16;
                self.LG3 = ((val >> 16) & 0xFFFF) as i16;
            }
            19 => {
                self.LB1 = (val & 0xFFFF) as i16;
                self.LB2 = ((val >> 16) & 0xFFFF) as i16;
            }
            20 => self.LB3 = val as i16,
            21 => self.RFC = val as i32,
            22 => self.GFC = val as i32,
            23 => self.BFC = val as i32,
            24 => self.OFX = val as i32,
            25 => self.OFY = val as i32,
            26 => self.H = val as u16,
            27 => self.DQA = val as i16,
            28 => self.DQB = val as i32,
            29 => self.ZSF3 = val as i16,
            30 => self.ZSF4 = val as i16,
            31 => self.FLAG = *self.FLAG.set_bits(12..=30, (val >> 12) & 0x7FFFF),
            _ => panic!(
                "Tried to write unknown GTE control register {} ({} RAW)",
                CTRL_REG_NAME[reg], reg
            ),
        }
    }

    pub(super) fn set_data_register(&mut self, reg: usize, val: u32) {
        // println!(
        //     "Writing data reg {} (raw {}) with val {:#X}",
        //     data_reg_name[reg],
        //     reg,
        //     val
        // );
        match reg {
            0 => {
                self.VX0 = val as i16;
                self.VY0 = (val >> 16) as i16;
            }
            1 => self.VZ0 = val as i16,
            2 => {
                self.VX1 = val as i16;
                self.VY1 = (val >> 16) as i16;
            }
            3 => self.VZ1 = val as i16,
            4 => {
                self.VX2 = val as i16;
                self.VY2 = (val >> 16) as i16;
            }
            5 => self.VZ2 = val as i16,
            6 => self.RGBC.set_word(val),
            7 => self.OTZ = val as u16,

            8 => self.IR0 = val as i16,
            9 => self.IR1 = val as i16,
            10 => self.IR2 = val as i16,
            11 => self.IR3 = val as i16,
            12 => {
                self.SX0 = val as i16;
                self.SY0 = (val >> 16) as i16;
            }

            13 => {
                self.SX1 = val as i16;
                self.SY1 = (val >> 16) as i16;
            }

            14 => {
                self.SX2 = val as i16;
                self.SY2 = (val >> 16) as i16;
            }

            15 => {
                self.push_sx(val as i16);
                self.push_sy((val >> 16) as i16);
            }

            16 => self.SZ0 = val as u16,
            17 => self.SZ1 = val as u16,
            18 => self.SZ2 = val as u16,
            19 => self.SZ3 = val as u16,

            20 => self.RGB0.set_word(val),
            21 => self.RGB1.set_word(val),
            22 => self.RGB2.set_word(val),
            23 => self.RES1 = val,

            24 => self.MAC0 = val as i32,
            25 => self.MAC1 = val as i32,
            26 => self.MAC2 = val as i32,
            27 => self.MAC3 = val as i32,
            28 => {
                self.irgb(val);
                self.IRGB = val & 0x7FFF;
            }

            29 => (), // Can't write to ORGB

            30 => self.LZCS = val as i32,
            31 => (), //Can't write lzcr
            _ => panic!(
                "Tried to write unknown GTE data register {} ({} RAW)",
                DATA_REG_NAME[reg], reg
            ),
        }
    }

    pub(super) fn data_register(&mut self, reg: usize) -> u32 {
        let val = match reg {
            0 => (((self.VY0 as u32) << 16) | (self.VX0 as u32 & 0xFFFF)),
            1 => self.VZ0 as u32,
            2 => ((self.VY1 as u32) << 16 | (self.VX1 as u32 & 0xFFFF)),
            3 => self.VZ1 as u32,
            4 => ((self.VY2 as u32) << 16 | (self.VX2 as u32 & 0xFFFF)),
            5 => self.VZ2 as u32,
            6 => self.RGBC.word(),

            9 => self.IR1 as u32,
            10 => self.IR2 as u32,
            11 => self.IR3 as u32,

            7 => self.OTZ as u32,
            8 => self.IR0 as u32,

            12 => (self.SY0 as u32) << 16 | self.SX0 as u32 & 0xFFFF,
            13 => (self.SY1 as u32) << 16 | self.SX1 as u32 & 0xFFFF,
            14 => (self.SY2 as u32) << 16 | self.SX2 as u32 & 0xFFFF,
            15 => (self.SY2 as u32) << 16 | self.SX2 as u32 & 0xFFFF,
            16 => self.SZ0 as u32,
            17 => self.SZ1 as u32,
            18 => self.SZ2 as u32,
            19 => self.SZ3 as u32,

            20 => self.RGB0.word(),
            21 => self.RGB1.word(),
            22 => self.RGB2.word(),

            23 => self.RES1,
            24 => self.MAC0 as u32,
            25 => self.MAC1 as u32,
            26 => self.MAC2 as u32,
            27 => self.MAC3 as u32,
            28..=29 => self.orgb(),
            30 => self.LZCS as u32,
            31 => self.lzcr(),
            _ => panic!(
                "Tried to read unknown GTE data register {} ({} RAW)",
                DATA_REG_NAME[reg], reg
            ),
        };
        //println!("Reading data reg {} value {:#X}", data_reg_name[reg], val);
        val
    }

    // Control register numbers are shifted down by 32
    pub(super) fn control_register(&self, reg: usize) -> u32 {
        let val = match reg {
            0 => (((self.RT12 as u32) << 16) | (self.RT11 as u32 & 0xFFFF)),
            1 => (((self.RT21 as u32) << 16) | (self.RT13 as u32 & 0xFFFF)),
            2 => (((self.RT23 as u32) << 16) | (self.RT22 as u32 & 0xFFFF)),
            3 => (((self.RT32 as u32) << 16) | (self.RT31 as u32 & 0xFFFF)),
            4 => self.RT33 as i32 as u32,
            5 => self.TRX as u32,
            6 => self.TRY as u32,
            7 => self.TRZ as u32,

            8 => (self.L11 as u32) & 0xFFFF | ((self.L12 as u32) << 16),
            9 => (self.L13 as u32) & 0xFFFF | ((self.L21 as u32) << 16),
            10 => (self.L22 as u32) & 0xFFFF | ((self.L23 as u32) << 16),
            11 => (self.L31 as u32) & 0xFFFF | ((self.L32 as u32) << 16),

            12 => self.L33 as i32 as u32,
            13 => self.RBK as u32,
            14 => self.GBK as u32,
            15 => self.BBK as u32,

            16 => (self.LR1 as u32) & 0xFFFF | ((self.LR2 as u32) << 16),
            17 => (self.LR3 as u32) & 0xFFFF | ((self.LG1 as u32) << 16),
            18 => (self.LG2 as u32) & 0xFFFF | ((self.LG3 as u32) << 16),
            19 => (self.LB1 as u32) & 0xFFFF | ((self.LB2 as u32) << 16),

            20 => self.LB3 as i32 as u32,
            21 => self.RFC as u32,
            22 => self.GFC as u32,
            23 => self.BFC as u32,
            24 => self.OFX as u32,
            25 => self.OFY as u32,
            26 => self.H as i16 as i32 as u32, // This replicates a sign extension bug in hardware
            27 => self.DQA as u32,
            28 => self.DQB as u32,
            29 => self.ZSF3 as u32,
            30 => self.ZSF4 as u32,
            31 => {
                // Handle bit 31 error flag
                let error = (self.FLAG & 0x7F87E000) != 0;
                self.FLAG | ((error as u32) << 31)
            }
            _ => panic!(
                "Tried to read unknown GTE control register {} ({} RAW)",
                CTRL_REG_NAME[reg], reg
            ),
        };
        //println!("Reading control reg {} value {:#X}", ctrl_reg_name[reg], val);
        val
    }

    pub(super) fn execute_command(&mut self, command: u32) {
        self.FLAG = 0; // Reset calculation error flags
        match command & 0x3F {
            0x1 => self.rtps(command),
            0x6 => self.nclip(),
            0xc => self.op(command),
            0x10 => self.dpcs(command),
            0x11 => self.intpl(command),
            0x12 => self.mvmva(command),
            0x13 => self.ncds(command),
            0x14 => self.cdp(command),
            0x16 => self.ncdt(command),
            0x1b => self.nccs(command),
            0x1c => self.cc(command),
            0x1e => self.ncs(command),
            0x20 => self.nct(command),
            0x30 => self.rtpt(command),
            0x2d => self.avsz3(),
            0x2e => self.avsz4(),
            0x3f => self.ncct(command),
            _ => (),
            //_ => println!("Unknown GTE command {:#X}!", command & 0x3F)
        };
    }
}

// Register functions
impl GTE {
    fn push_sz(&mut self, val: u16) {
        self.SZ0 = self.SZ1;
        self.SZ1 = self.SZ2;
        self.SZ2 = self.SZ3;
        self.SZ3 = val;
    }

    fn push_sx(&mut self, val: i16) {
        self.SX0 = self.SX1;
        self.SX1 = self.SX2;
        self.SX2 = val;
    }

    fn push_sy(&mut self, val: i16) {
        self.SY0 = self.SY1;
        self.SY1 = self.SY2;
        self.SY2 = val;
    }

    fn push_color(&mut self, val: Color) {
        self.RGB0 = self.RGB1;
        self.RGB1 = self.RGB2;
        self.RGB2 = val;
    }

    fn lzcr(&self) -> u32 {
        if self.LZCS >= 0 {
            self.LZCS.leading_zeros()
        } else {
            self.LZCS.leading_ones()
        }
    }

    fn irgb(&mut self, val: u32) {
        let red = val & 0x1F;
        let green = (val >> 5) & 0x1F;
        let blue = (val >> 10) & 0x1F;
        self.truncate_write_ir1((red * 0x80) as i32, false);
        self.truncate_write_ir2((green * 0x80) as i32, false);
        self.truncate_write_ir3((blue * 0x80) as i32, false);
    }

    fn orgb(&mut self) -> u32 {
        let red = self.IR1 / 0x80;
        let green = self.IR2 / 0x80;
        let blue = self.IR3 / 0x80;

        (blue.clamp(0, 0x1F) << 10) as u32
            | (green.clamp(0, 0x1F) << 5) as u32
            | red.clamp(0, 0x1F) as u32
    }
}

// Internal GTE commands
impl GTE {
    fn op(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        let x = (self.IR3 as i32 * self.RT22 as i32) - (self.IR2 as i32 * self.RT33 as i32);
        let y = (self.IR1 as i32 * self.RT33 as i32) - (self.IR3 as i32 * self.RT11 as i32);
        let z = (self.IR2 as i32 * self.RT11 as i32) - (self.IR1 as i32 * self.RT22 as i32);

        self.truncate_write_mac1(x as i64, shift);
        self.truncate_write_mac2(y as i64, shift);
        self.truncate_write_mac3(z as i64, shift);

        self.truncate_write_ir1(self.MAC1, lm);
        self.truncate_write_ir2(self.MAC2, lm);
        self.truncate_write_ir3(self.MAC3, lm);
    }

    fn mvmva(&mut self, command: u32) {
        let mx = command.get_bits(17..=18);
        let vx = command.get_bits(15..=16);
        let tx = command.get_bits(13..=14);

        let (m11, m12, m13, m21, m22, m23, m31, m32, m33) = match mx {
            0 => (
                self.RT11, self.RT12, self.RT13, self.RT21, self.RT22, self.RT23, self.RT31,
                self.RT32, self.RT33,
            ),
            1 => (
                self.L11, self.L12, self.L13, self.L21, self.L22, self.L23, self.L31, self.L32,
                self.L33,
            ),
            2 => (
                self.LR1, self.LR2, self.LR3, self.LG1, self.LG2, self.LG3, self.LB1, self.LB2,
                self.LB3,
            ),
            3 => (
                -(((self.RGBC.r as u16) << 4) as i16),
                ((self.RGBC.r as u16) << 4) as i16,
                self.IR0,
                self.RT13,
                self.RT13,
                self.RT13,
                self.RT22,
                self.RT22,
                self.RT22,
            ),
            _ => panic!("Unimplemented/Unknown MVMVA matrix!"),
        };

        let (mvx, mvy, mvz) = match vx {
            0 => (self.VX0, self.VY0, self.VZ0),
            1 => (self.VX1, self.VY1, self.VZ1),
            2 => (self.VX2, self.VY2, self.VZ2),
            3 => (self.IR1, self.IR2, self.IR3),
            _ => panic!("Unimplemented/Unknown MVMVA Multiply Vector!"),
        };

        let (tvx, tvy, tvz) = match tx {
            0 => (self.TRX, self.TRY, self.TRZ),
            1 => (self.RBK, self.GBK, self.BBK),
            2 => (self.RFC, self.GFC, self.BFC),
            3 => (0, 0, 0),
            n => panic!("Unimplemented/Unknown MVMVA translation vector {}!", n),
        };
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        // tx=2 is bugged on original hardware, so we have to do a weird version of the calculation
        if tx == 2 {
            let x = ((tvx as i64) << 12) + (m11 as i64 * mvx as i64);
            let y = ((tvy as i64) << 12) + (m21 as i64 * mvx as i64);
            let z = ((tvz as i64) << 12) + (m31 as i64 * mvx as i64);

            self.truncate_write_mac1(x, shift);
            self.truncate_write_mac2(y, shift);
            self.truncate_write_mac3(z, shift);

            self.truncate_write_ir1(self.MAC1, false);
            self.truncate_write_ir2(self.MAC2, false);
            self.truncate_write_ir3(self.MAC3, false);

            let x = (m12 as i64 * mvy as i64) + (m13 as i64 * mvz as i64);
            let y = (m22 as i64 * mvy as i64) + (m23 as i64 * mvz as i64);
            let z = (m32 as i64 * mvy as i64) + (m33 as i64 * mvz as i64);

            self.truncate_write_mac1(x, shift);
            self.truncate_write_mac2(y, shift);
            self.truncate_write_mac3(z, shift);

            self.truncate_write_ir1(self.MAC1, lm);
            self.truncate_write_ir2(self.MAC2, lm);
            self.truncate_write_ir3(self.MAC3, lm);
        } else {
            // Otherwise do the correct version
            let (x, y, z) = self.mul_matrix_with_offset(
                mvx, mvy, mvz, tvx, tvy, tvz, m11 as i64, m12 as i64, m13 as i64, m21 as i64,
                m22 as i64, m23 as i64, m31 as i64, m32 as i64, m33 as i64,
            );
            self.truncate_write_mac1(x, shift);
            self.truncate_write_mac2(y, shift);
            self.truncate_write_mac3(z, shift);

            self.truncate_write_ir1(self.MAC1, lm);
            self.truncate_write_ir2(self.MAC2, lm);
            self.truncate_write_ir3(self.MAC3, lm);
        }
    }

    fn rtps(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_rtps(self.VX0, self.VY0, self.VZ0, shift, true, lm);
    }

    fn rtpt(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_rtps(self.VX0, self.VY0, self.VZ0, shift, false, lm);
        self.do_rtps(self.VX1, self.VY1, self.VZ1, shift, false, lm);
        self.do_rtps(self.VX2, self.VY2, self.VZ2, shift, true, lm);
    }

    fn nclip(&mut self) {
        self.truncate_write_mac0(
            ((self.SX0 as i64) * (self.SY1 as i64))
                + ((self.SX1 as i64) * (self.SY2 as i64))
                + ((self.SX2 as i64) * (self.SY0 as i64))
                - ((self.SX0 as i64) * (self.SY2 as i64))
                - ((self.SX1 as i64) * (self.SY0 as i64))
                - (self.SX2 as i64) * (self.SY1 as i64),
            0,
        );
    }

    fn ncds(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_ncds(self.VX0, self.VY0, self.VZ0, shift, lm);
    }

    fn ncdt(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_ncds(self.VX0, self.VY0, self.VZ0, shift, lm);
        self.do_ncds(self.VX1, self.VY1, self.VZ1, shift, lm);
        self.do_ncds(self.VX2, self.VY2, self.VZ2, shift, lm);
    }

    fn ncs(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_ncs(self.VX0, self.VY0, self.VZ0, shift, lm);
    }

    fn nct(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_ncs(self.VX0, self.VY0, self.VZ0, shift, lm);
        self.do_ncs(self.VX1, self.VY1, self.VZ1, shift, lm);
        self.do_ncs(self.VX2, self.VY2, self.VZ2, shift, lm);
    }

    fn cdp(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        // [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (BK*1000h + LCM*IR) SAR (sf*12)

        let (dot_x_color, dot_y_color, dot_z_color) = self.mul_matrix_color_with_offset(
            self.IR1, self.IR2, self.IR3, self.RBK, self.GBK, self.BBK,
        );

        self.truncate_write_mac1(dot_x_color, shift);
        self.truncate_write_mac2(dot_y_color, shift);
        self.truncate_write_mac3(dot_z_color, shift);

        self.truncate_write_ir1((dot_x_color >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_color >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_color >> shift) as i32, lm);

        // [MAC1,MAC2,MAC3] = [R*IR1,G*IR2,B*IR3] SHL 4

        self.truncate_write_mac1((self.RGBC.r as u64 as i64 * self.IR1 as i64) << 4, 0);
        self.truncate_write_mac2((self.RGBC.g as u64 as i64 * self.IR2 as i64) << 4, 0);
        self.truncate_write_mac3((self.RGBC.b as u64 as i64 * self.IR3 as i64) << 4, 0);

        self.interpolate_color(self.MAC1, self.MAC2, self.MAC3, lm, shift);

        let final_color =
            self.make_color(self.MAC1 >> 4, self.MAC2 >> 4, self.MAC3 >> 4, self.RGBC.c);

        self.push_color(final_color);
    }

    fn cc(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        // [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (BK*1000h + LCM*IR) SAR (sf*12)

        let (dot_x_color, dot_y_color, dot_z_color) = self.mul_matrix_color_with_offset(
            self.IR1, self.IR2, self.IR3, self.RBK, self.GBK, self.BBK,
        );

        self.truncate_write_mac1(dot_x_color, shift);
        self.truncate_write_mac2(dot_y_color, shift);
        self.truncate_write_mac3(dot_z_color, shift);

        self.truncate_write_ir1((dot_x_color >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_color >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_color >> shift) as i32, lm);

        // [MAC1,MAC2,MAC3] = [R*IR1,G*IR2,B*IR3] SHL 4

        self.truncate_write_mac1((self.RGBC.r as i64 * self.IR1 as i64) << 4, shift);
        self.truncate_write_mac2((self.RGBC.g as i64 * self.IR2 as i64) << 4, shift);
        self.truncate_write_mac3((self.RGBC.b as i64 * self.IR3 as i64) << 4, shift);

        self.truncate_write_ir1(self.MAC1, lm);
        self.truncate_write_ir2(self.MAC2, lm);
        self.truncate_write_ir3(self.MAC3, lm);

        let final_color =
            self.make_color(self.MAC1 >> 4, self.MAC2 >> 4, self.MAC3 >> 4, self.RGBC.c);

        self.push_color(final_color);
    }

    fn dpcs(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.truncate_write_mac1(((self.RGBC.r as u64) << 16) as i64, 0);
        self.truncate_write_mac2(((self.RGBC.g as u64) << 16) as i64, 0);
        self.truncate_write_mac3(((self.RGBC.b as u64) << 16) as i64, 0);

        self.interpolate_color(self.MAC1, self.MAC2, self.MAC3, lm, shift);

        let final_color =
            self.make_color(self.MAC1 >> 4, self.MAC2 >> 4, self.MAC3 >> 4, self.RGBC.c);

        self.push_color(final_color);
    }

    fn intpl(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.truncate_write_mac1((self.IR1 as i64) << 12, 0);
        self.truncate_write_mac2((self.IR2 as i64) << 12, 0);
        self.truncate_write_mac3((self.IR3 as i64) << 12, 0);

        self.interpolate_color(self.MAC1, self.MAC2, self.MAC3, lm, shift);

        let final_color =
            self.make_color(self.MAC1 >> 4, self.MAC2 >> 4, self.MAC3 >> 4, self.RGBC.c);

        self.push_color(final_color);
    }

    fn nccs(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_nccs(self.VX0, self.VY0, self.VZ0, shift, lm);
    }

    fn ncct(&mut self, command: u32) {
        let shift = (command.get_bit(19) as usize) * 12;
        let lm = command.get_bit(10);

        self.do_nccs(self.VX0, self.VY0, self.VZ0, shift, lm);
        self.do_nccs(self.VX1, self.VY1, self.VZ1, shift, lm);
        self.do_nccs(self.VX2, self.VY2, self.VZ2, shift, lm);
    }

    // fn nct(&mut self) {
    //     warn!("Stubbing colors for now");
    //     self.RGB2 = self.RGBC.clone();
    // }

    // fn ncs(&mut self) {
    //     warn!("Stubbing colors for now");
    //     self.RGB2 = self.RGBC.clone();
    // }

    fn avsz3(&mut self) {
        let result =
            (self.ZSF3 as i64) * ((self.SZ1 as u32) + (self.SZ2 as u32) + (self.SZ3 as u32)) as i64;

        self.truncate_write_mac0(result, 0);

        self.truncate_write_otz(result >> 12);
    }

    fn avsz4(&mut self) {
        let result = (self.ZSF3 as i64)
            * ((self.SZ0 as u32) + (self.SZ1 as u32) + (self.SZ2 as u32) + (self.SZ3 as u32))
                as i64;

        self.truncate_write_mac0(result, 0);

        self.truncate_write_otz(result >> 12);
    }
}

// Command helper functions
impl GTE {
    fn interpolate_color(
        &mut self,
        in_mac1: i32,
        in_mac2: i32,
        in_mac3: i32,
        lm: bool,
        shift: usize,
    ) {
        let cx = ((self.RFC as i64) << 12) - in_mac1 as i64;
        let cy = ((self.GFC as i64) << 12) - in_mac2 as i64;
        let cz = ((self.BFC as i64) << 12) - in_mac3 as i64;

        self.truncate_write_mac1(cx, shift);
        self.truncate_write_mac2(cy, shift);
        self.truncate_write_mac3(cz, shift);

        self.truncate_write_ir1((cx >> shift) as i32, false);
        self.truncate_write_ir2((cy >> shift) as i32, false);
        self.truncate_write_ir3((cz >> shift) as i32, false);

        self.truncate_write_mac1(self.IR1 as i64 * self.IR0 as i64 + in_mac1 as i64, shift);
        self.truncate_write_mac2(self.IR2 as i64 * self.IR0 as i64 + in_mac2 as i64, shift);
        self.truncate_write_mac3(self.IR3 as i64 * self.IR0 as i64 + in_mac3 as i64, shift);

        self.truncate_write_ir1(self.MAC1, lm);
        self.truncate_write_ir2(self.MAC2, lm);
        self.truncate_write_ir3(self.MAC3, lm);
    }

    fn make_color(&mut self, r: i32, g: i32, b: i32, c: u8) -> Color {
        if !(0..=0xFF).contains(&r) {
            self.FLAG.set_bit(21, true);
        }

        if !(0..=0xFF).contains(&g) {
            self.FLAG.set_bit(20, true);
        }

        if !(0..=0xFF).contains(&b) {
            self.FLAG.set_bit(19, true);
        }

        return Color::new(
            clamp(r, 0, 0xFF) as u8,
            clamp(g, 0, 0xFF) as u8,
            clamp(b, 0, 0xFF) as u8,
            c,
        );
    }

    fn do_rtps(&mut self, vx: i16, vy: i16, vz: i16, shift: usize, last: bool, lm: bool) {
        let (x, y, z) = self.mul_matrix_rt_with_offset(vx, vy, vz, self.TRX, self.TRY, self.TRZ);

        self.truncate_write_mac1(x, shift);
        self.truncate_write_mac2(y, shift);
        self.truncate_write_mac3(z, shift);
        self.truncate_write_ir1(self.MAC1, lm);
        self.truncate_write_ir2(self.MAC2, lm);

        // This is just to lazily set the error flags
        self.truncate_write_ir3((z >> 12) as i32, false);

        // This actually sets ir3 to the unshifted mac3 value
        self.IR3 = match (self.MAC3 as i64, lm) {
            (val, true) if val < 0 => 0,
            (val, false) if val < -0x8000 => -0x8000,
            (val, _) if val > 0x7FFF => 0x7FFF,
            (val, _) => val as i16,
        };

        self.truncate_push_sz3((z >> 12) as i32);

        //println!("sz3 {}", self.SZ3);

        let div_val = unr_divide(self.H as u32, self.SZ3 as u32, &mut self.FLAG) as i64;

        let sx = div_val * self.IR1 as i64 + self.OFX as i64;
        self.truncate_write_mac0(sx, 0);
        self.saturate_push_sx(sx >> 16);

        let sy = div_val * self.IR2 as i64 + self.OFY as i64;
        self.truncate_write_mac0(sy, 0);
        self.saturate_push_sy(sy >> 16);

        if last {
            let depth = div_val * self.DQA as i64 + self.DQB as i64;
            self.truncate_write_mac0(depth, 0);
            let mut ir0_result = depth >> 12;
            if ir0_result < 0 {
                ir0_result = 0;
                self.FLAG.set_bit(12, true);
            }

            if ir0_result > 0x1000 {
                ir0_result = 0x1000;
                self.FLAG.set_bit(12, true);
            }
            self.IR0 = ir0_result as i16;
        }
    }

    fn do_ncds(&mut self, vx: i16, vy: i16, vz: i16, shift: usize, lm: bool) {
        //  [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (LLM*V0) SAR (sf*12)

        let (dot_x_light, dot_y_light, dot_z_light) = self.mul_matrix_light(vx, vy, vz);

        self.truncate_write_mac1(dot_x_light, shift);
        self.truncate_write_mac2(dot_y_light, shift);
        self.truncate_write_mac3(dot_z_light, shift);

        self.truncate_write_ir1((dot_x_light >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_light >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_light >> shift) as i32, lm);

        // [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (BK*1000h + LCM*IR) SAR (sf*12)

        let (dot_x_color, dot_y_color, dot_z_color) = self.mul_matrix_color_with_offset(
            self.IR1, self.IR2, self.IR3, self.RBK, self.GBK, self.BBK,
        );

        self.truncate_write_mac1(dot_x_color, shift);
        self.truncate_write_mac2(dot_y_color, shift);
        self.truncate_write_mac3(dot_z_color, shift);

        self.truncate_write_ir1((dot_x_color >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_color >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_color >> shift) as i32, lm);

        // [MAC1,MAC2,MAC3] = [R*IR1,G*IR2,B*IR3] SHL 4

        self.truncate_write_mac1((self.RGBC.r as u64 as i64 * self.IR1 as i64) << 4, 0);
        self.truncate_write_mac2((self.RGBC.g as u64 as i64 * self.IR2 as i64) << 4, 0);
        self.truncate_write_mac3((self.RGBC.b as u64 as i64 * self.IR3 as i64) << 4, 0);

        self.interpolate_color(self.MAC1, self.MAC2, self.MAC3, lm, shift);

        let final_color =
            self.make_color(self.MAC1 >> 4, self.MAC2 >> 4, self.MAC3 >> 4, self.RGBC.c);

        self.push_color(final_color);
    }

    fn do_nccs(&mut self, vx: i16, vy: i16, vz: i16, shift: usize, lm: bool) {
        //  [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (LLM*V0) SAR (sf*12)
        let (dot_x_light, dot_y_light, dot_z_light) = self.mul_matrix_light(vx, vy, vz);

        self.truncate_write_mac1(dot_x_light, shift);
        self.truncate_write_mac2(dot_y_light, shift);
        self.truncate_write_mac3(dot_z_light, shift);

        self.truncate_write_ir1((dot_x_light >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_light >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_light >> shift) as i32, lm);

        // [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (BK*1000h + LCM*IR) SAR (sf*12)

        let (dot_x_color, dot_y_color, dot_z_color) = self.mul_matrix_color_with_offset(
            self.IR1, self.IR2, self.IR3, self.RBK, self.GBK, self.BBK,
        );

        self.truncate_write_mac1(dot_x_color, shift);
        self.truncate_write_mac2(dot_y_color, shift);
        self.truncate_write_mac3(dot_z_color, shift);

        self.truncate_write_ir1((dot_x_color >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_color >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_color >> shift) as i32, lm);

        // [MAC1,MAC2,MAC3] = [R*IR1,G*IR2,B*IR3] SHL 4

        self.truncate_write_mac1((self.RGBC.r as i64 * self.IR1 as i64) << 4, shift);
        self.truncate_write_mac2((self.RGBC.g as i64 * self.IR2 as i64) << 4, shift);
        self.truncate_write_mac3((self.RGBC.b as i64 * self.IR3 as i64) << 4, shift);

        self.truncate_write_ir1(self.MAC1, lm);
        self.truncate_write_ir2(self.MAC2, lm);
        self.truncate_write_ir3(self.MAC3, lm);

        let final_color =
            self.make_color(self.MAC1 >> 4, self.MAC2 >> 4, self.MAC3 >> 4, self.RGBC.c);

        self.push_color(final_color);
    }

    fn do_ncs(&mut self, vx: i16, vy: i16, vz: i16, shift: usize, lm: bool) {
        //  [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (LLM*V0) SAR (sf*12)
        let (dot_x_light, dot_y_light, dot_z_light) = self.mul_matrix_light(vx, vy, vz);

        self.truncate_write_mac1(dot_x_light, shift);
        self.truncate_write_mac2(dot_y_light, shift);
        self.truncate_write_mac3(dot_z_light, shift);

        self.truncate_write_ir1((dot_x_light >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_light >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_light >> shift) as i32, lm);

        // [IR1,IR2,IR3] = [MAC1,MAC2,MAC3] = (BK*1000h + LCM*IR) SAR (sf*12)

        let (dot_x_color, dot_y_color, dot_z_color) = self.mul_matrix_color_with_offset(
            self.IR1, self.IR2, self.IR3, self.RBK, self.GBK, self.BBK,
        );

        self.truncate_write_mac1(dot_x_color, shift);
        self.truncate_write_mac2(dot_y_color, shift);
        self.truncate_write_mac3(dot_z_color, shift);

        self.truncate_write_ir1((dot_x_color >> shift) as i32, lm);
        self.truncate_write_ir2((dot_y_color >> shift) as i32, lm);
        self.truncate_write_ir3((dot_z_color >> shift) as i32, lm);

        let final_color =
            self.make_color(self.MAC1 >> 4, self.MAC2 >> 4, self.MAC3 >> 4, self.RGBC.c);

        self.push_color(final_color);
    }

    fn truncate_write_otz(&mut self, val: i64) {
        let (new_val, error) = match val {
            x if x > 0xFFFF => (0xFFFF, true),
            x if x < 0 => (0, true),
            x => (x as u16, false),
        };

        self.OTZ = new_val;
        self.FLAG.set_bit(18, error);
    }

    fn truncate_write_mac0(&mut self, val: i64, shift: usize) {
        match val {
            x if x > (i32::MAX as i64) => {
                self.FLAG.set_bit(16, true);
            }
            x if x < (i32::MIN as i64) => {
                self.FLAG.set_bit(15, true);
            }
            _ => (),
        };
        self.MAC0 = (val >> shift) as i32;
    }

    fn saturate_push_sx(&mut self, val: i64) {
        let new_val = match val {
            v if v < -0x400 => {
                self.FLAG.set_bit(14, true);
                -0x400
            }
            v if v > 0x3FF => {
                self.FLAG.set_bit(14, true);
                0x3FF
            }
            v => v,
        };

        self.push_sx(new_val as i16);
    }

    fn saturate_push_sy(&mut self, val: i64) {
        let new_val = match val {
            v if v < -0x400 => {
                self.FLAG.set_bit(13, true);
                -0x400
            }
            v if v > 0x3FF => {
                self.FLAG.set_bit(13, true);
                0x3FF
            }
            v => v,
        };
        self.push_sy(new_val as i16);
    }

    fn truncate_push_sz3(&mut self, val: i32) {
        let (new_val, error) = match val {
            x if x > 0xFFFF => (0xFFFF, true),
            x if x < 0 => (0, true),
            x => (x as u16, false),
        };

        self.push_sz(new_val);
        self.FLAG.set_bit(18, error);
    }

    fn truncate_write_mac1(&mut self, val: i64, shift: usize) {
        match val {
            x if x > (0x7ffffffffff) => {
                self.FLAG.set_bit(30, true);
            }
            x if x < (-0x80000000000) => {
                self.FLAG.set_bit(27, true);
            }
            _ => (),
        };
        self.MAC1 = (val >> shift) as u64 as i32;
    }

    fn truncate_write_mac2(&mut self, val: i64, shift: usize) {
        match val {
            x if x > (0x7ffffffffff) => {
                self.FLAG.set_bit(29, true);
            }
            x if x < (-0x80000000000) => {
                self.FLAG.set_bit(26, true);
            }
            _ => (),
        };
        self.MAC2 = (val >> shift) as u64 as i32;
    }

    fn truncate_write_mac3(&mut self, val: i64, shift: usize) {
        match val {
            x if x > (0x7ffffffffff) => {
                self.FLAG.set_bit(28, true);
            }
            x if x < (-0x80000000000) => {
                self.FLAG.set_bit(25, true);
            }
            _ => (),
        };
        self.MAC3 = (val >> shift) as u64 as i32;
    }

    fn truncate_write_ir1(&mut self, val: i32, lm_set: bool) {
        self.IR1 = match (val, lm_set) {
            (val, true) if val < 0 => {
                self.FLAG.set_bit(24, true);
                0
            }
            (val, false) if val < -0x8000 => {
                self.FLAG.set_bit(24, true);
                -0x8000
            }
            (val, _) if val > 0x7FFF => {
                self.FLAG.set_bit(24, true);
                0x7FFF
            }
            _ => val as i16,
        };
    }

    fn truncate_write_ir2(&mut self, val: i32, lm_set: bool) {
        self.IR2 = match (val, lm_set) {
            (val, true) if val < 0 => {
                self.FLAG.set_bit(23, true);
                0
            }
            (val, false) if val < -0x8000 => {
                self.FLAG.set_bit(23, true);
                -0x8000
            }
            (val, _) if val > 0x7FFF => {
                self.FLAG.set_bit(23, true);
                0x7FFF
            }
            _ => val as i16,
        }
    }

    fn truncate_write_ir3(&mut self, val: i32, lm_set: bool) {
        self.IR3 = match (val, lm_set) {
            (val, true) if val < 0 => {
                self.FLAG.set_bit(22, true);
                0
            }
            (val, false) if val < -0x8000 => {
                self.FLAG.set_bit(22, true);
                -0x8000
            }
            (val, _) if val > 0x7FFF => {
                self.FLAG.set_bit(22, true);
                0x7FFF
            }
            _ => val as i16,
        }
    }

    fn mul_matrix_rt_with_offset(
        &mut self,
        vx: i16,
        vy: i16,
        vz: i16,
        ox: i32,
        oy: i32,
        oz: i32,
    ) -> (i64, i64, i64) {
        self.mul_matrix_with_offset(
            vx,
            vy,
            vz,
            ox,
            oy,
            oz,
            self.RT11 as i64,
            self.RT12 as i64,
            self.RT13 as i64,
            self.RT21 as i64,
            self.RT22 as i64,
            self.RT23 as i64,
            self.RT31 as i64,
            self.RT32 as i64,
            self.RT33 as i64,
        )
    }

    fn mul_matrix_color_with_offset(
        &mut self,
        vx: i16,
        vy: i16,
        vz: i16,
        ox: i32,
        oy: i32,
        oz: i32,
    ) -> (i64, i64, i64) {
        self.mul_matrix_with_offset(
            vx,
            vy,
            vz,
            ox,
            oy,
            oz,
            self.LR1 as i64,
            self.LR2 as i64,
            self.LR3 as i64,
            self.LG1 as i64,
            self.LG2 as i64,
            self.LG3 as i64,
            self.LB1 as i64,
            self.LB2 as i64,
            self.LB3 as i64,
        )
    }

    fn mul_matrix_light(&mut self, vx: i16, vy: i16, vz: i16) -> (i64, i64, i64) {
        self.mul_matrix(
            vx,
            vy,
            vz,
            self.L11 as i64,
            self.L12 as i64,
            self.L13 as i64,
            self.L21 as i64,
            self.L22 as i64,
            self.L23 as i64,
            self.L31 as i64,
            self.L32 as i64,
            self.L33 as i64,
        )
    }

    fn mul_matrix(
        &mut self,
        vx: i16,
        vy: i16,
        vz: i16,
        m11: i64,
        m12: i64,
        m13: i64,
        m21: i64,
        m22: i64,
        m23: i64,
        m31: i64,
        m32: i64,
        m33: i64,
    ) -> (i64, i64, i64) {
        let sub_x = self.i64_to_i44(
            (m12 as i64) * (vy as i64) + ((m11 as i64) * vx as i64),
            MAC::One,
        );
        let x = self.i64_to_i44(((m13 as i64) * (vz as i64)) + sub_x, MAC::One);

        let sub_y = self.i64_to_i44(
            (m22 as i64) * (vy as i64) + ((m21 as i64) * vx as i64),
            MAC::Two,
        );
        let y = self.i64_to_i44(((m23 as i64) * (vz as i64)) + sub_y, MAC::Two);

        let sub_z = self.i64_to_i44(
            (m32 as i64) * (vy as i64) + ((m31 as i64) * vx as i64),
            MAC::Three,
        );
        let z = self.i64_to_i44(((m33 as i64) * (vz as i64)) + sub_z, MAC::Three);

        (x, y, z)
    }

    fn mul_matrix_with_offset(
        &mut self,
        vx: i16,
        vy: i16,
        vz: i16,
        ox: i32,
        oy: i32,
        oz: i32,
        m11: i64,
        m12: i64,
        m13: i64,
        m21: i64,
        m22: i64,
        m23: i64,
        m31: i64,
        m32: i64,
        m33: i64,
    ) -> (i64, i64, i64) {
        let sub_x = self.i64_to_i44(((m11 as i64) * vx as i64) + ((ox as i64) << 12), MAC::One);
        let sub_x = self.i64_to_i44((m12 as i64) * (vy as i64) + sub_x, MAC::One);
        let x = self.i64_to_i44(((m13 as i64) * (vz as i64)) + sub_x, MAC::One);

        let sub_y = self.i64_to_i44(((m21 as i64) * vx as i64) + ((oy as i64) << 12), MAC::Two);
        let sub_y = self.i64_to_i44((m22 as i64) * (vy as i64) + sub_y, MAC::Two);
        let y = self.i64_to_i44(((m23 as i64) * (vz as i64)) + sub_y, MAC::Two);

        let sub_z = self.i64_to_i44(((m31 as i64) * vx as i64) + ((oz as i64) << 12), MAC::Three);
        let sub_z = self.i64_to_i44((m32 as i64) * (vy as i64) + sub_z, MAC::Three);
        let z = self.i64_to_i44(((m33 as i64) * (vz as i64)) + sub_z, MAC::Three);

        (x, y, z)
    }

    fn i64_to_i44(&mut self, val: i64, mac: MAC) -> i64 {
        let (gt_bit, lt_bit) = match mac {
            MAC::One => (30, 27),
            MAC::Two => (29, 26),
            MAC::Three => (28, 25),
        };

        // Check for mac overflow
        match val {
            x if x > (0x7ffffffffff) => {
                self.FLAG.set_bit(gt_bit, true);
            }
            x if x < (-0x80000000000) => {
                self.FLAG.set_bit(lt_bit, true);
            }
            _ => (),
        };

        sign_extend(val, 44)
    }
}

// TODO: Combine with gpu.rs implementation
fn sign_extend(x: i64, nbits: u32) -> i64 {
    let notherbits = size_of_val(&x) as u32 * 8 - nbits;
    x.wrapping_shl(notherbits).wrapping_shr(notherbits)
}

enum MAC {
    One,
    Two,
    Three,
}

// Copy of duckstation's implementation
fn unr_divide(lhs: u32, rhs: u32, flag: &mut u32) -> u32 {
    if lhs < rhs * 2 {
        let shift = (rhs as u16).leading_zeros();
        let lhs_shift = lhs << shift;
        let rhs_shift = rhs << shift;
        let divisor = rhs_shift | 0x8000;
        let x: i32 = 0x101 + UNR_TABLE[(((divisor & 0x7FFF) + 0x40) >> 7) as usize] as i32;
        let d: i32 = ((divisor as i32 * -x) + 0x80) >> 8;
        let recip = ((x * (0x20000 + d) + 0x80) >> 8) as u32;
        let result = ((lhs_shift as u64 * recip as u64) + 0x8000) >> 16;
        return min(0x1FFFF, result as u32);
    } else {
        flag.set_bit(17, true);
        return 0x1FFFF;
    }
}

const UNR_TABLE: [u32; 0x101] = [
    0xFF, 0xFD, 0xFB, 0xF9, 0xF7, 0xF5, 0xF3, 0xF1, 0xEF, 0xEE, 0xEC, 0xEA, 0xE8, 0xE6, 0xE4, 0xE3,
    0xE1, 0xDF, 0xDD, 0xDC, 0xDA, 0xD8, 0xD6, 0xD5, 0xD3, 0xD1, 0xD0, 0xCE, 0xCD, 0xCB, 0xC9, 0xC8,
    0xC6, 0xC5, 0xC3, 0xC1, 0xC0, 0xBE, 0xBD, 0xBB, 0xBA, 0xB8, 0xB7, 0xB5, 0xB4, 0xB2, 0xB1, 0xB0,
    0xAE, 0xAD, 0xAB, 0xAA, 0xA9, 0xA7, 0xA6, 0xA4, 0xA3, 0xA2, 0xA0, 0x9F, 0x9E, 0x9C, 0x9B, 0x9A,
    0x99, 0x97, 0x96, 0x95, 0x94, 0x92, 0x91, 0x90, 0x8F, 0x8D, 0x8C, 0x8B, 0x8A, 0x89, 0x87, 0x86,
    0x85, 0x84, 0x83, 0x82, 0x81, 0x7F, 0x7E, 0x7D, 0x7C, 0x7B, 0x7A, 0x79, 0x78, 0x77, 0x75, 0x74,
    0x73, 0x72, 0x71, 0x70, 0x6F, 0x6E, 0x6D, 0x6C, 0x6B, 0x6A, 0x69, 0x68, 0x67, 0x66, 0x65, 0x64,
    0x63, 0x62, 0x61, 0x60, 0x5F, 0x5E, 0x5D, 0x5D, 0x5C, 0x5B, 0x5A, 0x59, 0x58, 0x57, 0x56, 0x55,
    0x54, 0x53, 0x53, 0x52, 0x51, 0x50, 0x4F, 0x4E, 0x4D, 0x4D, 0x4C, 0x4B, 0x4A, 0x49, 0x48, 0x48,
    0x47, 0x46, 0x45, 0x44, 0x43, 0x43, 0x42, 0x41, 0x40, 0x3F, 0x3F, 0x3E, 0x3D, 0x3C, 0x3C, 0x3B,
    0x3A, 0x39, 0x39, 0x38, 0x37, 0x36, 0x36, 0x35, 0x34, 0x33, 0x33, 0x32, 0x31, 0x31, 0x30, 0x2F,
    0x2E, 0x2E, 0x2D, 0x2C, 0x2C, 0x2B, 0x2A, 0x2A, 0x29, 0x28, 0x28, 0x27, 0x26, 0x26, 0x25, 0x24,
    0x24, 0x23, 0x22, 0x22, 0x21, 0x20, 0x20, 0x1F, 0x1E, 0x1E, 0x1D, 0x1D, 0x1C, 0x1B, 0x1B, 0x1A,
    0x19, 0x19, 0x18, 0x18, 0x17, 0x16, 0x16, 0x15, 0x15, 0x14, 0x14, 0x13, 0x12, 0x12, 0x11, 0x11,
    0x10, 0x0F, 0x0F, 0x0E, 0x0E, 0x0D, 0x0D, 0x0C, 0x0C, 0x0B, 0x0A, 0x0A, 0x09, 0x09, 0x08, 0x08,
    0x07, 0x07, 0x06, 0x06, 0x05, 0x05, 0x04, 0x04, 0x03, 0x03, 0x02, 0x02, 0x01, 0x01, 0x00, 0x00,
    0x00, // one extra table entry (for "(d-7FC0h)/80h"=100h)
];

const DATA_REG_NAME: [&str; 32] = [
    "vxy0", "vz0", "vxy1", "vz1", "vxy2", "vz2", "rgb", "otz", // 00
    "ir0", "ir1", "ir2", "ir3", "sxy0", "sxy1", "sxy2", "sxyp", // 08
    "sz0", "sz1", "sz2", "sz3", "rgb0", "rgb1", "rgb2", "res1", // 10
    "mac0", "mac1", "mac2", "mac3", "irgb", "orgb", "lzcs", "lzcr", // 18
];

const CTRL_REG_NAME: [&str; 32] = [
    "r11r12", "r13r21", "r22r23", "r31r32", "r33", "trx", "try", "trz", // 00
    "l11l12", "l13l21", "l22l23", "l31l32", "l33", "rbk", "gbk", "bbk", // 08
    "lr1lr2", "lr3lg1", "lg2lg3", "lb1lb2", "lb3", "rfc", "gfc", "bfc", // 10
    "ofx", "ofy", "h", "dqa", "dqb", "zsf3", "zsf4", "flag", // 18
];
