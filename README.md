# Extended Brookshear Machine Assembler

This is an assembler for the extended instruction set in JBrookshearMachine by Milan Gritta.

Improvements on the original:

- It supports evaluation of constant expressions with the following operators: +, -, *, /, %. E.g. `MOV R1 -> [somelabel + 1]`
- It packs DATA pseudo-instructions more efficiently, so that two consecutive DATA instructions will fill 2 bytes rather than using 4 bytes.
- It supports defining named constants. In fact, labels are just a special type of named constant which refers to the location of an instruction in the program.
- It supports hexadecimal, decimal, and binary literals, for example: `0A`, `10_d`, `1010_b`
- It doesn't have the bug from the original JBrookshearMachine assembler where you can't unconditionally jump to an address held in a register.
- If you try and use a named constant which doesn't exist, it will tell you the name which it couldn't fine, instead of just telling you it couldn't find one.

```
a:  CONST 10_d // the CONST pseudo-instruction is used to define a constant. These constants are labelled in the same way that real instructions are labelled. This does not take any space in the assembled output.
b:  CONST 05_d.
c:  CONST a + b
MOV c -> R1 // `c` can now be used the same as any other named constant, and this will move the value 15_d (aka 0F in hex) into R1.
```

It is a work in progress. It will have better error messages and usability once I've refactored everything to add complete exhaustive error handling and a GUI.

### TODO

- Implement the "messages" panel.
- Remove all panics that are expected to occur in normal operation of the program, replacing them with appropriate feedback to the user.
- Add documentation, and replace all remaining TODOs in the UI itself.
