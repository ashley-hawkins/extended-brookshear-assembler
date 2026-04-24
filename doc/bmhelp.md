# Extended Brookshear Machine Emulator

This emulator runs an extended Brookshear Machine, based on the machine described in _Computer Science: An Overview, 13th edition_, by J. Glenn Brookshear and extended with four additional instructions and a bitmapped display.

Programs may be entered as raw memory images or assembled from source code. For assembly syntax, see the [assembler help](./asmhelp.md).

## Contents

- [Machine architecture](#machine-architecture)
- [Instruction set](#instruction-set)
- [Memory panel](#memory-panel)
- [Assembly and file operations](#assembly-and-file-operations)
- [Register panel](#register-panel)
- [CPU controls](#cpu-controls)
- [Messages and help](#messages-and-help)
- [Bitmapped display](#bitmapped-display)

## Machine architecture

- Main memory has 256 bytes, addressed from `00` to `FF`.
- There are 16 general-purpose 8-bit registers, `R0` through `RF`.
- The program counter is a separate 8-bit register which increments by 2 after execution of each instruction, unless that instruction caused a jump (which would set the program counter to a new value) or halt (which would cause the machine to stop entirely). Unlike the general purpose registers, this register cannot be manipulated directly.
- Every instruction is 2 bytes long.

## Instruction set

The underlying machine instructions are:

- `0FFF`: no operation
- `1rxy`: load `memory[xy]` into `Rr`
- `2rxy`: load immediate `xy` into `Rr`
- `3rxy`: store `Rr` into `memory[xy]`
- `40rs`: copy `Rr` into `Rs`
- `5rst`: integer add `Rs` to `Rt` and store the result in `Rr`
- `6rst`: floating point add `Rs` to `Rt` and store the result in `Rr`
- `7rst`: bitwise OR `Rs` with `Rt` and store the result in `Rr`
- `8rst`: bitwise AND `Rs` with `Rt` and store the result in `Rr`
- `9rst`: bitwise XOR `Rs` with `Rt` and store the result in `Rr`
- `Ar0x`: rotate `Rr` right by `x` bits
- `Brxy`: jump to `xy` if `Rr == R0`
- `C000`: halt
- `D0rs`: load `memory[Rs]` into `Rr`
- `E0rs`: store `Rr` into `memory[Rs]`
- `Frxt`: compare `Rr` with `R0`, and if the selected test succeeds, jump to the address stored in `Rt`

  Where `x` selects the test as follows:

  - `x` = `0`: tests whether `Rr` equals `R0`
  - `x` = `1`: tests whether `Rr` does not equal `R0`
  - `x` = `2`: tests whether `Rr` is greater than or equal to `R0`
  - `x` = `3`: tests whether `Rr` is less than or equal to `R0`
  - `x` = `4`: tests whether `Rr` is greater than `R0`
  - `x` = `5`: tests whether `Rr` is less than `R0`

  These comparisons use unsigned arithmetic.

The float add instruction uses the machine's 8-bit float format `SEEEMMMM`. There is no enforcement of float normalization or an implicit leading 1 bit, so be careful if you want to compare two floats' equality as it's possible to have two different representations of the same number which would be considered not equal with a simple byte-wise comparison as the conditional jump instructions do.

## Memory panel

The memory table shows each byte in several representations:

- address
- binary
- hex
- unsigned decimal
- signed decimal
- float
- ASCII
- decoded instruction text (only at the beginning of an aligned instruction boundary containing a valid instruction)

Notes:

- Double-click a memory cell to edit it.
- Editing any representation updates the underlying byte and all other views.
- The instruction column decodes bytes at executable addresses.
- Instruction text can be shown either as descriptive English or as assembler-style disassembly, which is configured in the toolbar below the memory table.
- The highlighted row tracks the current program counter.
- The `Jump to cell...` box scrolls the table to a specific hex address.

## Assembly and file operations (the set of buttons below the memory table)

The memory toolbar provides these actions:

- _Reset_: clear all memory bytes to `00`
- _Save to File_: save the full 256-byte memory image as a binary file
- _Load from File_: load a 256-byte binary memory image, replacing the current memory
- _Assemble and load_: read a UTF-8 assembly source file, assemble it, and load the resulting 256-byte image into memory
- _Assemble to file_: assemble a UTF-8 assembly source file and save the resulting 256-byte image without changing memory
- Choose between English or assembler-style instruction text in the memory table with the radio buttons labeled "descriptive" and "assembler".

If assembly fails, memory is left unchanged and a detailed error report is available.

## Register panel

The register table shows the same byte representations as the memory table.

Notes:

- General-purpose registers are editable by double-clicking a cell.
- The program counter has its own hex entry box below the register table.
- `Reset` in this panel clears all registers and resets the program counter to `00`.

## CPU controls

The CPU controls panel provides:

- `Step`: execute exactly one instruction
- `Undo Step`: revert the most recent executed instruction when undo history is available
- `Continue` or `Pause`: resume or pause execution
- `Reset & Run`: reset registers and program counter, then start execution
- `Speed (instructions per second)`: execution rate control for continuous running. The slider goes from 0.5 to 200 IPS, but you may enter a custom value in the box up to 1000 IPS.

## Messages and help

- `Instructions` shows a condensed machine-code instruction reference.
- `Help` opens this general help page.
- `Assembler help` opens the [assembly-language reference](./asmhelp.md).
- The status message area shows the current instruction count or any emulator error.
- The status message box is clickable when detailed  output is available, such as full parser or assembler errors. The message will say "Click for details" if such details are available.

## Bitmapped display

The machine includes a 32x32 1-bit display backed by memory `80` through `FF`.

Display mapping:

- Each bit in `80..FF` maps to one pixel.
- Bit value `0` is black.
- Bit value `1` is white.
- Within each byte, bit 7 is the leftmost pixel and bit 0 is the rightmost. By bit 0, I mean the least significant bit, which on its own has a value of 1, and by bit 7, I mean the most significant bit, which on its own has a value of 128.

UI behavior:

- `Display On` and `Display Off` toggle whether the bitmapped display is shown.
- While the display is visible, clicking a pixel toggles its corresponding bit in memory.
- `Save Image` exports the current display as a 32x32 PNG.

The display updates immediately when memory changes, whether the change comes from a running program, direct memory edits, or clicking pixels.
