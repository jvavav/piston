//! An OpenGL back-end for Rust-Graphics

pub mod error;
pub mod shader_uniforms;
pub mod shader_utils;

mod back_end;
mod draw_state;
mod texture;

pub use self::{
    back_end::{Colored, GlGraphics, Textured},
    texture::Texture,
};
