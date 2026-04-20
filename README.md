# Extended Brookshear Machine Assembler

This is an assembler for the extended instruction set in JBrookshearMachine by Milan Gritta.

## Improvements - Assembler

- It supports evaluation of constant expressions with the following operators: +, -, *, /, %. E.g. `MOV R1 -> [somelabel + 1]`
- It packs DATA pseudo-instructions more efficiently, so that two consecutive 1-byte DATA instructions will fill 2 bytes rather than using 4 bytes.
- It supports defining named constants. In fact, labels are just a special type of named constant which refers to the location of an instruction in the program.
- It supports hexadecimal, decimal, and binary literals, for example: `0A`, `10_d`, `1010_b`
- It doesn't have the bug from the original JBrookshearMachine assembler where you can't unconditionally jump to an address held in a register (e.g. `JMP R1` fails to parse).
- If you try and use a named constant which doesn't exist, it will tell you the name which it couldn't find, instead of just telling you it couldn't find one.

```
a:  CONST 10_d // the CONST pseudo-instruction is used to define a constant. These constants are labelled in the same way that real instructions are labelled. This does not take any space in the assembled output.
b:  CONST 05_d
c:  CONST a + b
MOV c -> R1 // `c` can now be used the same as any other named constant, and this will move the value 15_d (aka 0F in hex) into R1.
```

It is a work in progress. It will have better error messages and usability once I've refactored everything to add complete exhaustive error handling and a GUI.

## Documentation

- [doc/asmhelp.md](./doc/asmhelp.md) documents the current assembler syntax, including `CONST`, constant expressions, literal forms, and the actual jump operand rules.
- [doc/bmhelp.md](./doc/bmhelp.md) documents the current emulator UI and file formats.
- [examples/chessboard.txt](./examples/chessboard.txt) is a simple example program that draws an 8x8 chessboard pattern in the bitmapped display.

## Improvements - GUI Emulator

- Uses the improved assembler
- Has more detailed error messages
- Parses and encodes floating point values properly
- You can click on a pixel in the bit-mapped display to toggle its corresponding bit.
- Saving the bit-mapped display as as an image gives you a 32x32 png - a pixel-for-pixel representation of the display, rather than giving a stretched or warped version based on the current resolution of the window.
- There is a button to clear / fill the bit-mapped display.
- There is a button to load a previously-saved image onto the bit-mapped display.
- The speed of execution is clearly communicated (instructions per second) rather than a 0-100 slider of unexplained units
- Runs in a web browser, and can be installed as a PWA

### TODO

- Address certain edge cases in 'undo' (like when the user loads a new program or modifies registers/memory mid-run, the undo history should be invalidated).
- Unit tests

Maybe:

- A way to uncap the speed of the emulator?
- A way to lock the display so you don't accidentally modify it by clicking on it
