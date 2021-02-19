# VaporStation
A toy playstation emulator written in rust. Better than my nes emulator, but still not anywhere near idiomatic rust.

![PSX boot logo. Rendered in VaporStation](https://i.imgur.com/LhGQ5QF.png)

## Status
Boots to, and draws, splash screen (!)

## Implemented (for the most part)
- MIPS R3000 CPU
- Memory (RAM)
- Video Memory (VRAM)
- BIOS ROM
- Timers
- Graphics processor (GPU)

## TODO
- CDROM drive
- Matrix multiplication engine (GTE)
- Massive code cleanup
- Speedup (Already runs at near full speed, but it can be a lot better)
- Sound prcessor (SPU)
