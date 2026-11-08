#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Float8ParseError {
    #[error("Failed to parse float: {0}")]
    InvalidFloat(#[from] std::num::ParseFloatError),
    #[error("Float value {value} cannot be represented in Brookshear float8 format")]
    Unrepresentable { value: f32 },
}

pub fn try_float8_add(a: u8, b: u8) -> Result<u8, Float8ParseError> {
    let a_f32 = float8_to_f32(a);
    let b_f32 = float8_to_f32(b);
    try_f32_to_float8(a_f32 + b_f32)
}

pub fn float8_to_f32(flt: u8) -> f32 {
    let sign = if flt & 0b1000_0000 != 0 { -1.0 } else { 1.0 };
    let exp = flt >> 4 & 0b0111;
    let mantissa = flt & 0b0000_1111;
    sign * (mantissa as f32) * f32::exp2(exp as f32 - ((1 << 2) as f32) - 4.0)
}

pub fn try_f32_to_float8(flt: f32) -> Result<u8, Float8ParseError> {
    if flt == 0.0 {
        return Ok(0);
    }

    if !flt.is_finite() {
        return Err(Float8ParseError::Unrepresentable { value: flt });
    }

    let sign = if flt < 0.0 { 0b1000_0000 } else { 0 };
    let abs_flt = flt.abs();
    let raw_exp = (abs_flt.log2().floor() as i32 + 1).clamp(-4, 3);
    let exp = (raw_exp + 4) as u8;
    let mantissa = (abs_flt / f32::exp2((raw_exp - 4) as f32)).round() as u8;

    if !(0b000..0b1000).contains(&exp) || mantissa > 0b1111 {
        return Err(Float8ParseError::Unrepresentable { value: flt });
    }

    let result = sign | ((exp & 0b0111) << 4) | (mantissa & 0b0000_1111);
    let roundtrip = float8_to_f32(result);

    if roundtrip != flt {
        Err(Float8ParseError::Unrepresentable { value: flt })
    } else {
        Ok(result)
    }
}

pub fn f32_to_float8(flt: f32) -> u8 {
    match try_f32_to_float8(flt) {
        Ok(value) => value,
        Err(Float8ParseError::Unrepresentable { .. }) => {
            panic!(
                "Todo: Graceful handling. Got float8 value that can't be represented: {}",
                flt
            )
        }
        Err(Float8ParseError::InvalidFloat(_)) => {
            unreachable!("f32 inputs are always syntactically valid")
        }
    }
}

pub fn float8_to_string(flt: u8) -> String {
    float8_to_f32(flt).to_string()
}

pub fn string_to_float8(s: &str) -> Result<u8, Float8ParseError> {
    let parsed = s.parse::<f32>()?;
    try_f32_to_float8(parsed)
}

#[cfg(test)]
mod tests {
    use super::{Float8ParseError, float8_to_f32, try_f32_to_float8};

    #[test]
    fn float8_f32_to_byte_to_f32_roundtrip() {
        // Generate all unique representable f32 values from the 256 byte patterns
        let mut representable = Vec::new();
        for b in 0..=255u8 {
            representable.push(float8_to_f32(b));
        }
        representable.sort_by(|a, b| a.partial_cmp(b).unwrap());
        representable.dedup_by(|a, b| (*a - *b).abs() < f32::EPSILON);

        let mut failures = Vec::new();
        for f in &representable {
            match try_f32_to_float8(*f) {
                Ok(b) => {
                    let f2 = float8_to_f32(b);
                    if (f - f2).abs() >= f32::EPSILON {
                        failures.push(format!(
                            "f32={f} -> byte={b:08b} -> f32={f2} (diff={})",
                            f - f2
                        ));
                    }
                }
                Err(e) => failures.push(format!("f32={f} -> error: {e}")),
            }
        }
        assert!(
            failures.is_empty(),
            "{} representable f32 round-trip failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn float8_unrepresentable_finite_values() {
        let unrepresentable_values = [8.0, 4.25, 2.125, 1.0625, 0.001953125];

        for &val in &unrepresentable_values {
            if let Ok(val) = try_f32_to_float8(val) {
                panic!("Expected unrepresentable value, but got Ok({val})");
            }
            if let Ok(val) = try_f32_to_float8(-val) {
                panic!("Expected unrepresentable value, but got Ok({val})");
            }
        }
    }

    #[test]
    fn float8_zero_roundtrip() {
        assert_eq!(try_f32_to_float8(0.0), Ok(0));
        assert_eq!(try_f32_to_float8(-0.0), Ok(0));
        assert_eq!(float8_to_f32(0x00), 0.0);
        assert_eq!(float8_to_f32(0x80), -0.0);
    }

    #[test]
    fn float8_special_values() {
        assert!(matches!(
            try_f32_to_float8(f32::NAN),
            Err(Float8ParseError::Unrepresentable { .. })
        ));
        assert!(matches!(
            try_f32_to_float8(f32::INFINITY),
            Err(Float8ParseError::Unrepresentable { .. })
        ));
        assert!(matches!(
            try_f32_to_float8(f32::NEG_INFINITY),
            Err(Float8ParseError::Unrepresentable { .. })
        ));
    }
}
