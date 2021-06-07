# VaporStation
A toy playstation emulator written in rust. Better than my nes emulator, but still not anywhere near idiomatic rust. Now with GDB target support!



<img src="https://i.imgur.com/LhGQ5QF.png" height=500px alt="boot screen">
<img src="https://i.imgur.com/3rMhbhJ.png" height=500px alt="gdb debugging">

       
## Building

Just run `cargo build --release` from /desktop to build the emulator core and desktop client. Right now windows is not supported due to a dependnecy on libcue.

## Status
My current goal is to make Puzzle Bobble 2 playable. Puzzle Bobble 2 is known for being an easy game to emulate. Most games require the **Graphics Transformation Engine (GTE)** to run. The GTE handles all the math required for 3D rendering. Puzzle Bobble 2, being a 2D only game, does not require the GTE. In fact, the game doesn't require many other hardware features at all. This simplicity makes it a good candidate for early emulation.

Previously acheived goals
- Can boot bios
- render boot screen
- load and execute software from cdrom
- display a very broken version of ridge racer

Right now I am working on fixing my GPU implementation while I wait for my copy of puzzle bobble 2 to arrive.

## Implemented (for the most part)
- MIPS R3000 CPU
- Memory (RAM)
- Video Memory (VRAM)
- BIOS ROM
- Timers
- Graphics processor (GPU)
- CDROM drive
- GDB support

## TODO
- Matrix multiplication engine (GTE)
- Massive code cleanup
- Speedup (Already runs at near full speed, but it can be a lot better)
- Sound prcessor (SPU)

# Why VaporStation?
![Vaporwave test rom](https://i.imgur.com/xs7LBiG.png)
The name VaporStation is in reference to the internet aesthetic known as vaporwave. The picture above was the output of a broken test, and in my opinion is a bit vaporwaveish. People in the EmuDev discord agreed, and suggested the name VaporStation.
