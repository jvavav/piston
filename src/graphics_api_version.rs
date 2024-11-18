//! A library for storing graphics API versions.

/// A graphics API developed by Khronos Group.
/// See <https://en.wikipedia.org/wiki/OpenGL> for more information.
pub const OPENGL: &str = "OpenGL";
/// A graphics API developed by Khronos Group.
/// See <https://en.wikipedia.org/wiki/Vulkan>_(API) for more information.
pub const VULKAN: &str = "Vulkan";
/// A graphics API developed by Microsoft.
/// See <https://en.wikipedia.org/wiki/DirectX> for more information.
pub const DIRECTX: &str = "DirectX";
/// A graphics API developed by Apple.
/// See <https://en.wikipedia.org/wiki/Metal>_%28API%29 for more information.
pub const METAL: &str = "Metal";

use std::{borrow::Cow, error::Error};

/// Stores graphics API version.
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct Version {
    /// A string identifying the API.
    pub api: Cow<'static, str>,
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
}

impl Version {
    /// Creates a new OpenGL version.
    #[must_use]
    pub fn opengl(major: u32, minor: u32) -> Version {
        Version {
            api: OPENGL.into(),
            major,
            minor,
        }
    }

    /// Creates a new Vulkan version.
    #[must_use]
    pub fn vulkan(major: u32, minor: u32) -> Version {
        Version {
            api: VULKAN.into(),
            major,
            minor,
        }
    }

    /// Creates a new DirectX version.
    #[must_use]
    pub fn directx(major: u32, minor: u32) -> Version {
        Version {
            api: DIRECTX.into(),
            major,
            minor,
        }
    }

    /// Creates a new Metal version.
    #[must_use]
    pub fn metal(major: u32, minor: u32) -> Version {
        Version {
            api: METAL.into(),
            major,
            minor,
        }
    }

    /// Returns `true` if the API is OpenGL, `false` otherwise.
    #[must_use]
    pub fn is_opengl(&self) -> bool {
        self.api == OPENGL
    }

    /// Returns `true` if the API is Vulkan, `false` otherwise.
    #[must_use]
    pub fn is_vulkan(&self) -> bool {
        self.api == VULKAN
    }

    /// Returns `true` if the API is DirectX, `false` otherwise.
    #[must_use]
    pub fn is_directx(&self) -> bool {
        self.api == DIRECTX
    }

    /// Returns `true` if the API is metal, `false` otherwise.
    #[must_use]
    pub fn is_metal(&self) -> bool {
        self.api == METAL
    }
}

/// An error for when a graphics API is unsupported.
#[derive(Debug)]
pub struct UnsupportedGraphicsApiError {
    /// The requested graphics API.
    pub found: Cow<'static, str>,
    /// A list of supported graphics APIs.
    pub expected: Vec<Cow<'static, str>>,
}

impl std::fmt::Display for UnsupportedGraphicsApiError {
    fn fmt(&self, w: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let mut list = String::new();
        for ex in &self.expected {
            list.push_str(&format!("{}, ", ex));
        }
        write!(
            w,
            "Unsupported graphics API: Expected {}found {}",
            list, self.found
        )
    }
}

impl Error for UnsupportedGraphicsApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_it() {
        let a = Version::opengl(3, 2);
        let b = Version::opengl(4, 0);
        assert!(b > a);
    }
}
