//! A Glutin window back-end for the Piston game engine.

// External crates.
use std::{collections::VecDeque, error::Error, thread, time::Duration};

use glutin::{
    context::{PossiblyCurrentContextGlSurfaceAccessor, PossiblyCurrentGlContext},
    display::GlDisplay,
    prelude::GlSurface,
};
use winit::platform::run_return::EventLoopExtRunReturn;

pub use crate::shader_version::OpenGL;
use crate::{
    graphics_api_version::{UnsupportedGraphicsApiError, Version as Api},
    input::{
        keyboard, Button, ButtonArgs, ButtonState, CloseArgs, Event, FileDrag, Input, MouseButton,
        ResizeArgs,
    },
    window::{
        AdvancedWindow, BuildFromWindowSettings, OpenGLWindow, Position, ProcAddress, Size, Window,
        WindowSettings,
    },
};

/// Contains stuff for game window.
pub struct GlutinWindow {
    /// The OpenGL context.
    pub ctx: glutin::context::PossiblyCurrentContext,
    /// The window surface.
    pub surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    /// The graphics display.
    pub display: glutin::display::Display,
    /// The window.
    pub window: winit::window::Window,
    // The back-end does not remember the title.
    title: String,
    exit_on_esc: bool,
    should_close: bool,
    automatic_close: bool,
    // Used to fake capturing of cursor,
    // to get relative mouse events.
    is_capturing_cursor: bool,
    // Stores the last known cursor position.
    last_cursor_pos: Option<[f64; 2]>,
    // Stores relative coordinates to emit on next poll.
    mouse_relative: Option<(f64, f64)>,
    // Used to emit cursor event after enter/leave.
    cursor_pos: Option<[f64; 2]>,
    // Used to filter repeated key presses (does not affect text repeat).
    last_key_pressed: Option<crate::input::Key>,
    // Polls events from window.
    event_loop: winit::event_loop::EventLoop<UserEvent>,
    // Stores list of events ready for processing.
    events: VecDeque<winit::event::Event<'static, UserEvent>>,
}

fn window_builder_from_settings(settings: &WindowSettings) -> winit::window::WindowBuilder {
    let Size { width, height } = settings.get_size();
    let size = winit::dpi::LogicalSize { width, height };
    let mut builder = winit::window::WindowBuilder::new()
        .with_inner_size(size)
        .with_decorations(settings.get_decorated())
        .with_title(settings.get_title())
        .with_resizable(settings.get_resizable())
        .with_transparent(settings.get_transparent());
    if settings.get_fullscreen() {
        let event_loop = winit::event_loop::EventLoop::new();
        let monitor = event_loop.primary_monitor();
        let fullscreen = winit::window::Fullscreen::Borderless(monitor);
        builder = builder.with_fullscreen(Some(fullscreen));
    }
    builder
}

fn graphics_api_from_settings(settings: &WindowSettings) -> Result<Api, Box<dyn Error>> {
    let api = settings
        .get_maybe_graphics_api()
        .unwrap_or(Api::opengl(3, 2));
    if api.api != "OpenGL" {
        return Err(UnsupportedGraphicsApiError {
            found: api.api,
            expected: vec!["OpenGL".into()],
        }
        .into());
    };
    Ok(api)
}

fn surface_attributes_builder_from_settings(
    settings: &WindowSettings,
) -> glutin::surface::SurfaceAttributesBuilder<glutin::surface::WindowSurface> {
    glutin::surface::SurfaceAttributesBuilder::<glutin::surface::WindowSurface>::new()
        .with_srgb(Some(settings.get_srgb()))
}

fn config_template_builder_from_settings(
    settings: &WindowSettings,
) -> glutin::config::ConfigTemplateBuilder {
    let x =
        glutin::config::ConfigTemplateBuilder::new().with_transparency(settings.get_transparent());
    let samples = settings.get_samples();
    if samples == 0 {
        x
    } else {
        x.with_multisampling(samples)
    }
}

impl GlutinWindow {
    /// Creates a new game window for Glutin.
    pub fn new(settings: &WindowSettings) -> Result<Self, Box<dyn Error>> {
        let event_loop = winit::event_loop::EventLoopBuilder::with_user_event().build();
        let window_builder = window_builder_from_settings(settings);
        Self::from_raw(settings, event_loop, window_builder)
    }

    /// Creates a game window from a pre-existing Glutin event loop and window builder.
    pub fn from_raw(
        settings: &WindowSettings,
        event_loop: winit::event_loop::EventLoop<UserEvent>,
        window_builder: winit::window::WindowBuilder,
    ) -> Result<Self, Box<dyn Error>> {
        use std::num::NonZeroU32;

        use glutin::{
            config::GlConfig,
            context::{ContextApi, NotCurrentGlContextSurfaceAccessor},
            display::GetGlDisplay,
        };
        use raw_window_handle::HasRawWindowHandle;

        let title = settings.get_title();
        let exit_on_esc = settings.get_exit_on_esc();

        let template = config_template_builder_from_settings(settings);
        let display_builder =
            glutin_winit::DisplayBuilder::new().with_window_builder(Some(window_builder));
        let (window, gl_config) = display_builder.build(&event_loop, template, |configs| {
            configs
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() > accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .unwrap()
        })?;
        let window = window.unwrap();
        let raw_window_handle = window.raw_window_handle();
        let draw_size = window.inner_size();
        let dw = NonZeroU32::new(draw_size.width).unwrap();
        let dh = NonZeroU32::new(draw_size.height).unwrap();
        let surface_attributes =
            surface_attributes_builder_from_settings(settings).build(raw_window_handle, dw, dh);

        let display: glutin::display::Display = gl_config.display();
        let surface = unsafe { display.create_window_surface(&gl_config, &surface_attributes)? };

        let api = graphics_api_from_settings(settings)?;
        let context_attributes = glutin::context::ContextAttributesBuilder::new()
            .with_context_api(glutin::context::ContextApi::OpenGl(Some(
                glutin::context::Version::new(api.major as u8, api.minor as u8),
            )))
            .build(Some(raw_window_handle));

        let fallback_context_attributes = glutin::context::ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw_window_handle));

        let legacy_context_attributes = glutin::context::ContextAttributesBuilder::new()
            .with_context_api(glutin::context::ContextApi::OpenGl(Some(
                glutin::context::Version::new(2, 1),
            )))
            .build(Some(raw_window_handle));

        let mut not_current_gl_context = Some(unsafe {
            if let Ok(x) = display.create_context(&gl_config, &context_attributes) {
                x
            } else if let Ok(x) = display.create_context(&gl_config, &fallback_context_attributes) {
                x
            } else {
                display.create_context(&gl_config, &legacy_context_attributes)?
            }
        });

        let ctx: glutin::context::PossiblyCurrentContext = not_current_gl_context
            .take()
            .unwrap()
            .make_current(&surface)?;

        if settings.get_vsync() {
            surface.set_swap_interval(
                &ctx,
                glutin::surface::SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
            )?;
        }

        // Load the OpenGL function pointers.
        gl::load_with(|s| {
            use std::ffi::CString;

            let s = CString::new(s).expect("CString::new failed");
            display.get_proc_address(&s) as *const _
        });

        Ok(GlutinWindow {
            ctx,
            display,
            surface,
            window,
            title,
            exit_on_esc,
            should_close: false,
            automatic_close: settings.get_automatic_close(),
            cursor_pos: None,
            is_capturing_cursor: false,
            last_cursor_pos: None,
            mouse_relative: None,
            last_key_pressed: None,
            event_loop,
            events: VecDeque::new(),
        })
    }

    fn wait_event(&mut self) -> Event {
        // First check for and handle any pending events.
        if let Some(event) = self.poll_event() {
            return event;
        }
        loop {
            {
                let events = &mut self.events;
                self.event_loop.run_return(|ev, _, control_flow| {
                    if let Some(event) = to_static_event(ev) {
                        events.push_back(event);
                    }
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                });
            }

            if let Some(event) = self.poll_event() {
                return event;
            }
        }
    }

    fn wait_event_timeout(&mut self, timeout: Duration) -> Option<Event> {
        // First check for and handle any pending events.
        if let Some(event) = self.poll_event() {
            return Some(event);
        }
        // Schedule wake up when time is out.
        let event_loop_proxy = self.event_loop.create_proxy();
        thread::spawn(move || {
            thread::sleep(timeout);
            // `send_event` can fail only if the event loop went away.
            event_loop_proxy.send_event(UserEvent::WakeUp).ok();
        });
        {
            let events = &mut self.events;
            self.event_loop.run_return(|ev, _, control_flow| {
                if let Some(event) = to_static_event(ev) {
                    events.push_back(event);
                }
                *control_flow = winit::event_loop::ControlFlow::Exit;
            });
        }

        self.poll_event()
    }

    fn poll_event(&mut self) -> Option<Event> {
        use winit::event::{Event as E, WindowEvent as WE};

        // Loop to skip unknown events.
        loop {
            let event = self.pre_pop_front_event();
            if event.is_some() {
                return event.map(|x| Event::Input(x, None));
            }

            if self.events.is_empty() {
                self.poll_events();
            }
            let mut ev = self.events.pop_front();

            if self.is_capturing_cursor && self.last_cursor_pos.is_none() {
                if let Some(E::WindowEvent {
                    event: WE::CursorMoved { position, .. },
                    ..
                }) = ev
                {
                    let scale = self.window.scale_factor();
                    let position = position.to_logical::<f64>(scale);
                    // Ignore this event since mouse positions
                    // should not be emitted when capturing cursor.
                    self.last_cursor_pos = Some([position.x, position.y]);

                    if self.events.is_empty() {
                        self.poll_events();
                    }
                    ev = self.events.pop_front();
                }
            }

            let mut unknown = false;
            let event = self.handle_event(ev, &mut unknown);
            if unknown {
                continue;
            };
            return event.map(|x| Event::Input(x, None));
        }
    }

    fn poll_events(&mut self) {
        // Ensure there's at least one event in the queue.
        let event_loop_proxy = self.event_loop.create_proxy();
        event_loop_proxy.send_event(UserEvent::WakeUp).ok();

        // Poll events currently in the queue, stopping when the queue is empty.
        let events = &mut self.events;
        self.event_loop.run_return(|ev, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Wait;
            if let Some(event) = to_static_event(ev) {
                if event == winit::event::Event::UserEvent(UserEvent::WakeUp) {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                events.push_back(event);
            }
        });
    }

    // These events are emitted before popping a new event from the queue.
    // This is because Piston handles some events separately.
    fn pre_pop_front_event(&mut self) -> Option<Input> {
        use crate::input::Motion;

        // Check for a pending mouse cursor move event.
        if let Some(pos) = self.cursor_pos {
            self.cursor_pos = None;
            return Some(Input::Move(Motion::MouseCursor(pos)));
        }

        // Check for a pending relative mouse move event.
        if let Some((x, y)) = self.mouse_relative {
            self.mouse_relative = None;
            return Some(Input::Move(Motion::MouseRelative([x, y])));
        }

        None
    }

    /// Convert an incoming Glutin event to Piston input.
    /// Update cursor state if necessary.
    ///
    /// The `unknown` flag is set to `true` when the event is not recognized.
    /// This is used to poll another event to make the event loop logic sound.
    /// When `unknown` is `true`, the return value is `None`.
    fn handle_event(
        &mut self,
        ev: Option<winit::event::Event<UserEvent>>,
        unknown: &mut bool,
    ) -> Option<Input> {
        use winit::event::{Event as E, MouseScrollDelta, WindowEvent as WE};

        use crate::input::{Key, Motion};

        match ev {
            None => {
                if self.is_capturing_cursor {
                    self.fake_capture();
                }
                None
            }
            Some(E::WindowEvent {
                event: WE::Resized(draw_size),
                ..
            }) => {
                use std::num::NonZeroU32;

                let size = self.size();

                // Some platforms (MacOS and Wayland) require the context to resize on window
                // resize. Check: https://github.com/PistonDevelopers/graphics/issues/1129
                let dw = NonZeroU32::new(draw_size.width)?;
                let dh = NonZeroU32::new(draw_size.height)?;
                self.surface.resize(&self.ctx, dw, dh);

                Some(Input::Resize(ResizeArgs {
                    window_size: [size.width, size.height],
                    draw_size: draw_size.into(),
                }))
            }
            Some(E::WindowEvent {
                event: WE::ReceivedCharacter(ch),
                ..
            }) => {
                let string = match ch {
                    // Ignore control characters and return ascii for Text event (like sdl2).
                    '\u{7f}' | // Delete
                    '\u{1b}' | // Escape
                    '\u{8}'  | // Backspace
                    '\r' | '\n' | '\t' => "".to_string(),
                    _ => ch.to_string()
                };
                Some(Input::Text(string))
            }
            Some(E::WindowEvent {
                event: WE::Focused(focused),
                ..
            }) => Some(Input::Focus(focused)),
            Some(E::WindowEvent {
                event:
                    WE::KeyboardInput {
                        input:
                            winit::event::KeyboardInput {
                                state: winit::event::ElementState::Pressed,
                                virtual_keycode: Some(key),
                                scancode,
                                ..
                            },
                        ..
                    },
                ..
            }) => {
                let piston_key = map_key(key);
                if let (true, Key::Escape) = (self.exit_on_esc, piston_key) {
                    self.should_close = true;
                }
                if let Some(last_key) = self.last_key_pressed {
                    if last_key == piston_key {
                        *unknown = true;
                        return None;
                    }
                }
                self.last_key_pressed = Some(piston_key);
                Some(Input::Button(ButtonArgs {
                    state: ButtonState::Press,
                    button: Button::Keyboard(piston_key),
                    scancode: Some(scancode as i32),
                }))
            }
            Some(E::WindowEvent {
                event:
                    WE::KeyboardInput {
                        input:
                            winit::event::KeyboardInput {
                                state: winit::event::ElementState::Released,
                                virtual_keycode: Some(key),
                                scancode,
                                ..
                            },
                        ..
                    },
                ..
            }) => {
                let piston_key = map_key(key);
                if let Some(last_key) = self.last_key_pressed {
                    if last_key == piston_key {
                        self.last_key_pressed = None;
                    }
                }
                Some(Input::Button(ButtonArgs {
                    state: ButtonState::Release,
                    button: Button::Keyboard(piston_key),
                    scancode: Some(scancode as i32),
                }))
            }
            Some(E::WindowEvent {
                event:
                    WE::Touch(winit::event::Touch {
                        phase,
                        location,
                        id,
                        ..
                    }),
                ..
            }) => {
                use winit::event::TouchPhase;

                use crate::input::{Touch, TouchArgs};

                let scale = self.window.scale_factor();
                let location = location.to_logical::<f64>(scale);

                Some(Input::Move(Motion::Touch(TouchArgs::new(
                    0,
                    id as i64,
                    [location.x, location.y],
                    1.0,
                    match phase {
                        TouchPhase::Started => Touch::Start,
                        TouchPhase::Moved => Touch::Move,
                        TouchPhase::Ended => Touch::End,
                        TouchPhase::Cancelled => Touch::Cancel,
                    },
                ))))
            }
            Some(E::WindowEvent {
                event: WE::CursorMoved { position, .. },
                ..
            }) => {
                let scale = self.window.scale_factor();
                let position = position.to_logical::<f64>(scale);
                let x = position.x;
                let y = position.y;

                if let Some(pos) = self.last_cursor_pos {
                    let dx = x - pos[0];
                    let dy = y - pos[1];
                    if self.is_capturing_cursor {
                        self.last_cursor_pos = Some([x, y]);
                        self.fake_capture();
                        // Skip normal mouse movement and emit relative motion only.
                        return Some(Input::Move(Motion::MouseRelative([dx, dy])));
                    }
                    // Send relative mouse movement next time.
                    self.mouse_relative = Some((dx, dy));
                }

                self.last_cursor_pos = Some([x, y]);
                Some(Input::Move(Motion::MouseCursor([x, y])))
            }
            Some(E::WindowEvent {
                event: WE::CursorEntered { .. },
                ..
            }) => Some(Input::Cursor(true)),
            Some(E::WindowEvent {
                event: WE::CursorLeft { .. },
                ..
            }) => Some(Input::Cursor(false)),
            Some(E::WindowEvent {
                event:
                    WE::MouseWheel {
                        delta: MouseScrollDelta::PixelDelta(pos),
                        ..
                    },
                ..
            }) => {
                let scale = self.window.scale_factor();
                let pos = pos.to_logical::<f64>(scale);
                Some(Input::Move(Motion::MouseScroll([pos.x, pos.y])))
            }
            Some(E::WindowEvent {
                event:
                    WE::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(x, y),
                        ..
                    },
                ..
            }) => Some(Input::Move(Motion::MouseScroll([x as f64, y as f64]))),
            Some(E::WindowEvent {
                event:
                    WE::MouseInput {
                        state: winit::event::ElementState::Pressed,
                        button,
                        ..
                    },
                ..
            }) => Some(Input::Button(ButtonArgs {
                state: ButtonState::Press,
                button: Button::Mouse(map_mouse(button)),
                scancode: None,
            })),
            Some(E::WindowEvent {
                event:
                    WE::MouseInput {
                        state: winit::event::ElementState::Released,
                        button,
                        ..
                    },
                ..
            }) => Some(Input::Button(ButtonArgs {
                state: ButtonState::Release,
                button: Button::Mouse(map_mouse(button)),
                scancode: None,
            })),
            Some(E::WindowEvent {
                event: WE::HoveredFile(path),
                ..
            }) => Some(Input::FileDrag(FileDrag::Hover(path))),
            Some(E::WindowEvent {
                event: WE::DroppedFile(path),
                ..
            }) => Some(Input::FileDrag(FileDrag::Drop(path))),
            Some(E::WindowEvent {
                event: WE::HoveredFileCancelled,
                ..
            }) => Some(Input::FileDrag(FileDrag::Cancel)),
            Some(E::WindowEvent {
                event: WE::CloseRequested,
                ..
            }) => {
                if self.automatic_close {
                    self.should_close = true;
                }
                Some(Input::Close(CloseArgs))
            }
            Some(E::UserEvent(UserEvent::WakeUp)) => None,
            _ => {
                *unknown = true;
                None
            }
        }
    }

    fn fake_capture(&mut self) {
        if let Some(pos) = self.last_cursor_pos {
            // Fake capturing of cursor.
            let size = self.size();
            let cx = size.width / 2.0;
            let cy = size.height / 2.0;
            let dx = cx - pos[0];
            let dy = cy - pos[1];
            if dx != 0.0 || dy != 0.0 {
                let pos = winit::dpi::LogicalPosition::new(cx, cy);
                if self.window.set_cursor_position(pos).is_ok() {
                    self.last_cursor_pos = Some([cx, cy]);
                }
            }
        }
    }
}

impl Window for GlutinWindow {
    fn size(&self) -> Size {
        let size = self
            .window
            .inner_size()
            .to_logical::<u32>(self.window.scale_factor());
        (size.width, size.height).into()
    }
    fn draw_size(&self) -> Size {
        let size = self.window.inner_size();
        (size.width, size.height).into()
    }
    fn should_close(&self) -> bool {
        self.should_close
    }
    fn set_should_close(&mut self, value: bool) {
        self.should_close = value;
    }
    fn swap_buffers(&mut self) {
        let _ = self.surface.swap_buffers(&self.ctx);
    }
    fn wait_event(&mut self) -> Event {
        self.wait_event()
    }
    fn wait_event_timeout(&mut self, timeout: Duration) -> Option<Event> {
        self.wait_event_timeout(timeout)
    }
    fn poll_event(&mut self) -> Option<Event> {
        self.poll_event()
    }
}

impl BuildFromWindowSettings for GlutinWindow {
    fn build_from_window_settings(settings: &WindowSettings) -> Result<Self, Box<dyn Error>> {
        GlutinWindow::new(settings)
    }
}

impl AdvancedWindow for GlutinWindow {
    fn get_title(&self) -> String {
        self.title.clone()
    }
    fn set_title(&mut self, value: String) {
        self.title = value;
        self.window.set_title(&self.title);
    }
    fn get_exit_on_esc(&self) -> bool {
        self.exit_on_esc
    }
    fn set_exit_on_esc(&mut self, value: bool) {
        self.exit_on_esc = value;
    }
    fn get_automatic_close(&self) -> bool {
        self.automatic_close
    }
    fn set_automatic_close(&mut self, value: bool) {
        self.automatic_close = value;
    }
    fn set_capture_cursor(&mut self, value: bool) {
        // Normally we would call `.grab_cursor(true)`
        // but since relative mouse events does not work,
        // the capturing of cursor is faked by hiding the cursor
        // and setting the position to the center of window.
        self.is_capturing_cursor = value;
        self.window.set_cursor_visible(!value);
        if value {
            self.fake_capture();
        }
    }
    fn show(&mut self) {
        self.window.set_visible(true);
    }
    fn hide(&mut self) {
        self.window.set_visible(false);
    }
    fn get_position(&self) -> Option<Position> {
        let pos = self.window.outer_position().ok()?;
        let scale = self.window.scale_factor();
        let winit::dpi::LogicalPosition { x, y } = pos.to_logical(scale);
        Some(Position { x, y })
    }
    fn set_position<P: Into<Position>>(&mut self, pos: P) {
        let Position { x, y } = pos.into();
        let pos = winit::dpi::LogicalPosition { x, y };
        self.window.set_outer_position(pos);
    }
    fn set_size<S: Into<Size>>(&mut self, size: S) {
        let Size { width, height } = size.into();
        let size = winit::dpi::LogicalSize { width, height };
        self.window.set_inner_size(size);
    }
}

impl OpenGLWindow for GlutinWindow {
    fn get_proc_address(&mut self, proc_name: &str) -> ProcAddress {
        use std::ffi::CString;

        let s = CString::new(proc_name).expect("CString::new failed");
        self.display.get_proc_address(&s) as *const _
    }

    fn is_current(&self) -> bool {
        self.ctx.is_current()
    }

    fn make_current(&mut self) {
        let _ = self.ctx.make_current(&self.surface);
    }
}

/// Maps Glutin's key to Piston's key.
pub fn map_key(keycode: winit::event::VirtualKeyCode) -> keyboard::Key {
    use winit::event::VirtualKeyCode as K;

    use crate::input::keyboard::Key;

    match keycode {
        K::Key0 => Key::D0,
        K::Key1 => Key::D1,
        K::Key2 => Key::D2,
        K::Key3 => Key::D3,
        K::Key4 => Key::D4,
        K::Key5 => Key::D5,
        K::Key6 => Key::D6,
        K::Key7 => Key::D7,
        K::Key8 => Key::D8,
        K::Key9 => Key::D9,
        K::A => Key::A,
        K::B => Key::B,
        K::C => Key::C,
        K::D => Key::D,
        K::E => Key::E,
        K::F => Key::F,
        K::G => Key::G,
        K::H => Key::H,
        K::I => Key::I,
        K::J => Key::J,
        K::K => Key::K,
        K::L => Key::L,
        K::M => Key::M,
        K::N => Key::N,
        K::O => Key::O,
        K::P => Key::P,
        K::Q => Key::Q,
        K::R => Key::R,
        K::S => Key::S,
        K::T => Key::T,
        K::U => Key::U,
        K::V => Key::V,
        K::W => Key::W,
        K::X => Key::X,
        K::Y => Key::Y,
        K::Z => Key::Z,
        K::Apostrophe => Key::Unknown,
        K::Backslash => Key::Backslash,
        K::Back => Key::Backspace,
        // K::CapsLock => Key::CapsLock,
        K::Delete => Key::Delete,
        K::Comma => Key::Comma,
        K::Down => Key::Down,
        K::End => Key::End,
        K::Return => Key::Return,
        K::Equals => Key::Equals,
        K::Escape => Key::Escape,
        K::F1 => Key::F1,
        K::F2 => Key::F2,
        K::F3 => Key::F3,
        K::F4 => Key::F4,
        K::F5 => Key::F5,
        K::F6 => Key::F6,
        K::F7 => Key::F7,
        K::F8 => Key::F8,
        K::F9 => Key::F9,
        K::F10 => Key::F10,
        K::F11 => Key::F11,
        K::F12 => Key::F12,
        K::F13 => Key::F13,
        K::F14 => Key::F14,
        K::F15 => Key::F15,
        K::F16 => Key::F16,
        K::F17 => Key::F17,
        K::F18 => Key::F18,
        K::F19 => Key::F19,
        K::F20 => Key::F20,
        K::F21 => Key::F21,
        K::F22 => Key::F22,
        K::F23 => Key::F23,
        K::F24 => Key::F24,
        // Possibly next code.
        // K::F25 => Key::Unknown,
        K::Numpad0 => Key::NumPad0,
        K::Numpad1 => Key::NumPad1,
        K::Numpad2 => Key::NumPad2,
        K::Numpad3 => Key::NumPad3,
        K::Numpad4 => Key::NumPad4,
        K::Numpad5 => Key::NumPad5,
        K::Numpad6 => Key::NumPad6,
        K::Numpad7 => Key::NumPad7,
        K::Numpad8 => Key::NumPad8,
        K::Numpad9 => Key::NumPad9,
        K::NumpadComma => Key::NumPadDecimal,
        K::NumpadDivide => Key::NumPadDivide,
        K::NumpadMultiply => Key::NumPadMultiply,
        K::NumpadSubtract => Key::NumPadMinus,
        K::NumpadAdd => Key::NumPadPlus,
        K::NumpadEnter => Key::NumPadEnter,
        K::NumpadEquals => Key::NumPadEquals,
        K::LShift => Key::LShift,
        K::LControl => Key::LCtrl,
        K::LAlt => Key::LAlt,
        K::RShift => Key::RShift,
        K::RControl => Key::RCtrl,
        K::RAlt => Key::RAlt,
        // Map to backslash?
        // K::GraveAccent => Key::Unknown,
        K::Home => Key::Home,
        K::Insert => Key::Insert,
        K::Left => Key::Left,
        K::LBracket => Key::LeftBracket,
        // K::Menu => Key::Menu,
        K::Minus => Key::Minus,
        K::Numlock => Key::NumLockClear,
        K::PageDown => Key::PageDown,
        K::PageUp => Key::PageUp,
        K::Pause => Key::Pause,
        K::Period => Key::Period,
        K::Snapshot => Key::PrintScreen,
        K::Right => Key::Right,
        K::RBracket => Key::RightBracket,
        K::Scroll => Key::ScrollLock,
        K::Semicolon => Key::Semicolon,
        K::Slash => Key::Slash,
        K::Space => Key::Space,
        K::Tab => Key::Tab,
        K::Up => Key::Up,
        // K::World1 => Key::Unknown,
        // K::World2 => Key::Unknown,
        _ => Key::Unknown,
    }
}

/// Maps Glutin's mouse button to Piston's mouse button.
pub fn map_mouse(mouse_button: winit::event::MouseButton) -> MouseButton {
    use winit::event::MouseButton as M;

    match mouse_button {
        M::Left => MouseButton::Left,
        M::Right => MouseButton::Right,
        M::Middle => MouseButton::Middle,
        M::Other(0) => MouseButton::X1,
        M::Other(1) => MouseButton::X2,
        M::Other(2) => MouseButton::Button6,
        M::Other(3) => MouseButton::Button7,
        M::Other(4) => MouseButton::Button8,
        _ => MouseButton::Unknown,
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
/// Custom events for the glutin event loop
pub enum UserEvent {
    /// Do nothing, just spin the event loop
    WakeUp,
}

// XXX Massive Hack XXX: `wait_event` and `wait_event_timeout` can't handle non-'static events, so
// they need to ignore events like `WindowEvent::ScaleFactorChanged` that contain references.
fn to_static_event(
    event: winit::event::Event<UserEvent>,
) -> Option<winit::event::Event<'static, UserEvent>> {
    use winit::event::{Event as E, WindowEvent as WE};
    let event = match event {
        E::NewEvents(s) => E::NewEvents(s),
        E::WindowEvent { window_id, event } => E::WindowEvent {
            window_id,
            event: match event {
                WE::Resized(size) => WE::Resized(size),
                WE::Moved(pos) => WE::Moved(pos),
                WE::CloseRequested => WE::CloseRequested,
                WE::Destroyed => WE::Destroyed,
                WE::DroppedFile(path) => WE::DroppedFile(path),
                WE::HoveredFile(path) => WE::HoveredFile(path),
                WE::HoveredFileCancelled => WE::HoveredFileCancelled,
                WE::ReceivedCharacter(c) => WE::ReceivedCharacter(c),
                WE::Focused(b) => WE::Focused(b),
                WE::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                } => WE::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                },
                WE::ModifiersChanged(_) => return None, // XXX?
                #[allow(deprecated)]
                WE::CursorMoved {
                    device_id,
                    position,
                    modifiers,
                } => WE::CursorMoved {
                    device_id,
                    position,
                    modifiers,
                },
                WE::CursorEntered { device_id } => WE::CursorEntered { device_id },
                WE::CursorLeft { device_id } => WE::CursorLeft { device_id },
                #[allow(deprecated)]
                WE::MouseWheel {
                    device_id,
                    delta,
                    phase,
                    modifiers,
                } => WE::MouseWheel {
                    device_id,
                    delta,
                    phase,
                    modifiers,
                },
                #[allow(deprecated)]
                WE::MouseInput {
                    device_id,
                    state,
                    button,
                    modifiers,
                } => WE::MouseInput {
                    device_id,
                    state,
                    button,
                    modifiers,
                },
                WE::TouchpadPressure {
                    device_id,
                    pressure,
                    stage,
                } => WE::TouchpadPressure {
                    device_id,
                    pressure,
                    stage,
                },
                WE::AxisMotion {
                    device_id,
                    axis,
                    value,
                } => WE::AxisMotion {
                    device_id,
                    axis,
                    value,
                },
                WE::Touch(touch) => WE::Touch(touch),
                WE::ScaleFactorChanged { .. } => return None,
                WE::ThemeChanged(theme) => WE::ThemeChanged(theme),
                WE::Ime(_) => return None,
                WE::TouchpadMagnify { .. } => return None,
                WE::TouchpadRotate { .. } => return None,
                WE::Occluded(_) => return None,
                WE::SmartMagnify { .. } => return None,
            },
        },
        E::DeviceEvent { device_id, event } => E::DeviceEvent { device_id, event },
        E::UserEvent(e) => E::UserEvent(e),
        E::Suspended => E::Suspended,
        E::Resumed => E::Resumed,
        E::MainEventsCleared => E::MainEventsCleared,
        E::RedrawRequested(window_id) => E::RedrawRequested(window_id),
        E::RedrawEventsCleared => E::RedrawEventsCleared,
        E::LoopDestroyed => E::LoopDestroyed,
    };
    Some(event)
}
