#![no_std]
#![forbid(unsafe_code)]

pub const MAX_ELEMENTS: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Shape,
    Limit,
    NonFinite,
    Grid,
}

pub type Result<T> = core::result::Result<T, Error>;

pub fn elements(a: usize, b: usize) -> Result<usize> {
    if a == 0 || b == 0 {
        return Err(Error::Shape);
    }
    let count = a.checked_mul(b).ok_or(Error::Limit)?;
    if count > MAX_ELEMENTS {
        return Err(Error::Limit);
    }
    Ok(count)
}

pub fn finite(values: &[f64]) -> Result<()> {
    if values.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(Error::NonFinite)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    input: usize,
    channels: usize,
    output: usize,
}

impl Shape {
    pub fn new(input: usize, channels: usize, output: usize) -> Result<Self> {
        elements(input, channels)?;
        elements(channels, output)?;
        elements(channels, 3)?;
        Ok(Self {
            input,
            channels,
            output,
        })
    }
    pub fn input(self) -> usize {
        self.input
    }
    pub fn channels(self) -> usize {
        self.channels
    }
    pub fn output(self) -> usize {
        self.output
    }
    pub fn gate_len(self) -> usize {
        self.input * self.channels
    }
    pub fn down_len(self) -> usize {
        self.channels * self.output
    }
    pub fn scratch_len(self) -> usize {
        self.channels * 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_refuses_zero_overflow_and_unbounded_work() {
        assert_eq!(Shape::new(0, 1, 1), Err(Error::Shape));
        assert_eq!(Shape::new(usize::MAX, 2, 1), Err(Error::Limit));
        assert_eq!(Shape::new(MAX_ELEMENTS, 2, 1), Err(Error::Limit));
        assert_eq!(Shape::new(3, 4, 2).unwrap().down_len(), 8);
    }
}
