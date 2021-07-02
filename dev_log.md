## 6-17-21
The amount of stuff I'm forgetting is getting ridiculous. I can spend days on a bug, only to forget what the problem was a few days after solving it. So now I will be logging what I am working on each day in this file.

Yesterday I managed to solve a GPU bug that prevented the bios from booting games. One of the DMA calls would appear to hammer the GPU with garbage data, causing the GPU to panic and halt the emulator. As it turns out, this garbage was texture data. The problem wasn't the data, but instead that the GPU wasn't ready to receive the data. Normally the program should initiate a CPU->VRAM transfer on the GPU, then send the pixel data. There was a transfer being initiated, but it just ended before the CPU was done sending data. 

The issue ended up being a problem with how transfer sizes are calculated. Because the data being transferred is a rectangular image, the CPU must give width and height information for the data about to be sent. These widths and heights are both measured in pixels. The total size is then obviously a simple width * height. Unfortunately, there is one complication here. The CPU is sending 32bit words, but the GPU stores pixels as 16bit halfwords. Two halfwords are OR'd together and sent in one word. The GPU then has to unpack the two pixels before it can store them. This is all well and nice, but what does it mean for our transfer size calculation? For each pixel sent, there's only half a transfer. So it follows that we need to halve the transfer size calculation. But that is not what I did. Instead, I made a dumb mistake and accidentally multiplied the transfer size by two. Now the transfer size is actually 4 times bigger than it should be. We definitely won't be missing texture data, but now future commands will be interpreted as texture data and therefore be missed. Let's say one of these commands is to start a new CPU->VRAM transfer. Instead of starting this new transfer, the GPU just continues with its old transfer, which is probably getting pretty close to finished by now. Once that original transfer ends, the GPU misinterprets the rest of the texture data as commands, causing the halt we saw earlier.

What I don't understand is why the emu worked at all with this bug. The bios was able to load the PlayStation logo textures just fine. The animated intro had no issues either. I can only assume this is one of those bugs that lined up just well enough to make everything look like it was working fine.

## 7-1-21

I made some good progress in ghidra tonight. The OT corruption appears to be caused by 'GsSortClear()' trying to change a resolution argument to account for the 24bit color depth mdec image. At least I'm pretty sure its' a resolution argument. I don't know for sure because I can't figure out what GPU function this is. For most gpu commands the color is added to the back of the command number. I can see where the colors are being set, but for some reason the actual command number itself is never set. I can think of a couple possible reasons for this.

  - GsSortClear uses a strange GPU command I don't know about. 
  - The command number is set before 'GsSortClear()' is called.

The first option is extremely unlikely. My GPU is designed to halt the emulator whenever it comes across an unknown command, so I probably would have seen a new command by now. The second one is definitely possible though. 'GsSortClear()' stores its' clear command at a hardcoded address, so I don't think it out of the question that unchanging values might be set in an initialization function. I know what address the command number should be at, so I'm going to take a look

### Later

I was right. In 'variable_init()' theres two calls (one for each ordering table) to a function called 'SetBlockFill()'. This function takes the address of the clear command, and initializes all the static values. 'SetBlockFill()' sets the command number to 2 (A quick rectangle draw), and the total command length to 3. Thankfully this is what I expected. Now I just need to figure out why this is corrupting the ordering table.

### Later

Turns out that 'GsSortClear()' is working fine. I need to adjust my gpu to handle the 24bit color depth, but thats no biggie. what I don't understand is why the OT address is getting screwed up. It is being set to 'ot->tag', maybe the tag value is getting corrupted for some reason?
