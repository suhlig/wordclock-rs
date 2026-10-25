//! Rendered frame: logical pixels plus physical padding.

use crate::color::{GammaLut, Rgb};

/// A complete frame at logical resolution, ready for correction and output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    logical: Vec<Rgb>,
    physical_len: usize,
}

impl Frame {
    /// Creates a frame from logical pixels and the physical strip length.
    pub fn new(logical: Vec<Rgb>, physical_len: usize) -> Self {
        assert!(
            logical.len() <= physical_len,
            "physical strip ({physical_len}) is shorter than the logical frame ({})",
            logical.len()
        );

        Self {
            logical,
            physical_len,
        }
    }

    /// Creates a black frame with 288 logical pixels.
    pub fn blank(physical_len: usize) -> Self {
        Self::new(vec![Rgb::BLACK; crate::LOGICAL_PIXELS], physical_len)
    }

    /// Sets one logical pixel.
    pub fn set(&mut self, index: usize, color: Rgb) {
        self.logical[index] = color;
    }

    /// The logical pixels, in stripe order.
    pub fn logical(&self) -> &[Rgb] {
        &self.logical
    }

    /// The physical strip length, including padding.
    pub fn physical_len(&self) -> usize {
        self.physical_len
    }

    /// Applies gamma and whitepoint correction to the 288 logical pixels.
    pub fn corrected_logical(&self, lut: &GammaLut) -> Vec<Rgb> {
        self.logical
            .iter()
            .map(|&color| lut.correct(color))
            .collect()
    }

    /// Applies correction and pads the physical strip with black.
    pub fn corrected_physical(&self, lut: &GammaLut) -> Vec<Rgb> {
        let mut pixels = vec![Rgb::BLACK; self.physical_len];

        for (pixel, &color) in pixels.iter_mut().zip(&self.logical) {
            *pixel = lut.correct(color);
        }

        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_is_black_and_logical_pixels_are_corrected() {
        let mut frame = Frame::blank(300);
        frame.set(0, Rgb::new(255, 0, 0));

        let lut = GammaLut::new([1.0, 1.0, 1.0]);
        let pixels = frame.corrected_physical(&lut);

        assert_eq!(pixels.len(), 300);
        assert_eq!(pixels[0], Rgb::new(255, 0, 0));
        assert!(pixels[288..].iter().all(|&pixel| pixel == Rgb::BLACK));
    }

    #[test]
    fn logical_pixels_stay_unpadded() {
        let frame = Frame::blank(300);
        let lut = GammaLut::new([1.0, 1.0, 1.0]);

        assert_eq!(frame.corrected_logical(&lut).len(), crate::LOGICAL_PIXELS);
    }
}
