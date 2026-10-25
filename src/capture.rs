//! What part of the desktop to record.
//!
//! This milestone models full-screen and rectangular capture. Monitor
//! selection needs display discovery first, so it arrives with a later
//! milestone rather than as an unusable flag.

use std::error::Error;
use std::fmt;

use crate::monitor::Monitor;

/// The area to capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Capture {
    /// The whole display, sized by FFmpeg's `x11grab` default.
    #[default]
    FullScreen,
    /// One monitor, resolved to its geometry before recording starts.
    Monitor(u32),
    /// An explicit rectangle in X11 root coordinates.
    Region(Region),
    /// Let FFmpeg's `x11grab` pick the rectangle with the mouse.
    SelectRegion,
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

/// Resolve a monitor index to its rectangle.
pub fn select(index: u32, monitors: &[Monitor]) -> Result<Region, CaptureError> {
    let monitor = monitors
        .get(index as usize)
        .ok_or(CaptureError::MonitorOutOfRange {
            index,
            available: monitors.len(),
        })?;

    Ok(Region {
        width: monitor.width,
        height: monitor.height,
        x: monitor.x,
        y: monitor.y,
    })
}

/// Check that an explicit rectangle fits inside the connected desktop.
pub fn validate_region(region: Region, monitors: &[Monitor]) -> Result<(), CaptureError> {
    let Some((screen_width, screen_height)) = desktop_bounds(monitors) else {
        return Err(CaptureError::NoMonitors);
    };

    let right = u64::from(region.x.max(0) as u32) + u64::from(region.width);
    let bottom = u64::from(region.y.max(0) as u32) + u64::from(region.height);

    if right > u64::from(screen_width) || bottom > u64::from(screen_height) {
        return Err(CaptureError::RegionOutOfBounds {
            region,
            screen_width,
            screen_height,
        });
    }

    Ok(())
}

/// The desktop size implied by the connected monitor rectangles.
fn desktop_bounds(monitors: &[Monitor]) -> Option<(u32, u32)> {
    let width = monitors
        .iter()
        .map(|monitor| (monitor.x + monitor.width as i32).max(0) as u32)
        .max()?;
    let height = monitors
        .iter()
        .map(|monitor| (monitor.y + monitor.height as i32).max(0) as u32)
        .max()?;
    Some((width, height))
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
    /// The requested monitor index does not exist.
    MonitorOutOfRange { index: u32, available: usize },
    /// No connected monitors, so desktop bounds are unknown.
    NoMonitors,
    /// The rectangle does not fit inside the desktop.
    RegionOutOfBounds {
        region: Region,
        screen_width: u32,
        screen_height: u32,
    },
}

impl CaptureError {
    /// Exit code for capture-selection problems (see README).
    pub const EXIT_CODE: u8 = 2;
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
            Self::MonitorOutOfRange { index, available } => write!(
                f,
                "monitor {index} does not exist; {available} connected monitor(s) found"
            ),
            Self::NoMonitors => write!(
                f,
                "cannot validate the region: no connected monitors were found"
            ),
            Self::RegionOutOfBounds {
                region,
                screen_width,
                screen_height,
            } => write!(
                f,
                "region {}x{}+{}+{} does not fit within the {screen_width}x{screen_height} desktop",
                region.width, region.height, region.x, region.y,
            ),
        }
    }
}

impl Error for CaptureError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(width: u32, height: u32, x: i32, y: i32) -> Monitor {
        Monitor {
            name: "eDP-1".to_owned(),
            width,
            height,
            x,
            y,
            primary: true,
        }
    }

    #[test]
    fn selects_a_monitor_rectangle() {
        let monitors = [monitor(1920, 1200, 0, 0), monitor(2560, 1440, 1920, 0)];

        assert_eq!(
            select(1, &monitors).expect("second monitor"),
            Region {
                width: 2560,
                height: 1440,
                x: 1920,
                y: 0,
            }
        );
    }

    #[test]
    fn accepts_a_region_inside_the_desktop() {
        let monitors = [monitor(1920, 1200, 0, 0)];

        assert!(validate_region(Region::parse("640x480+100+80").unwrap(), &monitors).is_ok());
        assert!(validate_region(Region::parse("1920x1200+0+0").unwrap(), &monitors).is_ok());
    }

    #[test]
    fn rejects_a_region_outside_the_desktop() {
        let monitors = [monitor(1920, 1200, 0, 0)];
        let region = Region::parse("640x480+1500+800").unwrap();

        assert_eq!(
            validate_region(region, &monitors).unwrap_err(),
            CaptureError::RegionOutOfBounds {
                region,
                screen_width: 1920,
                screen_height: 1200,
            }
        );
    }

    #[test]
    fn uses_the_union_of_monitors_for_bounds() {
        let monitors = [monitor(1920, 1200, 0, 0), monitor(2560, 1440, 1920, 0)];

        assert!(validate_region(Region::parse("2560x1440+1920+0").unwrap(), &monitors).is_ok());
        assert!(validate_region(Region::parse("2560x1440+1920+1").unwrap(), &monitors).is_err());
    }

    #[test]
    fn rejects_a_region_without_monitors() {
        assert_eq!(
            validate_region(Region::parse("640x480+0+0").unwrap(), &[]).unwrap_err(),
            CaptureError::NoMonitors
        );
    }

    #[test]
    fn rejects_an_unknown_monitor() {
        let monitors = [monitor(1920, 1200, 0, 0)];

        assert_eq!(
            select(3, &monitors).unwrap_err(),
            CaptureError::MonitorOutOfRange {
                index: 3,
                available: 1,
            }
        );
    }

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
