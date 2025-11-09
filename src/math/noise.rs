use std::ops::Add;
use noiz::NoiseFunction;
use noiz::rng::NoiseRng;

pub struct Combined<N, M>(pub N, pub M);

impl<I: Copy, N: NoiseFunction<I>, M: NoiseFunction<I, Output: Add<N::Output>>> NoiseFunction<I>
for Combined<N, M>
{
    type Output = <M::Output as Add<N::Output>>::Output;

    #[inline]
    fn evaluate(&self, input: I, seeds: &mut NoiseRng) -> Self::Output {
        self.1.evaluate(input, seeds) + self.0.evaluate(input, seeds)
    }
}