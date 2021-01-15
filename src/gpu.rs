use core::num;

use bit_field::BitField;
use num_derive::FromPrimitive;

pub struct Gpu {
    vram: Vec<u16>,
    status_reg: u32,
    pixel_count: u32,
    enabled: bool,
    gp0_words_to_read: usize,
    gp0_buffer: [u32; 1024],
    gp0_buffer_address: usize,

    texpage_x_base: u16,
    texpage_y_base: u16,

    draw_area_top_left_x: u16,
    draw_area_top_left_y: u16,
    draw_area_bottom_right_x: u16,
    draw_area_bottom_right_y: u16,

    irq_fired: bool,
    vlank_consumed: bool,
}


impl Gpu {
    pub fn new() -> Gpu {
        Gpu {
            vram: vec![0; (1_048_576 / 2)],
            status_reg: 0,
            pixel_count: 0,
            enabled: false,
            gp0_words_to_read: 0,
            gp0_buffer: [0; 1024],
            gp0_buffer_address: 0,

            texpage_x_base: 0,
            texpage_y_base: 0,

            draw_area_top_left_x: 0,
            draw_area_top_left_y: 0,
            draw_area_bottom_right_x: 0,
            draw_area_bottom_right_y: 0,

            irq_fired: false,
            vlank_consumed: false,
        }
    }

    pub fn read_status_register(&self) -> u32 {
        self.status_reg
    }

    pub fn read_word_gp0(&mut self) -> u32 {
        0
    }

    pub fn send_gp0_command(&mut self, value: u32) {
        self.gp0_push(value);

        let command = self.gp0_buffer[0];

        match command.gp0_header() {
            0x0 => {
                //Random junk
                match command >> 24 {
                    0x2 => {
                        //Quick rectangle fill
                        if self.gp0_buffer_address < 3 {
                            //Not enough commands
                            return;
                        }

                        let x1 = self.gp0_buffer[1] & 0xFFFF;
                        let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        let x2 = (self.gp0_buffer[2] & 0xFFFF) + x1;
                        let y2 = ((self.gp0_buffer[2] >> 16) & 0xFFFF) + y1;

                        self.draw_solid_box(
                            x1,
                            y1,
                            x2,
                            y2,
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                            false,
                        );
                    }
                    _ => {
                        //NOP
                    }
                }
            }

            0x1 => {
                //Render Polygon

                // If the polygon is textured or gouraud shaded, lets just lock up the emulator.
                // I only want to test flat shaded polygons right now
                if command.get_bit(28) || command.get_bit(1) {
                    self.gp0_buffer_address = 1; //Prevent overflowing the buffer with more calls.
                    println!("Textured or shaded polygon!");
                    return;
                }

                let verts = if command.get_bit(27) { 4 } else { 3 };

                if self.gp0_buffer_address < verts {
                    // Not enough words for the command. Return early
                    return;
                }

                //Actually draw the polygon
                panic!("Tried to draw a polygon. I don't want to do this right now");
            }

            0x3 => {
                //Render Rectangle

                // If the rectangle is textured, lets just lock up the emulator.
                // I only want to test flat shaded rectangles right now
                if command.get_bit(26) {
                    println!("Textured rectangle");
                    self.gp0_buffer_address = 1; //Prevent overflowing the buffer with more calls.
                    return;
                }

                let size = (command >> 27) & 0x3;

                let length = 2 + if size == 0 { 1 } else { 0 };

                if self.gp0_buffer_address < length {
                    //Not enough commands
                    return;
                }

                match size {
                    0b01 => {
                        //Draw single pixel
                        let x = self.gp0_buffer[1] & 0xFFFF;
                        let y = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        let address = self.point_to_address(x, y) as usize;
                        let color = if command.get_bit(25) {
                            //Transparent
                            alpha_composite(
                                self.vram[address],
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                            )
                        } else {
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF)
                        };
                        self.vram[address] = color;
                    }

                    0b0 => {
                        //Draw variable size
                        let x1 = self.gp0_buffer[1] & 0xFFFF;
                        let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        let x2 = (self.gp0_buffer[2] & 0xFFFF) + x1;
                        let y2 = ((self.gp0_buffer[2] >> 16) & 0xFFFF) + y1;

                        self.draw_solid_box(
                            x1,
                            y1,
                            x2,
                            y2,
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                            command.get_bit(25),
                        );
                    }

                    0b10 => {
                        //8x8 sprite
                        let x1 = self.gp0_buffer[1] & 0xFFFF;
                        let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        self.draw_solid_box(
                            x1,
                            y1,
                            x1 + 8,
                            y1 + 8,
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                            command.get_bit(25),
                        );
                    }

                    0b11 => {
                        //16x16 sprite
                        let x1 = self.gp0_buffer[1] & 0xFFFF;
                        let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        self.draw_solid_box(
                            x1,
                            y1,
                            x1 + 16,
                            y1 + 16,
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                            command.get_bit(25),
                        );
                    }

                    _ => {
                        //Lets do nothing with the others
                    }
                }
            }

            0x4 => {
                //VRAM to VRAM blit
                if self.gp0_buffer_address < 4 {
                    //Not enough commands
                    return;
                }

                let x_source = self.gp0_buffer[1] & 0xFFFF;
                let y_source = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                let x_dest = self.gp0_buffer[2] & 0xFFFF;
                let y_dest = (self.gp0_buffer[2] >> 16) & 0xFFFF;
                let width = self.gp0_buffer[3] & 0xFFFF;
                let height = (self.gp0_buffer[3] >> 16) & 0xFFFF;

                self.copy_rectangle(x_source, y_source, x_dest, y_dest, width, height);
            }
            0x5 => {
                //CPU To VRAM
                let width = ((self.gp0_buffer[2] & 0xFFFF) as u16);
                let height = (((self.gp0_buffer[2] >> 16) & 0xFFFF) as u16) * 2;
                let length = (((width / 2) * height) / 2) + 3;
                if self.gp0_buffer_address < length as usize {
                    //Not enough commands
                    return;
                }

                let base_x = ((self.gp0_buffer[1] & 0xFFFF) as u16);
                let base_y = ((self.gp0_buffer[1] >> 16) & 0xFFFF) as u16;

                for index in 3..(length) {
                    let p1 = ((self.gp0_buffer[index as usize] >> 16) & 0xFFFF) as u16;
                    let p2 = (self.gp0_buffer[index as usize] & 0xFFFF) as u16;
                    let x = (base_x + (((index - 3) * 2) % width));
                    let y = (base_y + (((index - 3) * 2) / width));
                    let addr = self.point_to_address(x as u32, y as u32);
                    self.vram[addr as usize] = p1;
                    self.vram[(addr + 1) as usize] = p2;
                }
            }
            0x7 => {
                //Env commands
                match command.command() {
                    0xE1 => {
                        //Draw Mode Setting
                        //TODO I'm going to ignore everything but the texture page settings for now
                        self.texpage_x_base = ((command & 0xF) * 64) as u16;
                        self.texpage_y_base = if command.get_bit(4) { 256 } else { 0 };
                    }

                    0xE3 => {
                        //Set Drawing Area Top Left
                        self.draw_area_top_left_x = (command & 0x3FF) as u16;
                        self.draw_area_top_left_y = ((command >> 10) & 0x1FF) as u16;
                    }

                    0xE4 => {
                        //Set Drawing Area Bottom Right
                        self.draw_area_bottom_right_x = (command & 0x3FF) as u16;
                        self.draw_area_bottom_right_y = ((command >> 10) & 0x1FF) as u16;
                    }

                    0xE5 => {
                        //Set Drawing Offset
                        //TODO Implement. I'm too lazy right now
                    }
                    _ => panic!("Unknown GPU ENV command {:#X}", command.command()),
                }
            }

            _ => panic!("unknown gp0 {:#X}!", command.gp0_header()),
        }
        //Made it to the end, so the command must have been executed
        self.gp0_clear();
    }

    pub fn send_gp1_command(&mut self, command: u32) {
        match command.command() {
            0x0 => {
                //Reset GPU
                self.enabled = false;
                self.status_reg = 0;
                self.pixel_count = 0;
                self.vram = vec![0; 1_048_576 / 2];
            }

            0x6 => {
                //Horizontal Display Range
                //Ignore this one for now
            }

            0x10 => {
                //Get gpu information
                //Ignoring this too
            }
            _ => println!(
                "Unknown gp1 command {:#X} parameter {}!",
                command.command(),
                command.parameter()
            ),
        }
    }

    pub fn execute_cycle(&mut self) {
        self.pixel_count += 1;

        if self.pixel_count > 640 * 512 {
            self.pixel_count = 0;
            self.vlank_consumed = false;
        }
    }

    pub fn is_vblank(&self) -> bool {
        self.pixel_count > 640 * 480 && self.pixel_count < 640 * 512
    }

    pub fn consume_vblank(&mut self) -> bool {
        if !self.vlank_consumed && self.is_vblank() {
            self.vlank_consumed = true;
            true
        } else {
            false
        }
    }

    pub fn end_of_frame(&self) -> bool {
        self.pixel_count == 640 * 512
    }

    pub fn get_vram(&self) -> &Vec<u16> {
        &self.vram
    }

    ///Returns irq status. If true, function will return true then clear irq status
    pub fn consume_irq(&mut self) -> bool {
        if self.irq_fired {
            self.irq_fired = false;
            true
        } else {
            false
        }
    }

    fn gp0_push(&mut self, val: u32) {
        self.gp0_buffer[self.gp0_buffer_address] = val;
        self.gp0_buffer_address += 1;
    }

    fn gp0_pop(&mut self) -> u32 {
        self.gp0_buffer_address -= 1;
        self.gp0_buffer[self.gp0_buffer_address]
    }

    fn gp0_clear(&mut self) {
        self.gp0_buffer_address = 0;
    }

    fn point_to_address(&self, x: u32, y: u32) -> u32 {
        ((1024) as u32 * y) + x
    }

    fn copy_horizontal_line(
        &mut self,
        x_source: u32,
        y_source: u32,
        x_dest: u32,
        y_dest: u32,
        width: u32,
    ) {
        for x_offset in 0..width {
            let val = self.vram[self.point_to_address(x_source + x_offset, y_source) as usize];
            let addr = self.point_to_address(x_dest + x_offset, y_dest) as usize;
            self.vram[addr] = val;
        }
    }

    fn copy_rectangle(
        &mut self,
        x_source: u32,
        y_source: u32,
        x_dest: u32,
        y_dest: u32,
        width: u32,
        height: u32,
    ) {
        for y_offset in 0..height {
            self.copy_horizontal_line(
                x_source,
                y_source + y_offset,
                x_dest,
                y_dest + y_offset,
                width,
            );
        }
    }

    fn draw_horizontal_line(&mut self, x1: u32, x2: u32, y: u32, fill: u16, transparent: bool) {
        for x in x1..=x2 {
            let address = self.point_to_address(x, y) as usize;
            let color = if transparent {
                alpha_composite(self.vram[address], fill)
            } else {
                fill
            };
            self.vram[address] = color;
        }
    }

    fn draw_solid_box(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, fill: u16, transparent: bool) {
        for y in y1..y2 {
            self.draw_horizontal_line(x1, x2, y, fill, transparent);
        }
    }
}

fn b24color_to_b15color(color: u32) -> u16 {
    let r = ((color >> 16) & 0xFF) / 8;
    let g = ((color >> 8) & 0xFF) / 8;
    let b = (color & 0xFF) / 8;
    ((r << 10) | (g << 5) | b) as u16
}

fn b15_to_rgb(color: u16) -> (u8, u8, u8) {
    (
        ((color >> 10) & 0x1F) as u8,
        ((color >> 5) & 0x1F) as u8,
        (color & 0x1F) as u8,
    )
}

fn rgb_to_b15(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16) << 10) | ((g as u16) << 5) | (b as u16)
}

//TODO Make colors more accurate
fn alpha_composite(background_color: u16, alpha_color: u16) -> u16 {
    let (b_r, b_g, b_b) = b15_to_rgb(background_color);
    let (a_r, a_g, a_b) = b15_to_rgb(alpha_color);
    rgb_to_b15((a_r + b_r), (a_g + b_g), (a_b + b_b))
}

//Helper trait + impl
trait Command {
    fn gp0_header(&self) -> u8;
    fn command(&self) -> u8;
    fn parameter(&self) -> u32;
}

impl Command for u32 {
    fn gp0_header(&self) -> u8 {
        ((self.clone() >> 29) & 0x7) as u8
    }

    fn command(&self) -> u8 {
        ((self.clone() >> 24) & 0xFF) as u8
    }

    fn parameter(&self) -> u32 {
        (self.clone() & 0x7FFFFF)
    }
}
