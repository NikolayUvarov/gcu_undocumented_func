use nrl_core::{elements, finite, Error, Result};

pub struct Affine<'a> {
    inputs: usize,
    outputs: usize,
    weights: &'a [f64],
    bias: &'a [f64],
}

pub struct AffineGradients<'a> {
    pub weights: &'a mut [f64],
    pub bias: &'a mut [f64],
}

impl<'a> Affine<'a> {
    pub fn new(inputs: usize, outputs: usize, weights: &'a [f64], bias: &'a [f64]) -> Result<Self> {
        if weights.len() != elements(inputs, outputs)? {
            return Err(Error::Shape);
        }
        crate::bias::validate(bias, outputs)?;
        finite(weights)?;
        Ok(Self {
            inputs,
            outputs,
            weights,
            bias,
        })
    }
    pub fn forward(&self, input: &[f64], output: &mut [f64]) -> Result<()> {
        if input.len() != self.inputs || output.len() != self.outputs {
            return Err(Error::Shape);
        }
        finite(input)?;
        for (o, value) in output.iter_mut().enumerate() {
            *value = self.weights[o * self.inputs..(o + 1) * self.inputs]
                .iter()
                .zip(input)
                .map(|(w, x)| w * x)
                .sum::<f64>()
                + crate::bias::value(self.bias, o);
        }
        finite(output)
    }
    // Parameter gradients accumulate; the input gradient is replaced.
    pub fn backward(
        &self,
        input: &[f64],
        dy: &[f64],
        dx: &mut [f64],
        gradients: &mut AffineGradients<'_>,
    ) -> Result<()> {
        if input.len() != self.inputs
            || dy.len() != self.outputs
            || dx.len() != self.inputs
            || gradients.weights.len() != self.weights.len()
            || gradients.bias.len() != self.bias.len()
        {
            return Err(Error::Shape);
        }
        finite(input)?;
        finite(dy)?;
        finite(gradients.weights)?;
        finite(gradients.bias)?;
        dx.fill(0.0);
        for (o, d) in dy.iter().enumerate() {
            if !gradients.bias.is_empty() {
                gradients.bias[o] += d;
            }
            for i in 0..self.inputs {
                gradients.weights[o * self.inputs + i] += d * input[i];
                dx[i] += self.weights[o * self.inputs + i] * d;
            }
        }
        finite(dx)?;
        finite(gradients.weights)?;
        finite(gradients.bias)
    }
}
