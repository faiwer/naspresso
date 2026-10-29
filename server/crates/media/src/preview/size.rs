use super::PreviewError;

/// A preview's longest side. Smaller images aren't upscaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSize {
    S128,
    S256,
    S512,
}

impl PreviewSize {
    /// Largest first: each size is resized from the previous one.
    pub(super) const DESCENDING: [PreviewSize; 3] =
        [PreviewSize::S512, PreviewSize::S256, PreviewSize::S128];

    pub fn px(self) -> u32 {
        match self {
            PreviewSize::S128 => 128,
            PreviewSize::S256 => 256,
            PreviewSize::S512 => 512,
        }
    }
}

impl TryFrom<u32> for PreviewSize {
    type Error = PreviewError;

    fn try_from(px: u32) -> Result<Self, Self::Error> {
        Self::DESCENDING
            .into_iter()
            .find(|size| size.px() == px)
            .ok_or(PreviewError::InvalidSize(px))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_known_sizes() {
        assert_eq!(PreviewSize::try_from(256).unwrap(), PreviewSize::S256);
        assert!(PreviewSize::try_from(300).is_err());
    }
}
