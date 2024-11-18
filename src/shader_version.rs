//! detecting and picking compatible shaders

use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};

use graphics_api_version::Version;

use crate::graphics_api_version;

/// Shader picker.
pub struct Shaders<'a, V, S: 'a + ?Sized>(BTreeMap<V, &'a S>);

impl<V, S: ?Sized> Default for Shaders<'_, V, S>
where
    V: PickShader,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, V, S: ?Sized> Shaders<'a, V, S>
where
    V: PickShader,
{
    /// Creates a new shader picker.
    pub fn new() -> Self {
        Shaders(BTreeMap::new())
    }

    /// Sets source for a shader version.
    pub fn set(&mut self, version: V, source: &'a S) -> &mut Self {
        self.0.insert(version, source);
        self
    }

    /// Get the closest shader to a shader version.
    pub fn get(&self, version: V) -> Option<&S> {
        version.pick_shader(self)
    }
}

/// Implemented by shader version enums.
pub trait PickShader: Ord + Sized {
    /// Pick shader.
    fn pick_shader<'a, S: ?Sized>(self, shaders: &Shaders<'a, Self, S>) -> Option<&'a S>;
}

/// Models versions of OpenGL Shader Language (GLSL)
///
/// For OpenGL version 3.3 and above,
/// the GLSL version is the same as the OpenGL version.
///
/// Source: <http://www.opengl.org/wiki/Core_Language>_%28GLSL%29
#[allow(missing_docs)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum GLSL {
    V1_10,
    V1_20,
    V1_30,
    V1_40,
    V1_50,
    V3_30,
    V4_00,
    V4_10,
    V4_20,
    V4_30,
    V4_40,
    V4_50,
}

impl GLSL {
    /// Gets OpenGL version associated with GLSL.
    #[allow(non_snake_case)]
    pub fn to_opengl(&self) -> OpenGL {
        match *self {
            GLSL::V1_10 => OpenGL::V2_0,
            GLSL::V1_20 => OpenGL::V2_1,
            GLSL::V1_30 => OpenGL::V3_0,
            GLSL::V1_40 => OpenGL::V3_1,
            GLSL::V1_50 => OpenGL::V3_2,
            GLSL::V3_30 => OpenGL::V3_3,
            GLSL::V4_00 => OpenGL::V4_0,
            GLSL::V4_10 => OpenGL::V4_1,
            GLSL::V4_20 => OpenGL::V4_2,
            GLSL::V4_30 => OpenGL::V4_3,
            GLSL::V4_40 => OpenGL::V4_4,
            GLSL::V4_50 => OpenGL::V4_5,
        }
    }
}

impl PickShader for GLSL {
    fn pick_shader<'a, S: ?Sized>(self, shaders: &Shaders<'a, Self, S>) -> Option<&'a S> {
        // OpenGL since 3.2 in core profile doesn't support GLSL lower than 1.50.
        // Since there are no compatible shader in this case, it will return `None`.
        let low = if self < GLSL::V1_50 {
            GLSL::V1_10
        } else {
            GLSL::V1_50
        };
        shaders
            .0
            .iter()
            .skip_while(|&(v, _)| *v < low)
            .take_while(|&(v, _)| *v <= self)
            .last()
            .map(|(_, &s)| s)
    }
}

impl FromStr for GLSL {
    type Err = ParseGLSLError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1.10" => Ok(GLSL::V1_10),
            "1.20" => Ok(GLSL::V1_20),
            "1.30" => Ok(GLSL::V1_30),
            "1.40" => Ok(GLSL::V1_40),
            "1.50" => Ok(GLSL::V1_50),
            "3.30" => Ok(GLSL::V3_30),
            "4.00" => Ok(GLSL::V4_00),
            "4.10" => Ok(GLSL::V4_10),
            "4.20" => Ok(GLSL::V4_20),
            "4.30" => Ok(GLSL::V4_30),
            "4.40" => Ok(GLSL::V4_40),
            "4.50" => Ok(GLSL::V4_50),
            error => Err(ParseGLSLError {
                input: error.into(),
            }),
        }
    }
}

/// Represents an error while trying to get `GLSL` from `&str`.
#[derive(Debug)]
pub struct ParseGLSLError {
    input: String,
}

impl fmt::Display for ParseGLSLError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "`{}` is not a valid GLSL version", self.input)
    }
}

impl Error for ParseGLSLError {
    fn description(&self) -> &str {
        "Invalid GLSL version"
    }
}

/// Models versions of OpenGL
#[allow(non_camel_case_types, missing_docs)]
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum OpenGL {
    V2_0,
    V2_1,
    V3_0,
    V3_1,
    V3_2,
    V3_3,
    V4_0,
    V4_1,
    V4_2,
    V4_3,
    V4_4,
    V4_5,
}

impl From<OpenGL> for Version {
    fn from(val: OpenGL) -> Self {
        let (major, minor) = val.get_major_minor();
        Version::opengl(major as u32, minor as u32)
    }
}

impl OpenGL {
    /// Creates a new `OpenGL` version from graphics API version.
    pub fn from_api(val: Version) -> Option<Self> {
        if val.api == "OpenGL" {
            Some(match (val.major, val.minor) {
                (2, 0) => Self::V2_0,
                (2, 1) => Self::V2_1,
                (3, 0) => Self::V3_0,
                (3, 1) => Self::V3_1,
                (3, 2) => Self::V3_2,
                (3, 3) => Self::V3_3,
                (4, 0) => Self::V4_0,
                (4, 1) => Self::V4_1,
                (4, 2) => Self::V4_2,
                (4, 3) => Self::V4_3,
                (4, 4) => Self::V4_4,
                (4, 5) => Self::V4_5,
                (_, _) => return None,
            })
        } else {
            None
        }
    }

    /// Gets the minor version of OpenGL.
    pub fn get_major_minor(&self) -> (isize, isize) {
        match *self {
            OpenGL::V2_0 => (2, 0),
            OpenGL::V2_1 => (2, 1),
            OpenGL::V3_0 => (3, 0),
            OpenGL::V3_1 => (3, 1),
            OpenGL::V3_2 => (3, 2),
            OpenGL::V3_3 => (3, 3),
            OpenGL::V4_0 => (4, 0),
            OpenGL::V4_1 => (4, 1),
            OpenGL::V4_2 => (4, 2),
            OpenGL::V4_3 => (4, 3),
            OpenGL::V4_4 => (4, 4),
            OpenGL::V4_5 => (4, 5),
        }
    }

    /// Gets GLSL version associated with OpenGL.
    #[allow(non_snake_case)]
    pub fn to_glsl(&self) -> GLSL {
        match *self {
            OpenGL::V2_0 => GLSL::V1_10,
            OpenGL::V2_1 => GLSL::V1_20,
            OpenGL::V3_0 => GLSL::V1_30,
            OpenGL::V3_1 => GLSL::V1_40,
            OpenGL::V3_2 => GLSL::V1_50,
            OpenGL::V3_3 => GLSL::V3_30,
            OpenGL::V4_0 => GLSL::V4_00,
            OpenGL::V4_1 => GLSL::V4_10,
            OpenGL::V4_2 => GLSL::V4_20,
            OpenGL::V4_3 => GLSL::V4_30,
            OpenGL::V4_4 => GLSL::V4_40,
            OpenGL::V4_5 => GLSL::V4_50,
        }
    }
}

impl FromStr for OpenGL {
    type Err = ParseOpenGLError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "2.0" => Ok(OpenGL::V2_0),
            "2.1" => Ok(OpenGL::V2_1),
            "3.0" => Ok(OpenGL::V3_0),
            "3.1" => Ok(OpenGL::V3_1),
            "3.2" => Ok(OpenGL::V3_2),
            "3.3" => Ok(OpenGL::V3_3),
            "4.0" => Ok(OpenGL::V4_0),
            "4.1" => Ok(OpenGL::V4_1),
            "4.2" => Ok(OpenGL::V4_2),
            "4.3" => Ok(OpenGL::V4_3),
            "4.4" => Ok(OpenGL::V4_4),
            "4.5" => Ok(OpenGL::V4_5),
            error => Err(ParseOpenGLError {
                input: error.into(),
            }),
        }
    }
}

/// Represents an error while trying to get `OpenGL` from `&str`.
#[derive(Debug)]
pub struct ParseOpenGLError {
    input: String,
}

impl fmt::Display for ParseOpenGLError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "`{}` is not a valid OpenGL version", self.input)
    }
}

impl Error for ParseOpenGLError {
    fn description(&self) -> &str {
        "Invalid OpenGL version"
    }
}
