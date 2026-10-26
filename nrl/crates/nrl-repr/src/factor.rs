use crate::silu;
use nrl_core::{elements, finite, Error, Result, Shape};

#[derive(Clone, Copy)]
pub struct Factor<'a> {
    inputs: usize,
    outputs: usize,
    rank: usize,
    left: &'a [f64],
    right: &'a [f64],
    bias: &'a [f64],
}

pub struct FactorGradients<'a> {
    pub bias: &'a mut [f64],
    pub left: &'a mut [f64],
    pub right: &'a mut [f64],
}

impl<'a> Factor<'a> {
    pub fn new(
        inputs: usize,
        outputs: usize,
        rank: usize,
        left: &'a [f64],
        right: &'a [f64],
        bias: &'a [f64],
    ) -> Result<Self> {
        if rank == 0
            || rank > inputs.min(outputs)
            || left.len() != elements(outputs, rank)?
            || right.len() != elements(rank, inputs)?
        {
            return Err(Error::Shape);
        }
        crate::bias::validate(bias, outputs)?;
        finite(left)?;
        finite(right)?;
        Ok(Self {
            inputs,
            outputs,
            rank,
            left,
            right,
            bias,
        })
    }
    pub fn rank(self) -> usize {
        self.rank
    }
    pub fn reconstruct(&self, weights: &mut [f64]) -> Result<()> {
        if weights.len() != elements(self.outputs, self.inputs)? {
            return Err(Error::Shape);
        }
        for o in 0..self.outputs {
            for i in 0..self.inputs {
                weights[o * self.inputs + i] = (0..self.rank)
                    .map(|r| self.left[o * self.rank + r] * self.right[r * self.inputs + i])
                    .sum();
            }
        }
        finite(weights)
    }
    pub fn forward(&self, input: &[f64], output: &mut [f64], scratch: &mut [f64]) -> Result<()> {
        if input.len() != self.inputs || output.len() != self.outputs || scratch.len() != self.rank
        {
            return Err(Error::Shape);
        }
        finite(input)?;
        for (r, value) in scratch.iter_mut().enumerate() {
            *value = (0..self.inputs)
                .map(|i| self.right[r * self.inputs + i] * input[i])
                .sum();
        }
        finite(scratch)?;
        for (o, value) in output.iter_mut().enumerate() {
            *value = (0..self.rank)
                .map(|r| self.left[o * self.rank + r] * scratch[r])
                .sum::<f64>()
                + crate::bias::value(self.bias, o);
        }
        finite(output)
    }
    // All gradient buffers accumulate; callers initialize them explicitly.
    pub fn backward(
        &self,
        input: &[f64],
        dy: &[f64],
        dx: &mut [f64],
        gradients: &mut FactorGradients<'_>,
        scratch: &mut [f64],
    ) -> Result<()> {
        if input.len() != self.inputs
            || dy.len() != self.outputs
            || dx.len() != self.inputs
            || gradients.left.len() != self.left.len()
            || gradients.bias.len() != self.bias.len()
            || gradients.right.len() != self.right.len()
            || scratch.len() != self.rank
        {
            return Err(Error::Shape);
        }
        finite(gradients.bias)?;
        finite(input)?;
        finite(dy)?;
        finite(dx)?;
        finite(gradients.left)?;
        finite(gradients.right)?;
        for (r, value) in scratch.iter_mut().enumerate() {
            *value = (0..self.inputs)
                .map(|i| self.right[r * self.inputs + i] * input[i])
                .sum();
        }
        finite(scratch)?;
        for (bias, dy) in gradients.bias.iter_mut().zip(dy) {
            *bias += dy;
        }
        for r in 0..self.rank {
            let mut dh = 0.0;
            for (o, d) in dy.iter().enumerate() {
                gradients.left[o * self.rank + r] += d * scratch[r];
                dh += self.left[o * self.rank + r] * d;
            }
            for i in 0..self.inputs {
                gradients.right[r * self.inputs + i] += dh * input[i];
                dx[i] += self.right[r * self.inputs + i] * dh;
            }
        }
        finite(dx)?;
        finite(gradients.left)?;
        finite(gradients.right)?;
        finite(gradients.bias)
    }
}

pub struct SvdBlock<'a> {
    shape: Shape,
    gate: Factor<'a>,
    up: Factor<'a>,
    down: Factor<'a>,
    scratch: usize,
}

pub struct SvdGradients<'a> {
    pub gate: FactorGradients<'a>,
    pub up: FactorGradients<'a>,
    pub down: FactorGradients<'a>,
}

impl<'a> SvdBlock<'a> {
    pub fn new(shape: Shape, gate: Factor<'a>, up: Factor<'a>, down: Factor<'a>) -> Result<Self> {
        if (gate.inputs, gate.outputs) != (shape.input(), shape.channels())
            || (up.inputs, up.outputs) != (shape.input(), shape.channels())
            || (down.inputs, down.outputs) != (shape.channels(), shape.output())
        {
            return Err(Error::Shape);
        }
        let rank = gate.rank.max(up.rank).max(down.rank);
        let scratch = elements(shape.channels(), 7)?
            .checked_add(rank)
            .ok_or(Error::Limit)?;
        if scratch > nrl_core::MAX_ELEMENTS {
            return Err(Error::Limit);
        }
        Ok(Self {
            shape,
            gate,
            up,
            down,
            scratch,
        })
    }
    pub fn scratch_len(&self) -> usize {
        self.scratch
    }
    fn features(&self, input: &[f64], scratch: &mut [f64]) -> Result<()> {
        if scratch.len() != self.scratch {
            return Err(Error::Shape);
        }
        let w = self.shape.channels();
        let (buffers, temp) = scratch.split_at_mut(w * 7);
        let (gate, rest) = buffers.split_at_mut(w);
        let (up, rest) = rest.split_at_mut(w);
        let (activation, rest) = rest.split_at_mut(w);
        let features = &mut rest[..w];
        self.gate
            .forward(input, gate, &mut temp[..self.gate.rank])?;
        self.up.forward(input, up, &mut temp[..self.up.rank])?;
        for c in 0..w {
            activation[c] = silu(gate[c]).0;
            features[c] = activation[c] * up[c];
        }
        finite(activation)?;
        finite(features)
    }
    pub fn forward(&self, input: &[f64], output: &mut [f64], scratch: &mut [f64]) -> Result<()> {
        self.features(input, scratch)?;
        let w = self.shape.channels();
        let (buffers, temp) = scratch.split_at_mut(w * 7);
        self.down
            .forward(&buffers[3 * w..4 * w], output, &mut temp[..self.down.rank])
    }
    // Parameter gradients accumulate; the input gradient is replaced.
    pub fn backward(
        &self,
        input: &[f64],
        dy: &[f64],
        dx: &mut [f64],
        gradients: &mut SvdGradients<'_>,
        scratch: &mut [f64],
    ) -> Result<()> {
        if dx.len() != self.shape.input() {
            return Err(Error::Shape);
        }
        self.features(input, scratch)?;
        let w = self.shape.channels();
        let (buffers, temp) = scratch.split_at_mut(w * 7);
        let (gate, rest) = buffers.split_at_mut(w);
        let (up, rest) = rest.split_at_mut(w);
        let (activation, rest) = rest.split_at_mut(w);
        let (features, rest) = rest.split_at_mut(w);
        let (df, rest) = rest.split_at_mut(w);
        let (dg, du) = rest.split_at_mut(w);
        df.fill(0.0);
        self.down.backward(
            features,
            dy,
            df,
            &mut gradients.down,
            &mut temp[..self.down.rank],
        )?;
        for c in 0..w {
            dg[c] = df[c] * up[c] * silu(gate[c]).1;
            du[c] = df[c] * activation[c];
        }
        finite(dg)?;
        finite(du)?;
        dx.fill(0.0);
        self.gate.backward(
            input,
            dg,
            dx,
            &mut gradients.gate,
            &mut temp[..self.gate.rank],
        )?;
        self.up
            .backward(input, du, dx, &mut gradients.up, &mut temp[..self.up.rank])
    }
}
