//! A library for 2D graphics that works with multiple back-ends.
//!
//! Piston-Graphics was started in 2014 by Sven Nilsen to test
//! back-end agnostic design for 2D in Rust.
//! This means generic code can be reused across projects and platforms.
//!
//! ### Design
//!
//! A graphics back-end implements the `Graphics` trait.
//!
//! This library uses immediate design for flexibility.
//! By default, triangles are generated from 2D shapes and passed in chunks
//! to the back-end. This behavior can be overridden by a back-end library.
//!
//! The structures used for drawing 2D shapes contains settings for rendering.
//! The separation of shapes and settings allows more reuse and flexibility.
//! For example, to render an image, you use an `Image` object.
//!
//! The `math` module contains useful methods for 2D geometry.
//!
//! `Context` stores settings that are commonly shared when rendering.
//! It can be copied and changed without affecting any global state.
//!
//! At top level, there are some shortcut methods for common operations.
//! For example, `ellipse` is a simplified version of `Ellipse`.

pub use character::{Character, CharacterCache};
pub use circle_arc::CircleArc;
pub use colored::Colored;
pub use context::Context;
pub use draw_state::DrawState;
pub use ellipse::Ellipse;
pub use image::Image;
pub use line::Line;
use math::{Affine2, Scalar};
pub use polygon::Polygon;
pub use radians::Radians;
pub use rectangle::Rectangle;
pub use rectangled::Rectangled;
pub use source_rectangled::SourceRectangled;
pub use text::Text;
pub use transformed::Transformed;

pub use crate::{texture::ImageSize, viewport::Viewport};

/// Any triangulation method called on the back-end
/// never exceeds this number of vertices.
/// This can be used to initialize buffers that fit the chunk size.
///
/// Must be a multiple of 3 because you need 3 vertices per triangle
/// in a triangle list.
pub const BACK_END_MAX_VERTEX_COUNT: usize = 1023;

mod colored;
mod rectangled;
mod source_rectangled;
mod transformed;

pub mod character;
pub mod circle_arc;
pub mod color;
pub mod context;
pub mod draw_state;
pub mod ellipse;
pub mod grid;
pub mod image;
pub mod line;
pub mod math;
pub mod modular_index;
pub mod polygon;
pub mod rectangle;
pub mod text;
pub mod texture_packer;
pub mod triangulation;
pub mod types;

pub mod radians {
    //! Reexport radians helper trait from vecmath

    pub use vecmath::traits::Radians;
}

/// Clears the screen.
pub fn clear<G>(color: types::Color, g: &mut G)
where
    G: Graphics,
{
    g.clear_color(color);
    g.clear_stencil(0);
}

/// Draws image.
pub fn image<G>(image: &<G as Graphics>::Texture, transform: math::Affine2, g: &mut G)
where
    G: Graphics,
{
    Image::new().draw(image, &Default::default(), transform, g);
}

/// Draws ellipse by corners.
pub fn ellipse_from_to<P: Into<types::Vec2d>, G>(
    color: types::Color,
    from: P,
    to: P,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    Ellipse::new(color).draw_from_to(from, to, &Default::default(), transform, g);
}

/// Draws ellipse.
pub fn ellipse<R: Into<types::Rectangle>, G>(
    color: types::Color,
    rect: R,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    Ellipse::new(color).draw(rect, &Default::default(), transform, g);
}

/// Draws arc
pub fn circle_arc<R: Into<types::Rectangle>, G>(
    color: types::Color,
    radius: types::Radius,
    start: types::Scalar,
    end: types::Scalar,
    rect: R,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    CircleArc::new(color, radius, start, end).draw(rect, &Default::default(), transform, g);
}

/// Draws rectangle.
pub fn rectangle_from_to<P: Into<types::Vec2d>, G>(
    color: types::Color,
    from: P,
    to: P,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    Rectangle::new(color).draw_from_to(from, to, &Default::default(), transform, g);
}

/// Draws rectangle.
pub fn rectangle<R: Into<types::Rectangle>, G>(
    color: types::Color,
    rect: R,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    Rectangle::new(color).draw(rect, &Default::default(), transform, g);
}

/// Draws polygon.
pub fn polygon<G>(
    color: types::Color,
    polygon: types::Polygon<'_>,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    Polygon::new(color).draw(polygon, &Default::default(), transform, g);
}

/// Draws line between points.
pub fn line_from_to<P: Into<types::Vec2d>, G>(
    color: types::Color,
    radius: types::Radius,
    from: P,
    to: P,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    Line::new(color, radius).draw_from_to(from, to, &Default::default(), transform, g)
}

/// Draws line.
pub fn line<L: Into<types::Line>, G>(
    color: types::Color,
    radius: types::Radius,
    line: L,
    transform: math::Affine2,
    g: &mut G,
) where
    G: Graphics,
{
    Line::new(color, radius).draw(line, &Default::default(), transform, g)
}

/// Draws text.
pub fn text<C, G>(
    color: types::Color,
    font_size: types::FontSize,
    text: &str,
    cache: &mut C,
    transform: math::Affine2,
    g: &mut G,
) -> Result<(), C::Error>
where
    C: character::CharacterCache,
    G: Graphics<Texture = <C as character::CharacterCache>::Texture>,
{
    Text::new_color(color, font_size).draw(text, cache, &Default::default(), transform, g)
}
/// Implemented by all graphics back-ends.
///
/// [An example back-end using raw OpenGL](https://github.com/PistonDevelopers/opengl_graphics)
///
/// By default, this design uses triangles as graphics primitives.
/// This is supported by all GPUs and easy to implement in shader languages.
///
/// Default trait methods can be overridden for better performance or higher
/// quality.
///
/// When drawing, use this trait as generic constraint:
///
/// ```
/// use graphics::{Context, Graphics};
///
/// fn draw<G: Graphics>(c: &Context, g: &mut G) {
///     //...
/// }
/// ```
///
/// Color space is sRGB.
///
/// ### Notice for back-end authors
///
/// When sRGB is enabled for a back-end shader, the gamma must be converted
/// to linear space when used as vertex color or uniform parameter.
/// To convert gamma, use `color::gamma_srgb_to_linear`.
///
/// For more information, see
/// <https://github.com/PistonDevelopers/piston/issues/1014>.
pub trait Graphics: Sized {
    /// The texture type associated with the back-end.
    ///
    /// In generic code, this type is often unknown.
    /// This might lead to more boilerplate code:
    ///
    /// ```
    /// use graphics::{Context, Graphics, ImageSize};
    ///
    /// fn draw_texture<G, T>(c: &Context, g: &mut G)
    /// where
    ///     G: Graphics<Texture = T>,
    ///     T: ImageSize,
    /// {
    ///     //...
    /// }
    /// ```
    ///
    /// Code written specifically for one back-end can be easier to write.
    /// Later, when the code is done, it can be refactored into generic code.
    type Texture: ImageSize;

    /// Clears background with a color.
    ///
    /// The color should replace the values in the buffer.
    ///
    /// Color space is sRGB.
    fn clear_color(&mut self, color: types::Color);

    /// Clears stencil buffer with a value, usually 0.
    ///
    /// A stencil buffer contains values that are not visible on the screen.
    /// These values are used to test against the pixel to paint.
    ///
    /// If you are drawing a shape for clipping and forgot to clear the
    /// stencil buffer, then the clipping shape will carry over in next frame
    /// and cause artifacts.
    fn clear_stencil(&mut self, value: u8);

    /// Renders list of 2d triangles using a solid color.
    ///
    /// All vertices share the same color.
    ///
    /// The back-end calls the closure with a closure to receive vertices.
    /// First, the back-end sets up shaders and such to prepare.
    /// Then it calls the closure, which calls back with chunks of vertices.
    /// The number of vertices per chunk never exceeds
    /// `BACK_END_MAX_VERTEX_COUNT`.
    /// Vertex positions are encoded `[[x0, y0], [x1, y1], ...]`.
    ///
    /// Color space is sRGB.
    fn tri_list<F>(&mut self, draw_state: &DrawState, color: &[f32; 4], f: F)
    where
        F: FnMut(&mut dyn FnMut(&[[f32; 2]]));

    /// Same as `tri_list`, but with individual vertex colors.
    ///
    /// Argument are `|vertices: &[[f32; 2], colors: &[[f32; 4]]]|`.
    fn tri_list_c<F>(&mut self, draw_state: &DrawState, f: F)
    where
        F: FnMut(&mut dyn FnMut(&[[f32; 2]], &[[f32; 4]]));

    /// Renders list of 2d triangles using a color and a texture.
    ///
    /// All vertices share the same color.
    ///
    /// Tip: For objects of different colors, use grayscale textures.
    /// The texture color gets multiplied with the color.
    ///
    /// A texture coordinate is assigned per vertex (from [0, 0] to [1, 1]).
    ///
    /// The back-end calls the closure with a closure to receive vertices.
    /// First, the back-end sets up shaders and such to prepare.
    /// Then it calls the closure, which calls back with chunks of vertices.
    /// The number of vertices per chunk never exceeds
    /// `BACK_END_MAX_VERTEX_COUNT`.
    /// Vertex positions are encoded `[[x0, y0], [x1, y1], ...]`.
    /// Texture coordinates are encoded `[[u0, v0], [u1, v1], ...]`.
    ///
    /// Chunks uses separate buffer for vertex positions and texture coordinates.
    /// Arguments are `|vertices: &[[f32; 2]], texture_coords: &[[f32; 2]]|`.
    ///
    /// Color space is sRGB.
    fn tri_list_uv<F>(
        &mut self,
        draw_state: &DrawState,
        color: &[f32; 4],
        texture: &<Self as Graphics>::Texture,
        f: F,
    ) where
        F: FnMut(&mut dyn FnMut(&[[f32; 2]], &[[f32; 2]]));

    /// Same as `tri_list_uv`, but with individual vertex colors.
    ///
    /// Argument are `|vertices: &[[f32; 2], texture_coors: &[[f32; 2]], colors: &[[f32; 4]]]|`.
    fn tri_list_uv_c<F>(
        &mut self,
        draw_state: &DrawState,
        texture: &<Self as Graphics>::Texture,
        f: F,
    ) where
        F: FnMut(&mut dyn FnMut(&[[f32; 2]], &[[f32; 2]], &[[f32; 4]]));

    /// Draws a rectangle.
    ///
    /// Can be overriden in the back-end for higher performance.
    ///
    /// Instead of calling this directly, use `Rectangle::draw`.
    #[inline(always)]
    fn rectangle<R: Into<types::Rectangle>>(
        &mut self,
        r: &Rectangle,
        rectangle: R,
        draw_state: &DrawState,
        transform: Affine2,
    ) {
        r.draw_tri(rectangle, draw_state, transform, self);
    }

    /// Draws a polygon.
    ///
    /// Can be overridden in the back-end for higher performance.
    ///
    /// Instead of calling this directly, use `Polygon::draw`.
    #[inline(always)]
    fn polygon(
        &mut self,
        p: &Polygon,
        polygon: types::Polygon<'_>,
        draw_state: &DrawState,
        transform: Affine2,
    ) {
        p.draw_tri(polygon, draw_state, transform, self);
    }

    /// Draws a tweened polygon using linear interpolation.
    ///
    /// Can be overridden in the back-end for higher performance.
    ///
    /// Instead of calling this directly, use `Polygon::draw_tween_lerp`.
    #[inline(always)]
    fn polygon_tween_lerp(
        &mut self,
        p: &Polygon,
        polygons: types::Polygons<'_>,
        tween_factor: Scalar,
        draw_state: &DrawState,
        transform: Affine2,
    ) {
        p.draw_tween_lerp_tri(polygons, tween_factor, draw_state, transform, self);
    }

    /// Draws image.
    ///
    /// Can be overridden in the back-end for higher performance.
    ///
    /// Instead of calling this directly, use `Image::draw`.
    #[inline(always)]
    fn image(
        &mut self,
        image: &Image,
        texture: &Self::Texture,
        draw_state: &DrawState,
        transform: Affine2,
    ) {
        image.draw_tri(texture, draw_state, transform, self);
    }

    /// Draws ellipse.
    ///
    /// Can be overridden in the back-end for higher performance.
    ///
    /// Instead of calling this directly, use `Ellipse::draw`.
    #[inline(always)]
    fn ellipse<R: Into<types::Rectangle>>(
        &mut self,
        e: &Ellipse,
        rectangle: R,
        draw_state: &DrawState,
        transform: Affine2,
    ) {
        e.draw_tri(rectangle, draw_state, transform, self);
    }

    /// Draws line.
    ///
    /// Can be overridden in the back-end for higher performance.
    ///
    /// Instead of calling this directly, use `Line::draw`.
    #[inline(always)]
    fn line<L: Into<types::Line>>(
        &mut self,
        l: &Line,
        line: L,
        draw_state: &DrawState,
        transform: Affine2,
    ) {
        l.draw_tri(line, draw_state, transform, self);
    }

    /// Draws circle arc.
    ///
    /// Can be overriden in the back-end for higher performance.
    ///
    /// Instead of calling this directly, use `CircleArc::draw`.
    #[inline(always)]
    fn circle_arc<R: Into<types::Rectangle>>(
        &mut self,
        c: &CircleArc,
        rectangle: R,
        draw_state: &DrawState,
        transform: Affine2,
    ) {
        c.draw_tri(rectangle, draw_state, transform, self);
    }
}
