use std::{f64::consts::PI, mem::size_of_val};

use bit_field::BitField;
use byteorder::{ByteOrder, LittleEndian};

use super::MdecCommand;

const END_CODE: u16 = 0xFE00;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ColorDepth {
    B4 = 0,
    B8 = 1,
    B24 = 2,
    B15 = 3,
}
#[derive(Clone, Copy)]
pub(crate) struct DecodeMacroblockCommand {
    depth: ColorDepth,
    signed: bool,
    set_b15: bool,
    size: usize,
}

impl DecodeMacroblockCommand {
    pub(crate) fn new(command_word: u32) -> Self {
        if command_word >> 29 != 1 {
            panic!(
                "Not a decode_macroblock command! Command number = {}",
                command_word >> 29
            );
        }

        let depth = match (command_word >> 27) & 3 {
            0 => ColorDepth::B4,
            1 => ColorDepth::B8,
            2 => ColorDepth::B24,
            3 => ColorDepth::B15,
            _ => unreachable!(),
        };

        let signed = command_word.get_bit(26);
        let set_b15 = command_word.get_bit(25);
        let size = (command_word & 0xFFFF) as usize;
        //println!("Color depth {}", (command_word >> 27) & 3);
        //println!("MACROBLOCK size: {}", size);

        Self {
            depth,
            signed,
            set_b15,
            size,
        }
    }
}

impl MdecCommand for DecodeMacroblockCommand {
    fn parameter_words(&self) -> usize {
        self.size
    }

    fn execute(&self, ctx: &mut super::MDEC) {
        let mut parameters = vec![];
        for w in &ctx.parameter_buffer {
            parameters.push((w & 0xFFFF) as u16);
            parameters.push((w >> 16) as u16);
        }

        let mut decoder = MacroblockDecoder::new();

        for parameter in parameters {
            if decoder.complete() {
                decoder.print_stats();
                let decoded_block = decoder.decode(ctx, &self.depth);
                ctx.result_buffer.extend(decoded_block);
                decoder = MacroblockDecoder::new();
            }
            decoder.push_value(parameter);
        }
        //println!("Done");
    }

    fn box_clone(&self) -> Box<dyn MdecCommand> {
        Box::new((*self).clone())
    }

    fn name(&self) -> &str {
        "DecodeMacroblock"
    }

    fn set_status(&self, status: &mut u32) {
        *status |= match self.depth {
            ColorDepth::B4 => 0,
            ColorDepth::B8 => 1,
            ColorDepth::B24 => 2,
            ColorDepth::B15 => 3,
        } << 25;

        status.set_bit(24, self.signed);
        status.set_bit(23, self.set_b15);
    }
}

#[derive(Debug, PartialEq)]
enum MacroblockBlock {
    Cr,
    Cb,
    Y1,
    Y2,
    Y3,
    Y4,
}

#[derive(PartialEq, Debug)]
enum DecodeState {
    Waiting,
    ReceiveDC,
    ReceiveAC,
    Complete,
}

struct MacroblockDecoder {
    current_block: MacroblockBlock,
    current_decode: DecodeState,
    rlc_index: usize,

    cr_block: Vec<u16>,
    cb_block: Vec<u16>,
    y1_block: Vec<u16>,
    y2_block: Vec<u16>,
    y3_block: Vec<u16>,
    y4_block: Vec<u16>,
}

impl MacroblockDecoder {
    fn new() -> Self {
        Self {
            current_block: MacroblockBlock::Cr,
            current_decode: DecodeState::Waiting,
            rlc_index: 0,

            cr_block: vec![],
            cb_block: vec![],
            y1_block: vec![],
            y2_block: vec![],
            y3_block: vec![],
            y4_block: vec![],
        }
    }

    fn push_value(&mut self, value: u16) {
        match self.current_decode {
            DecodeState::Waiting => {
                if value != END_CODE {
                    self.current_decode = DecodeState::ReceiveDC;
                    self.working_block_mut().push(value);
                }
            }
            DecodeState::ReceiveDC => {
                if value == END_CODE {
                    self.current_decode = DecodeState::Waiting;
                    self.increment_block();
                } else {
                    self.rlc_index += 1 + (value >> 10) as usize;
                    self.working_block_mut().push(value);

                    self.current_decode = DecodeState::ReceiveAC;

                    if self.rlc_index == 63 {
                        self.rlc_index = 0;
                        self.current_decode = DecodeState::Waiting;
                        self.increment_block();
                    }
                }
            }
            DecodeState::ReceiveAC => {
                if value == END_CODE {
                    self.rlc_index = 0;
                    self.current_decode = DecodeState::Waiting;
                    self.increment_block();
                } else if self.rlc_index < 63 {
                    self.rlc_index += 1 + (value >> 10) as usize;
                    self.working_block_mut().push(value);
                }

                if self.rlc_index == 63 {
                    self.rlc_index = 0;
                    self.current_decode = DecodeState::Waiting;
                    self.increment_block();
                }
            }
            DecodeState::Complete => {
                panic!(
                    "Tried to push value to complete macroblock decoder! value: {:#X}",
                    value
                );
            }
        }
    }

    fn block_data(&self, block: MacroblockBlock) -> &Vec<u16> {
        match block {
            MacroblockBlock::Cr => &self.cr_block,
            MacroblockBlock::Cb => &self.cb_block,
            MacroblockBlock::Y1 => &self.y1_block,
            MacroblockBlock::Y2 => &self.y2_block,
            MacroblockBlock::Y3 => &self.y3_block,
            MacroblockBlock::Y4 => &self.y4_block,
        }
    }

    fn working_block_mut(&mut self) -> &mut Vec<u16> {
        match self.current_block {
            MacroblockBlock::Cr => &mut self.cr_block,
            MacroblockBlock::Cb => &mut self.cb_block,
            MacroblockBlock::Y1 => &mut self.y1_block,
            MacroblockBlock::Y2 => &mut self.y2_block,
            MacroblockBlock::Y3 => &mut self.y3_block,
            MacroblockBlock::Y4 => &mut self.y4_block,
        }
    }

    fn increment_block(&mut self) {
        self.current_block = match self.current_block {
            MacroblockBlock::Cr => MacroblockBlock::Cb,
            MacroblockBlock::Cb => MacroblockBlock::Y1,
            MacroblockBlock::Y1 => MacroblockBlock::Y2,
            MacroblockBlock::Y2 => MacroblockBlock::Y3,
            MacroblockBlock::Y3 => MacroblockBlock::Y4,
            MacroblockBlock::Y4 => {
                self.current_decode = DecodeState::Complete;
                MacroblockBlock::Y4
            }
        };
    }

    fn complete(&self) -> bool {
        self.current_decode == DecodeState::Complete
    }

    fn print_stats(&self) {
        //println!("State: {:?}", self.current_decode);
        //println!("cr_len {}", self.cr_block.len());
        //println!("cb_len {}", self.cb_block.len());
        //println!("y1_len {}", self.y1_block.len());
        //println!("y2_len {}", self.y2_block.len());
        //println!("y3_len {}", self.y3_block.len());
        //println!("y4_len {}", self.y4_block.len());
    }

    fn decode(&self, ctx: &super::MDEC, color_depth: &ColorDepth) -> Vec<u32> {
        let decoded_cr = decode_block(ctx, self.block_data(MacroblockBlock::Cr), true);
        let decoded_cb = decode_block(ctx, self.block_data(MacroblockBlock::Cb), true);
        let decoded_y1 = decode_block(ctx, self.block_data(MacroblockBlock::Y1), false);
        let decoded_y2 = decode_block(ctx, self.block_data(MacroblockBlock::Y2), false);
        let decoded_y3 = decode_block(ctx, self.block_data(MacroblockBlock::Y3), false);
        let decoded_y4 = decode_block(ctx, self.block_data(MacroblockBlock::Y4), false);

        // Combine blocks into (Y, Cb, Cr) pixels

        let mut chroma_block: Vec<(f32, f32, f32)> = vec![(0.0, 0.0, 0.0); 16 * 16];

        fn loc_px(x: i32, y: i32) -> usize {
            (y * 16 + x) as usize
        }

        fn loc_bk(x: i32, y: i32) -> usize {
            (y * 8 + x) as usize
        }

        for x in 0..8 {
            for y in 0..8 {
                chroma_block[loc_px(x, y)].0 = decoded_y1[loc_bk(x, y)] + 128.0;
                chroma_block[loc_px(x + 8, y)].0 = decoded_y2[loc_bk(x, y)] + 128.0;
                chroma_block[loc_px(x, y + 8)].0 = decoded_y3[loc_bk(x, y)] + 128.0;
                chroma_block[loc_px(x + 8, y + 8)].0 = decoded_y4[loc_bk(x, y)] + 128.0;

                chroma_block[loc_px(x * 2, y * 2)].1 = decoded_cb[loc_bk(x, y)];
                chroma_block[loc_px(x * 2 + 1, y * 2)].1 = decoded_cb[loc_bk(x, y)];
                chroma_block[loc_px(x * 2, y * 2 + 1)].1 = decoded_cb[loc_bk(x, y)];
                chroma_block[loc_px(x * 2 + 1, y * 2 + 1)].1 = decoded_cb[loc_bk(x, y)];

                chroma_block[loc_px(x * 2, y * 2)].2 = decoded_cr[loc_bk(x, y)];
                chroma_block[loc_px(x * 2 + 1, y * 2)].2 = decoded_cr[loc_bk(x, y)];
                chroma_block[loc_px(x * 2, y * 2 + 1)].2 = decoded_cr[loc_bk(x, y)];
                chroma_block[loc_px(x * 2 + 1, y * 2 + 1)].2 = decoded_cr[loc_bk(x, y)];
            }
        }

        //println!("chroma {:?}", chroma_block);

        // Convert to rgb

        let rgb_block: Vec<(u8, u8, u8)> = chroma_block
            .iter()
            .map(|(y, cb, cr)| {
                let red = (y + 1.402 * cr).clamp(0.0, 255.0) as u8;
                let green = (y - (0.3437 * cb) - (0.7143 * cr)).clamp(0.0, 255.0) as u8;
                let blue = (y + 1.772 * cb).clamp(0.0, 255.0) as u8;
                (red, green, blue)
            })
            .collect();

        // TODO do the real decoding
        match color_depth {
            ColorDepth::B4 => todo!(),
            ColorDepth::B8 => todo!(),
            ColorDepth::B24 => {
                let bytes: Vec<u8> = rgb_block.iter().fold(Vec::<u8>::new(), |mut acc, pixel| {
                    acc.extend(&[pixel.0, pixel.1, pixel.2]);
                    acc
                });

                bytes
                    .chunks(4)
                    .map(|bytes| LittleEndian::read_u32(bytes))
                    .collect()
            }
            ColorDepth::B15 => rgb_block
                .chunks(2)
                .map(|chunk| {
                    let c1 = (((chunk[0].2 as u16 / 8) & 0x1f) << 10)
                        | (((chunk[0].1 as u16 / 8) & 0x1f) << 5)
                        | ((chunk[0].0 as u16 / 8) & 0x1f);
                    let c2 = (((chunk[1].2 as u16 / 8) & 0x1f) << 10)
                        | (((chunk[1].1 as u16 / 8) & 0x1f) << 5)
                        | ((chunk[1].0 as u16 / 8) & 0x1f);
                    (c2 as u32) << 16 | (c1 as u32)
                })
                .collect(),
        }
    }
}

fn sign_extend(x: i32, nbits: u32) -> i32 {
    let notherbits = size_of_val(&x) as u32 * 8 - nbits;
    x.wrapping_shl(notherbits).wrapping_shr(notherbits)
}

fn decode_block(ctx: &super::MDEC, raw_block: &Vec<u16>, is_chroma: bool) -> Vec<f32> {
    // Algorithm copied from https://raw.githubusercontent.com/m35/jpsxdec/readme/jpsxdec/PlayStation1_STR_format.txt

    // Translate the DC and AC run length codes into a 64 value list

    let mut coefficient_list: Vec<i16> = vec![0; 64];

    let dc_coefficient = sign_extend((raw_block[0] & 0x3FF) as i32, 10);
    let quantization_scale = raw_block[0] >> 10;

    coefficient_list[0] = dc_coefficient as i16;

    let mut i = 0;
    for rlc in &raw_block[1..] {
        i += 1 + (rlc >> 10);
        coefficient_list[i as usize] = sign_extend((rlc & 0x3FF) as i32, 10) as i16;
    }

    // Un-zig-zag the list into a matrix

    let mut coefficient_matrix: Vec<i32> = vec![0; 64];

    for i in 0..64 {
        coefficient_matrix[i] = coefficient_list[ZIG_ZAG_MATRIX[i]] as i32;
    }

    // Dequantization of the matrix
    let mut dequantized_matrix: Vec<i32> = vec![0; 64];

    for i in 0..64 {
        let quant = if is_chroma {
            ctx.color_quant_table[i]
        } else {
            ctx.luminance_quant_table[i]
        };
        if i == 0 {
            dequantized_matrix[i] = coefficient_matrix[i] * quant as i32;
        } else {
            dequantized_matrix[i] =
                (2 * coefficient_matrix[i] * quant as i32 * quantization_scale as i32) / 16;
        }
    }

    ////println!("dequant {:?}", dequantized_matrix);

    // Apply Inverse Discrete Cosine Transform

    let mut transformed_matrix: Vec<f32> = vec![0.0; 64];

    for block_x in 0..8 {
        for block_y in 0..8 {
            let mut total: f64 = 0.0;

            for dct_x in 0..8 {
                for dct_y in 0..8 {
                    let mut sub_total = dequantized_matrix[dct_y * 8 + dct_x] as f64;

                    if dct_x == 0 {
                        sub_total *= ((1.0 / 8.0) as f64).sqrt();
                    } else {
                        sub_total *= ((2.0 / 8.0) as f64).sqrt();
                    }

                    if dct_y == 0 {
                        sub_total *= ((1.0 / 8.0) as f64).sqrt();
                    } else {
                        sub_total *= ((2.0 / 8.0) as f64).sqrt();
                    }

                    sub_total *=
                        f64::cos(dct_x as f64 * PI * ((2.0 * block_x as f64 + 1.0) / 16.0));
                    sub_total *=
                        f64::cos(dct_y as f64 * PI * ((2.0 * block_y as f64 + 1.0) / 16.0));
                    total += sub_total;
                }
            }

            transformed_matrix[block_y * 8 + block_x] = total as f32;
        }
    }
    //println!("cos transform {:?}", transformed_matrix);
    transformed_matrix
}

const ZIG_ZAG_MATRIX: [usize; 64] = [
    0, 1, 5, 6, 14, 15, 27, 28, 2, 4, 7, 13, 16, 26, 29, 42, 3, 8, 12, 17, 25, 30, 41, 43, 9, 11,
    18, 24, 31, 40, 44, 53, 10, 19, 23, 32, 39, 45, 52, 54, 20, 22, 33, 38, 46, 51, 55, 60, 21, 34,
    37, 47, 50, 56, 59, 61, 35, 36, 48, 49, 57, 58, 62, 63,
];
