fn normalize_exponent(mut text: String) -> String {
    let Some(at) = text.find(|character| character == 'e' || character == 'E') else {
        return text;
    };
    let sign = at + 1;
    let has_sign = text
        .as_bytes()
        .get(sign)
        .is_some_and(|character| *character == b'+' || *character == b'-');
    if !has_sign {
        text.insert(sign, '+');
    }
    let digits = sign + 1;
    if text.len() - digits < 2 {
        text.insert_str(digits, &"0".repeat(2 - (text.len() - digits)));
    } else {
        let mut first = digits;
        while text.as_bytes().get(first) == Some(&b'0') && text.len() - first > 2 {
            first += 1;
        }
        if first > digits {
            text.drain(digits..first);
        }
    }
    text
}

fn special(value: f64) -> Option<String> {
    if value.is_nan() {
        Some(if value.is_sign_negative() { "-nan" } else { "nan" }.to_owned())
    } else if value.is_infinite() {
        Some(if value.is_sign_negative() { "-inf" } else { "inf" }.to_owned())
    } else {
        None
    }
}

fn scientific(value: f64, precision: usize) -> (String, i32) {
    let text = format!("{:.*e}", precision, value);
    let at = text.find('e').expect("Rust formatter omitted float exponent");
    let exponent = text[at + 1..].parse().expect("Rust formatter returned bad exponent");
    (text[..at].to_owned(), exponent)
}

/// Formats with C `printf`'s `%g` conversion and the requested significant-digit precision.
pub fn g(value: f64, precision: usize) -> String {
    let mut text = String::new();
    g_into(&mut text, value, precision);
    text
}

/// A fixed buffer `write!` fills without allocating.
struct Stack {
    bytes: [u8; 64],
    len: usize,
}

impl std::fmt::Write for Stack {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let end = self.len + s.len();
        if end > self.bytes.len() {
            return Err(std::fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// `g` into `out` (cleared first): no allocation once `out` has room for the text.
pub fn g_into(out: &mut String, value: f64, precision: usize) {
    use std::fmt::Write;
    out.clear();
    if let Some(text) = special(value) {
        out.push_str(&text);
        return;
    }
    let precision = precision.max(1);
    let mut stack = Stack { bytes: [0; 64], len: 0 };
    if write!(stack, "{:.*e}", precision - 1, value).is_err() {
        // ponytail: past 60 significant digits the scientific text is made on the heap
        let (mantissa, exponent) = scientific(value, precision - 1);
        return plain_into(out, &mantissa, exponent, precision);
    }
    let text = std::str::from_utf8(&stack.bytes[..stack.len]).expect("Rust formatter wrote UTF-8");
    let at = text.find('e').expect("Rust formatter omitted float exponent");
    let exponent = text[at + 1..].parse().expect("Rust formatter returned bad exponent");
    plain_into(out, &text[..at], exponent, precision);
}

/// `%g`'s text of a scientific `mantissa` and `exponent`.
fn plain_into(out: &mut String, mantissa: &str, exponent: i32, precision: usize) {
    use std::fmt::Write;
    let mantissa = mantissa.trim_end_matches('0');
    let mantissa = mantissa.strip_suffix('.').unwrap_or(mantissa);
    if exponent < -4 || (exponent >= 0 && exponent as usize >= precision) {
        let _ = write!(out, "{mantissa}e{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.unsigned_abs());
        return;
    }
    let negative = mantissa.starts_with('-');
    let digits = || mantissa.chars().filter(|character| *character != '-' && *character != '.');
    let shifted = 1 + exponent;
    let n = digits().count() as i32;
    if negative {
        out.push('-');
    }
    if shifted <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-shifted) as usize));
        out.extend(digits());
    } else if shifted >= n {
        out.extend(digits());
        out.extend(std::iter::repeat_n('0', (shifted - n) as usize));
    } else {
        let split = shifted as usize;
        out.extend(digits().take(split));
        out.push('.');
        out.extend(digits().skip(split));
    }
}

/// Formats with C `printf`'s `%f` conversion and the requested fractional-digit precision.
pub fn f(value: f64, precision: usize) -> String {
    special(value).unwrap_or_else(|| format!("{:.*}", precision, value))
}

/// Formats with C `printf`'s `%e` conversion and the requested fractional-digit precision.
pub fn e(value: f64, precision: usize) -> String {
    if let Some(text) = special(value) {
        return text;
    }
    let (mantissa, exponent) = scientific(value, precision);
    normalize_exponent(format!("{mantissa}e{exponent}"))
}

#[cfg(test)]
mod tests {
    use super::{e, f, g};

    #[test]
    fn printf_conversions_match_c_style_spelling() {
        assert_eq!(g(1.234_567_89, 6), "1.23457");
        assert_eq!(g(0.000_01, 6), "1e-05");
        assert_eq!(f(1.25, 1), "1.2");
        assert_eq!(f(-0.0, 2), "-0.00");
        assert_eq!(e(1234.0, 2), "1.23e+03");
    }

    /// `g` as it was before `g_into` (heap strings throughout): the reference it must match.
    fn reference_g(value: f64, precision: usize) -> String {
        if let Some(text) = super::special(value) {
            return text;
        }
        let precision = precision.max(1);
        let (mut mantissa, exponent) = super::scientific(value, precision - 1);
        while mantissa.ends_with('0') {
            mantissa.pop();
        }
        if mantissa.ends_with('.') {
            mantissa.pop();
        }
        if exponent < -4 || (exponent >= 0 && exponent as usize >= precision) {
            return super::normalize_exponent(format!("{mantissa}e{exponent}"));
        }
        let negative = mantissa.starts_with('-');
        let digits: String = mantissa.chars().filter(|character| *character != '-' && *character != '.').collect();
        let shifted = 1 + exponent;
        let n = digits.len() as i32;
        let mut plain = String::new();
        if negative {
            plain.push('-');
        }
        if shifted <= 0 {
            plain.push_str("0.");
            plain.extend(std::iter::repeat_n('0', (-shifted) as usize));
            plain.push_str(&digits);
        } else if shifted >= n {
            plain.push_str(&digits);
            plain.extend(std::iter::repeat_n('0', (shifted - n) as usize));
        } else {
            let split = shifted as usize;
            plain.push_str(&digits[..split]);
            plain.push('.');
            plain.push_str(&digits[split..]);
        }
        plain
    }

    #[test]
    fn g_into_matches_the_heap_spelling() {
        let mut values = vec![0.0, -0.0, 1.0, -1.0, 0.5, 9.9999995, 99999.95, 1e-5, 1e-4, 123456.0, 1234567.0, 1e100, -2.5e-300, f64::MAX, f64::MIN_POSITIVE, 5e-324, f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        let mut x = 1.0e-7f64;
        while x < 1e9 {
            values.extend([x, -x, x * 1.000_000_7, 15.0 + x % 105.0]);
            x *= 1.37;
        }
        let mut out = String::new();
        for &v in &values {
            for precision in [0, 1, 2, 3, 6, 9, 12, 17, 30, 70] {
                super::g_into(&mut out, v, precision);
                assert_eq!(out, reference_g(v, precision), "{v} at {precision}");
            }
        }
    }
}
