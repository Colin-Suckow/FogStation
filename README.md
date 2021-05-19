# VaporStation
A toy playstation emulator written in rust. Better than my nes emulator, but still not anywhere near idiomatic rust. Now with GDB target support!



<img src="https://i.imgur.com/LhGQ5QF.png" height=500px alt="boot screen">
<img src="https://i.imgur.com/3rMhbhJ.png" height=500px alt="gdb debugging">

       


## Status
Can boot bios, display boot screen, and load and run software from cdrom, gpu bugs are blocking ridge racer from being playable

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
