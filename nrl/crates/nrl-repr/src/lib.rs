#![no_std]
#![forbid(unsafe_code)]

use nrl_core::{elements, finite, Error, Result, Shape};

pub mod factor;
pub mod pruning;

#[derive(Clone, Copy)]
pub struct Spline<'a> {
    cubic: bool,
    channels: usize,
    knots: usize,
    low: &'a [f64],
    high: &'a [f64],
    values: &'a [f64],
}

#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub value: f64,
    pub derivative: f64,
    pub left: usize,
    pub fraction: f64,
    pub indices: [usize; 4],
    pub weights: [f64; 4],
}

impl<'a> Spline<'a> {
    pub fn new(
        channels: usize,
        knots: usize,
        low: &'a [f64],
        high: &'a [f64],
        values: &'a [f64],
    ) -> Result<Self> {
        if knots < 2
            || low.len() != channels
            || high.len() != channels
            || values.len() != elements(channels, knots)?
        {
            return Err(Error::Shape);
        }
        finite(low)?;
        finite(high)?;
        finite(values)?;
        for c in 0..channels {
            let span = high[c] - low[c];
            let scale = (knots - 1) as f64 / span;
            if span <= 0.0 || !span.is_finite() || !scale.is_finite() || scale <= 0.0 {
                return Err(Error::Grid);
            }
            for j in 0..knots - 1 {
                if !((values[c * knots + j + 1] - values[c * knots + j]) * scale).is_finite() {
                    return Err(Error::NonFinite);
                }
            }
        }
        Ok(Self {
            cubic: false,
            channels,
            knots,
            low,
            high,
            values,
        })
    }

    pub fn hermite(
        channels: usize,
        knots: usize,
        low: &'a [f64],
        high: &'a [f64],
        values: &'a [f64],
    ) -> Result<Self> {
        let mut spline = Self::new(channels, knots, low, high, values)?;
        spline.cubic = true;
        Ok(spline)
    }

    pub fn sample(&self, channel: usize, x: f64) -> Result<Sample> {
        if channel >= self.channels {
            return Err(Error::Shape);
        }
        finite(&[x])?;
        let scale = (self.knots - 1) as f64 / (self.high[channel] - self.low[channel]);
        let u = (x - self.low[channel]) / (self.high[channel] - self.low[channel])
            * (self.knots - 1) as f64;
        finite(&[u])?;
        let segment = u.max(0.0).min((self.knots - 2) as f64) as usize;
        let left = channel * self.knots + segment;
        let fraction = u - segment as f64;
        let delta = self.values[left + 1] - self.values[left];
        let mut value = self.values[left] + delta * fraction;
        let mut derivative = delta * scale;
        let mut indices = [left, left + 1, left, left];
        let mut weights = [1.0 - fraction, fraction, 0.0, 0.0];
        if self.cubic && u >= 0.0 && u <= (self.knots - 1) as f64 {
            let t = fraction;
            let t2 = t * t;
            let t3 = t2 * t;
            let h = [
                2.0 * t3 - 3.0 * t2 + 1.0,
                t3 - 2.0 * t2 + t,
                -2.0 * t3 + 3.0 * t2,
                t3 - t2,
            ];
            let dh = [
                6.0 * t2 - 6.0 * t,
                3.0 * t2 - 4.0 * t + 1.0,
                -6.0 * t2 + 6.0 * t,
                3.0 * t2 - 2.0 * t,
            ];
            let basis = |h: [f64; 4]| {
                let mut b = [0.0, h[0], h[2], 0.0];
                if segment == 0 {
                    b[1] -= h[1];
                    b[2] += h[1];
                } else {
                    b[0] -= h[1] * 0.5;
                    b[2] += h[1] * 0.5;
                }
                if segment == self.knots - 2 {
                    b[1] -= h[3];
                    b[2] += h[3];
                } else {
                    b[1] -= h[3] * 0.5;
                    b[3] += h[3] * 0.5;
                }
                b
            };
            indices = [
                channel * self.knots + segment.saturating_sub(1),
                left,
                left + 1,
                channel * self.knots + (segment + 2).min(self.knots - 1),
            ];
            weights = basis(h);
            let derivatives = basis(dh);
            value = (0..4).map(|i| weights[i] * self.values[indices[i]]).sum();
            derivative = (0..4)
                .map(|i| derivatives[i] * self.values[indices[i]])
                .sum::<f64>()
                * scale;
        }
        finite(&[value, derivative])?;
        finite(&weights)?;
        Ok(Sample {
            value,
            derivative,
            left,
            fraction,
            indices,
            weights,
        })
    }
    pub fn coefficients(&self) -> usize {
        self.values.len()
    }
}

#[derive(Clone, Copy)]
pub enum Activation<'a> {
    Silu,
    Spline(Spline<'a>),
}

pub fn silu(x: f64) -> (f64, f64) {
    let sigmoid = if x >= 0.0 {
        1.0 / (1.0 + libm::exp(-x))
    } else {
        let exp = libm::exp(x);
        exp / (1.0 + exp)
    };
    (x * sigmoid, sigmoid * (1.0 + x * (1.0 - sigmoid)))
}

pub struct Block<'a> {
    shape: Shape,
    gate: &'a [f64],
    up: &'a [f64],
    down: &'a [f64],
    activation: Activation<'a>,
}

pub struct Gradients<'a> {
    pub gate: &'a mut [f64],
    pub up: &'a mut [f64],
    pub down: &'a mut [f64],
    pub values: &'a mut [f64],
}

impl<'a> Block<'a> {
    pub fn new(
        shape: Shape,
        gate: &'a [f64],
        up: &'a [f64],
        down: &'a [f64],
        activation: Activation<'a>,
    ) -> Result<Self> {
        if gate.len() != shape.gate_len()
            || up.len() != shape.gate_len()
            || down.len() != shape.down_len()
        {
            return Err(Error::Shape);
        }
        if let Activation::Spline(spline) = activation {
            if spline.channels != shape.channels() {
                return Err(Error::Shape);
            }
        }
        finite(gate)?;
        finite(up)?;
        finite(down)?;
        Ok(Self {
            shape,
            gate,
            up,
            down,
            activation,
        })
    }

    pub fn scratch_len(&self) -> usize {
        self.shape.scratch_len()
    }

    fn features(&self, input: &[f64], scratch: &mut [f64]) -> Result<()> {
        if input.len() != self.shape.input() || scratch.len() != self.shape.scratch_len() {
            return Err(Error::Shape);
        }
        finite(input)?;
        let n = self.shape.input();
        let w = self.shape.channels();
        for c in 0..w {
            let mut gate = 0.0;
            let mut up = 0.0;
            for (i, x) in input.iter().enumerate() {
                gate += self.gate[c * n + i] * x;
                up += self.up[c * n + i] * x;
            }
            finite(&[gate, up])?;
            scratch[c] = gate;
            scratch[w + c] = up;
            scratch[2 * w + c] = match self.activation {
                Activation::Silu => silu(gate).0,
                Activation::Spline(spline) => spline.sample(c, gate)?.value,
            };
        }
        finite(scratch)
    }

    pub fn forward(&self, input: &[f64], output: &mut [f64], scratch: &mut [f64]) -> Result<()> {
        if output.len() != self.shape.output() {
            return Err(Error::Shape);
        }
        self.features(input, scratch)?;
        let w = self.shape.channels();
        for (o, y) in output.iter_mut().enumerate() {
            *y = (0..w)
                .map(|c| self.down[o * w + c] * scratch[w + c] * scratch[2 * w + c])
                .sum();
        }
        finite(output)
    }

    // Parameter gradients accumulate; the input gradient is replaced.
    pub fn backward(
        &self,
        input: &[f64],
        output_gradient: &[f64],
        input_gradient: &mut [f64],
        gradients: &mut Gradients<'_>,
        scratch: &mut [f64],
    ) -> Result<()> {
        let n = self.shape.input();
        let w = self.shape.channels();
        let count = match self.activation {
            Activation::Silu => 0,
            Activation::Spline(s) => s.coefficients(),
        };
        if input_gradient.len() != n
            || output_gradient.len() != self.shape.output()
            || gradients.gate.len() != self.gate.len()
            || gradients.up.len() != self.up.len()
            || gradients.down.len() != self.down.len()
            || gradients.values.len() != count
        {
            return Err(Error::Shape);
        }
        finite(output_gradient)?;
        finite(gradients.gate)?;
        finite(gradients.up)?;
        finite(gradients.down)?;
        finite(gradients.values)?;
        self.features(input, scratch)?;
        input_gradient.fill(0.0);
        for c in 0..w {
            let mut feature_gradient = 0.0;
            for (o, dy) in output_gradient.iter().enumerate() {
                feature_gradient += self.down[o * w + c] * dy;
                gradients.down[o * w + c] += dy * scratch[w + c] * scratch[2 * w + c];
            }
            let (activation_gradient, sample) = match self.activation {
                Activation::Silu => (silu(scratch[c]).1, None),
                Activation::Spline(spline) => {
                    let sample = spline.sample(c, scratch[c])?;
                    (sample.derivative, Some(sample))
                }
            };
            let dz = feature_gradient * scratch[w + c] * activation_gradient;
            let du = feature_gradient * scratch[2 * w + c];
            for i in 0..n {
                gradients.gate[c * n + i] += dz * input[i];
                gradients.up[c * n + i] += du * input[i];
                input_gradient[i] += dz * self.gate[c * n + i] + du * self.up[c * n + i];
            }
            if let Some(sample) = sample {
                let dv = feature_gradient * scratch[w + c];
                for i in 0..4 {
                    gradients.values[sample.indices[i]] += dv * sample.weights[i];
                }
            }
        }
        finite(input_gradient)?;
        finite(gradients.gate)?;
        finite(gradients.up)?;
        finite(gradients.down)?;
        finite(gradients.values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knots_use_the_right_segment_and_tails_are_linear() {
        let spline = Spline::new(1, 3, &[0.0], &[2.0], &[0.0, 1.0, 4.0]).unwrap();
        assert_eq!(spline.sample(0, -1.0).unwrap().value, -1.0);
        assert_eq!(spline.sample(0, 1.0).unwrap().derivative, 3.0);
        assert_eq!(spline.sample(0, 3.0).unwrap().value, 7.0);
    }

    #[test]
    fn refuses_bad_grids_and_non_finite_data() {
        assert!(matches!(
            Spline::new(1, 2, &[1.0], &[1.0], &[0.0, 1.0]),
            Err(Error::Grid)
        ));
        assert!(matches!(
            Spline::new(1, 2, &[0.0], &[1.0], &[f64::NAN, 1.0]),
            Err(Error::NonFinite)
        ));
        let spline = Spline::new(1, 2, &[0.0], &[1.0], &[0.0, 1.0]).unwrap();
        assert!(matches!(
            spline.sample(0, f64::INFINITY),
            Err(Error::NonFinite)
        ));
        assert!(matches!(spline.sample(1, 0.5), Err(Error::Shape)));
    }

    #[test]
    fn silu_handles_saturation_without_overflow() {
        assert_eq!(silu(1000.0), (1000.0, 1.0));
        assert_eq!(silu(-1000.0).0, 0.0);
    }

    #[test]
    fn caller_buffer_mismatches_and_arithmetic_overflow_are_errors() {
        let shape = Shape::new(1, 1, 1).unwrap();
        let block = Block::new(shape, &[1.0], &[1.0], &[1.0], Activation::Silu).unwrap();
        assert_eq!(
            block.forward(&[0.5], &mut [0.0], &mut []),
            Err(Error::Shape)
        );
        assert_eq!(
            block.forward(&[0.5], &mut [], &mut [0.0; 3]),
            Err(Error::Shape)
        );
        let mut g = [0.0];
        let mut u = [0.0];
        let mut d = [0.0];
        let mut values = [];
        let mut gradients = Gradients {
            gate: &mut g,
            up: &mut u,
            down: &mut d,
            values: &mut values,
        };
        assert_eq!(
            block.backward(&[0.5], &[], &mut [0.0], &mut gradients, &mut [0.0; 3]),
            Err(Error::Shape)
        );
        let huge = Block::new(shape, &[f64::MAX], &[1.0], &[1.0], Activation::Silu).unwrap();
        assert_eq!(
            huge.forward(&[2.0], &mut [0.0], &mut [0.0; 3]),
            Err(Error::NonFinite)
        );
    }
}
