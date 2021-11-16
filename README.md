# VaporStation
A toy playstation emulator written in rust. Better than my nes emulator, but still not anywhere near idiomatic rust. Now with GDB target support!



<img src="https://i.imgur.com/LhGQ5QF.png" height=500px alt="boot screen">
<img src="https://i.imgur.com/3rMhbhJ.png" height=500px alt="gdb debugging">

       
## Building

Just run `cargo build --release` from /desktop to build the emulator core and desktop client. Right now windows is not supported due to a dependnecy on libcue.

## Status

### Runs some games, most still freeze up while booting

Thanks to fixing one last issue with the OTC DMA, things are starting to work! Puzzle Bobble 2 is 100% playable, with minor graphical issues. Ridge racer is rendering some 3d graphics now, but there are some strange vertex explosions happening on the cars. I don't know if this is a GTE or GPU issue right now. On the brightside, I can see the proper geometry rendering under the vertex issues. Aside from that there is still some texure issues, but I believe that is due to texture paging, which I haven't implemented yet.


Previously acheived goals
- Can boot bios
- render boot screen
- load and execute software from cdrom
- display a very broken version of ridge racer
- Implement basic GTE commands

## Implemented (for the most part)
- MIPS R3000 CPU
- Memory (RAM)
- Video Memory (VRAM)
- BIOS ROM
- Timers
- Graphics processor (GPU)
- CDROM drive
- GDB support
- Matrix multiplication accelerator (GTE)


## TODO
- Massive code cleanup
- Optimize (Runs at full speed on my desktop, but thats not a good thing)
- Sound processor (SPU)

# Why VaporStation?
![Vaporwave test rom](https://i.imgur.com/xs7LBiG.png)
The name VaporStation is in reference to the internet aesthetic known as vaporwave. The picture above was the output of a broken test, and in my opinion is a bit vaporwaveish. People in the EmuDev discord agreed, and suggested the name VaporStation.
