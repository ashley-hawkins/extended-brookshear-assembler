# Extended Brookshear Machine Assembler {#extended-brookshear-machine-assembler}

This assembler targets the extended Brookshear Machine used by this project.

For emulator and UI details, see the [general help](#bm-help).

## Contents {#contents}

-   [Source format](#source-format)
-   [Literals and constant expressions](#literals-and-constant-expressions)
-   [MOV instruction](#mov-instruction)
-   [Register operations](#register-operations)
-   [Control instructions](#control-instructions)
-   [DATA pseudo-instruction](#data-pseudo-instruction)
-   [CONST pseudo-instruction](#const-pseudo-instruction)
-   [Addresses, labels, and constants](#addresses-labels-and-constants)
-   [Example](#example)

## Source format {#source-format}

Each source line has the form:

`annotation instruction // comment`

All three parts are optional.

-   An annotation is either an explicit address like `80:` or a symbolic name like `loop:`.
-   Comments may use `// line comments` or `/* block comments */`.
-   Instruction mnemonics are case-insensitive.
-   Symbolic names are case-sensitive.
-   Technically symbolic names can be the same as instruction mnemonics since it\'s not ambiguous which one is required within a given context, but you\'d be ill-advised to do so in my opinion, I can\'t stop you though.

Examples:

Explanation: specifies a MOV instruction with the label `start`. This causes `start` to become a constant representing the address of the instruction, so that you may JMP to it from some other point in the program.

``` text
start:  MOV 80 -> R1      // load immediate byte 80_h into R1
```

Explanation: puts the bytes `01`, `02`, and `03` into memory starting at address `20`, since that was specified in the annotation.

``` text
20:     DATA 01, 02, 03
```

Explanation: jump to the address `start`, which as mentioned before, evaluates to the address of the instruction with that label.

``` text
        JMP start
```

## Literals and constant expressions {#literals-and-constant-expressions}

The assembler accepts 8-bit integer literals and symbolic constants.

Supported literal forms:

-   Hexadecimal: `0A`, `FF`, `0A_h`
-   Decimal: `10_d`, `255_d` - the \_d suffix is required
-   Binary: `00001010`, `1010_b` - the \_b suffix is only needed if you specify less than 8 digits

Unlike the memory/register editor in the emulator, assembly source does not currently support floating-point or character literals.

Immediate operands may use constant expressions built from:

-   literals
-   labels
-   `CONST` names
-   `+`, `-`, `*`, `/`, `%`
-   parentheses

Examples:

``` text
display_begin: CONST 80
MOV display_begin + 4 -> R1
MOV R3 -> [display_begin + offset]
ROT R3, (8 - 1)
```

Expression results are evaluated as single bytes, so arithmetic wraps modulo 256.

## MOV instruction {#mov-instruction}

General form:

`MOV source -> destination`

Supported forms are:

`MOV expr -> Rn`

-   Load an immediate byte into register `Rn`.
-   `expr` may be any constant expression.
-   BM opcode: `2`

`MOV Rm -> Rn`

-   Copy one register to another.
-   BM opcode: `4`

`MOV [expr] -> Rn`

-   Load from memory at the direct address given by `expr`.
-   BM opcode: `1`

`MOV Rn -> [expr]`

-   Store to memory at the direct address given by `expr`.
-   BM opcode: `3`

`MOV [Rm] -> Rn`

-   Load from the memory address stored in register `Rm`.
-   BM opcode: `D`

`MOV Rn -> [Rm]`

-   Store to the memory address stored in register `Rm`.
-   BM opcode: `E`

Not all combinations are legal. For example, `MOV [R1] -> [R2]` and `MOV 10 -> [R1]` are rejected.

## Register operations {#register-operations}

`ROT Rn, expr`

-   Rotate register `Rn` to the right by the immediate amount given by `expr`.
-   The amount is encoded in the instruction\'s low nibble.
-   BM opcode: `A`

The remaining register operations all have this form:

`OP Rn, Rm -> Rp`

Supported operations:

-   `ADDI`: add as signed two\'s-complement integers. Opcode `5`
-   `ADDF`: add using the machine\'s 8-bit float format already stored in registers. Opcode `6`
-   `OR`: bitwise OR. Opcode `7`
-   `AND`: bitwise AND. Opcode `8`
-   `XOR`: bitwise XOR. Opcode `9`

All source operands must be registers and the destination must be a register.

## Control instructions {#control-instructions}

`NOP`

-   No operation.
-   Opcode `0`

`HALT`

-   Stop execution.
-   Opcode `C`

`JMP expr`

-   Jump to an immediate address given by a constant expression.
-   Opcode `B`

`JMP Rn`

-   Jump to the address currently stored in register `Rn`.
-   Opcode `F`

`JMPEQ expr, Rm`

-   Jump to the immediate address `expr` if `Rm == R0`.
-   Opcode `B`

`JMPEQ Rn, Rm`

-   Jump to the address in `Rn` if `Rm == R0`.
-   Opcode `F`

The remaining conditional jumps always take their jump target from a register:

-   `JMPNE Rn, Rm`: jump if `Rm != R0`
-   `JMPGE Rn, Rm`: jump if `Rm >= R0` as unsigned bytes
-   `JMPLE Rn, Rm`: jump if `Rm <= R0` as unsigned bytes
-   `JMPGT Rn, Rm`: jump if `Rm > R0` as unsigned bytes
-   `JMPLT Rn, Rm`: jump if `Rm < R0` as unsigned bytes

These all assemble to opcode `F`.

This means `JMP` and `JMPEQ` may use either an immediate target or a register target, but `JMPNE`, `JMPGE`, `JMPLE`, `JMPGT`, and `JMPLT` require a register target.

## DATA pseudo-instruction {#data-pseudo-instruction}

`DATA` inserts raw bytes into memory instead of generating a machine instruction.

Examples:

``` text
table: DATA 01, 02, 03, 04
mask:  DATA 11110000
```

Each operand must be an immediate constant expression and produces one byte.

If no explicit address is given, `DATA` is placed at the next free byte. If code follows DATA without an explicit new address, the next instruction is aligned to the next instruction boundary (2-byte alignment).

Consecutive data bytes are *not* aligned to a 2-byte boundary like regular instructions are. They are tightly packed according to however many bytes of data they declare.

## CONST pseudo-instruction {#const-pseudo-instruction}

`CONST` is a special instruction which is used to define a named constant without taking any memory space.

General form:

`name: CONST expr`

A label attached to a `CONST` pseudo-instruction will evaluate to that constant\'s value, rather than an address (since a CONST has no address).

Examples:

``` text
display_begin:  CONST 80 // display_begin will now be equivalent to 80 in expressions
```

Rules:

-   A `CONST` pseudo-instruction must have at least one label. The label will then be used to refer to its value.
-   The single operand must be an immediate constant expression.
-   `CONST` does not emit any bytes in the final machine code, it just replaces the label with the value at assembly time.
-   Forward references are allowed (you may refer to a constant that is defined later in the code).
-   Cyclic constant definitions are rejected. For example, this is an error:

``` text
foo: CONST bar + 1
bar: CONST foo + 1 // rejected, because `bar` depends on `foo`, which depends on `bar`
```

## Addresses, labels, and constants {#addresses-labels-and-constants}

Annotations before a statement can do two different jobs.

Explicit addresses:

``` text
80: DATA 00, 00, 00
20: JMP finish
```

-   An explicit address sets the location where the following data or instruction will be placed.
-   Explicit addresses are byte addresses.
-   You can write them with any supported literal syntax, such as `80:`, `128_d:`, or `10000000_b:`.

Symbolic names:

``` text
loop:   ADDI R1, R2 -> R1
finish: HALT
```

-   A symbolic name records the current byte address.
-   Labels attached to instructions or `DATA` become address constants.
-   Labels attached to `CONST` become named value constants.
-   Symbolic names follow identifier syntax: letters or `_`, followed by letters, digits, or `_`.

Because purely numeric annotations are parsed as explicit addresses, symbolic names should not look like number literals.

## Example {#example}

[`examples/chessboard.txt`](https://github.com/ashley-hawkins/extended-brookshear-assembler/blob/master/examples/chessboard.txt) is a simple example program that fills the bitmapped display with an 8x8 chessboard pattern.

Algorithm summary:

-   The display occupies memory `80` through `FF`.
-   Each display row uses 4 bytes.
-   The 32x32 display is divided into 8x8 tiles, so each tile is 4x4 pixels.
-   The program alternates between `00001111` and `11110000` across each row.
-   After writing 16 bytes, it swaps patterns for the next 4-row band.

Excerpt:

``` text
display_begin:  CONST 80
bytes_per_row:  CONST 4
rows_per_band:  CONST 4
bytes_per_band: CONST bytes_per_row * rows_per_band
band_count:     CONST 8

setup:  MOV display_begin -> R1
        MOV 00001111 -> R2
        MOV 11110000 -> R3
        MOV bytes_per_band -> R4

loop:   MOV R2 -> [R1]
        ADDI R1, R6 -> R1
        ADDI R4, R7 -> R4
        JMPEQ next_band, R4
        JMP loop

next_band:
        MOV bytes_per_band -> R4
        MOV R2 -> R8
        MOV R3 -> R2
        MOV R8 -> R3
```

This example shows:

-   `CONST` definitions
-   arithmetic expressions such as `bytes_per_row * rows_per_band`
-   immediate binary literals
-   indirect memory stores with `MOV R2 -> [R1]`
