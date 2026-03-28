#[derive(Debug, PartialEq, Clone, Copy, strum::FromRepr)]
#[repr(u8)]
pub enum Register {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    RA,
    RB,
    RC,
    RD,
    RE,
    RF,
}

impl Register {
    pub fn as_index(self) -> u8 {
        self as u8
    }
}
