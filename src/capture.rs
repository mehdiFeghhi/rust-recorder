//! What part of the desktop to record.
//!
//! This milestone models full-screen and rectangular capture. Monitor
//! selection needs display discovery first, so it arrives with a later
//! milestone rather than as an unusable flag.

use std::error::Error;
use std::fmt;

/// The area to capture.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Capture {
    /// The whole display, sized by FFmpeg's `x11grab` default.
    #[default]
    FullScreen,
    /// An explicit rectangle in X11 root coordinates.
    Region(Region),
}

/// A rectangle in X11 root coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

impl Region {
    /// Parse `WIDTHxHEIGHT+X+Y`, for example `1280x720+100+80`.
    pub fn parse(text: &str) -> Result<Self, CaptureError> {
        let malformed = || CaptureError::MalformedRegion(text.to_owned());

        let (size, offset) = text.split_once('+').ok_or_else(malformed)?;
        let (width, height) = size.split_once('x').ok_or_else(malformed)?;
        let (x, y) = offset.split_once('+').ok_or_else(malformed)?;

        let width: u32 = width.parse().map_err(|_| malformed())?;
        let height: u32 = height.parse().map_err(|_| malformed())?;
        let x: i32 = x.parse().map_err(|_| malformed())?;
        let y: i32 = y.parse().map_err(|_| malformed())?;

        if width == 0 || height == 0 {
            return Err(CaptureError::ZeroDimension);
        }
        if x < 0 || y < 0 {
            return Err(CaptureError::NegativeOrigin);
        }

        Ok(Self {
            width,
            height,
            x,
            y,
        })
    }

    /// The `-video_size` value FFmpeg expects.
    pub fn video_size(self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    /// The offset appended to the X11 input, for example `+100,80`.
    pub fn offset(self) -> String {
        format!("+{},{}", self.x, self.y)
    }
}

/// Why a capture region could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    /// The text does not follow `WIDTHxHEIGHT+X+Y`.
    MalformedRegion(String),
    /// Width or height is zero.
    ZeroDimension,
    /// The origin is negative; X11 root coordinates start at 0,0.
    NegativeOrigin,
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedRegion(value) => write!(
                f,
                "invalid region {value:?}: expected WIDTHxHEIGHT+X+Y, e.g. 1280x720+100+80"
            ),
            Self::ZeroDimension => write!(f, "region width and height must be greater than zero"),
            Self::NegativeOrigin => {
                write!(f, "region origin must not be negative")
            }
        }
    }
}

impl Error for CaptureError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_region() {
        let region = Region::parse("1280x720+100+80").expect("valid region");
        assert_eq!(
            region,
            Region {
                width: 1280,
                height: 720,
                x: 100,
                y: 80
            }
        );
        assert_eq!(region.video_size(), "1280x720");
        assert_eq!(region.offset(), "+100,80");
    }

    #[test]
    fn parses_a_zero_origin() {
        let region = Region::parse("640x480+0+0").expect("valid region");
        assert_eq!((region.x, region.y), (0, 0));
    }

    #[test]
    fn rejects_zero_dimensions() {
        assert_eq!(
            Region::parse("0x480+0+0").unwrap_err(),
            CaptureError::ZeroDimension
        );
        assert_eq!(
            Region::parse("640x0+0+0").unwrap_err(),
            CaptureError::ZeroDimension
        );
    }

    #[test]
    fn rejects_negative_origin() {
        assert_eq!(
            Region::parse("640x480+-1+0").unwrap_err(),
            CaptureError::NegativeOrigin
        );
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",
            "1280",
            "1280x720",
            "1280x720+100",
            "axb+1+2",
            "1280x720+1+2+3",
            "1280x720+1.5+2",
        ] {
            let error = Region::parse(bad).expect_err(bad);
            assert!(
                matches!(error, CaptureError::MalformedRegion(_)),
                "{bad:?} produced {error:?}"
            );
        }
    }
}
