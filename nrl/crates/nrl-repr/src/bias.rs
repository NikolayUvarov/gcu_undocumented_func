use nrl_core::{finite, Error, Result, Shape};

pub fn validate(values: &[f64], outputs: usize) -> Result<()> {
    if !values.is_empty() && values.len() != outputs {
        return Err(Error::Shape);
    }
    finite(values)
}

pub(crate) fn value(values: &[f64], index: usize) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values[index]
    }
}

#[derive(Clone, Copy, Default)]
pub struct Biases<'a> {
    pub gate: &'a [f64],
    pub up: &'a [f64],
    pub down: &'a [f64],
}
impl Biases<'_> {
    pub fn validate(self, shape: Shape) -> Result<()> {
        validate(self.gate, shape.channels())?;
        validate(self.up, shape.channels())?;
        validate(self.down, shape.output())
    }
}

#[derive(Default)]
pub struct BiasGradients<'a> {
    pub gate: &'a mut [f64],
    pub up: &'a mut [f64],
    pub down: &'a mut [f64],
}
impl BiasGradients<'_> {
    pub fn validate(&self, biases: Biases<'_>) -> Result<()> {
        for (grad, weights) in [
            (&*self.gate, biases.gate),
            (&*self.up, biases.up),
            (&*self.down, biases.down),
        ] {
            if grad.len() != weights.len() {
                return Err(Error::Shape);
            }
            finite(grad)?;
        }
        Ok(())
    }
}
