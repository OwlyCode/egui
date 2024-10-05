mod builder;
#[doc = include_str!("../README.md")]
mod event;
#[cfg(feature = "snapshot")]
mod snapshot;
#[cfg(feature = "snapshot")]
pub use snapshot::*;
#[cfg(feature = "wgpu")]
mod texture_to_bytes;
#[cfg(feature = "wgpu")]
pub mod wgpu;

pub use kittest;
use std::mem;

use crate::event::{kittest_key_to_egui, pointer_button_to_egui};
pub use accesskit_consumer;
pub use builder::*;
use egui::{Event, Modifiers, Pos2, Rect, TexturesDelta, Vec2, ViewportId};
use kittest::{ElementState, Node, Queryable, SimulatedEvent};

/// The test Harness. This contains everything needed to run the test.
/// Create a new Harness using [`Harness::new`] or [`Harness::builder`].
pub struct Harness<'a> {
    pub ctx: egui::Context,
    input: egui::RawInput,
    kittest: kittest::State,
    output: egui::FullOutput,
    texture_deltas: Vec<TexturesDelta>,
    update_fn: Box<dyn FnMut(&egui::Context) + 'a>,

    last_mouse_pos: Pos2,
    modifiers: Modifiers,
}

impl<'a> Harness<'a> {
    pub(crate) fn from_builder(
        builder: &HarnessBuilder,
        mut app: impl FnMut(&egui::Context) + 'a,
    ) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut input = egui::RawInput {
            screen_rect: Some(builder.screen_rect),
            ..Default::default()
        };
        let viewport = input.viewports.get_mut(&ViewportId::ROOT).unwrap();
        viewport.native_pixels_per_point = Some(builder.dpi);

        let mut output = ctx.run(input.clone(), &mut app);

        Self {
            update_fn: Box::new(app),
            ctx,
            input,
            kittest: kittest::State::new(
                output
                    .platform_output
                    .accesskit_update
                    .take()
                    .expect("AccessKit was disabled"),
            ),
            texture_deltas: vec![mem::take(&mut output.textures_delta)],
            output,

            last_mouse_pos: Pos2::ZERO,
            modifiers: Modifiers::NONE,
        }
    }

    pub fn builder() -> HarnessBuilder {
        HarnessBuilder::default()
    }

    /// Create a new Harness with the given app closure.
    ///
    /// The ui closure will immediately be called once to create the initial ui.
    ///
    /// If you e.g. want to customize the size of the window, you can use [`Harness::builder`].
    ///
    /// # Example
    /// ```rust
    /// # use egui::CentralPanel;
    /// # use egui_kittest::Harness;
    /// let mut harness = Harness::new(|ctx| {
    ///     CentralPanel::default().show(ctx, |ui| {
    ///         ui.label("Hello, world!");
    ///     });
    /// });
    /// ```
    pub fn new(app: impl FnMut(&egui::Context) + 'a) -> Self {
        Self::builder().build(app)
    }

    /// Set the size of the window.
    /// Note: If you only want to set the size once at the beginning,
    /// prefer using [`HarnessBuilder::with_size`].
    #[inline]
    pub fn set_size(&mut self, size: Vec2) -> &mut Self {
        self.input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, size));
        self
    }

    /// Set the DPI of the window.
    /// Note: If you only want to set the DPI once at the beginning,
    /// prefer using [`HarnessBuilder::with_dpi`].
    #[inline]
    pub fn set_dpi(&mut self, dpi: f32) -> &mut Self {
        self.ctx.set_pixels_per_point(dpi);
        self
    }

    /// Run a frame.
    /// This will call the app closure with the current context and update the Harness.
    pub fn run(&mut self) {
        for event in self.kittest.take_events() {
            match event {
                kittest::Event::ActionRequest(e) => {
                    self.input.events.push(Event::AccessKitActionRequest(e));
                }
                kittest::Event::Simulated(e) => match e {
                    SimulatedEvent::CursorMoved { position } => {
                        self.input.events.push(Event::PointerMoved(Pos2::new(
                            position.x as f32,
                            position.y as f32,
                        )));
                    }
                    SimulatedEvent::MouseInput { state, button } => {
                        let button = pointer_button_to_egui(button);
                        if let Some(button) = button {
                            self.input.events.push(Event::PointerButton {
                                button,
                                modifiers: self.modifiers,
                                pos: self.last_mouse_pos,
                                pressed: matches!(state, ElementState::Pressed),
                            });
                        }
                    }
                    SimulatedEvent::Ime(text) => {
                        self.input.events.push(Event::Text(text));
                    }
                    SimulatedEvent::KeyInput { state, key } => {
                        match key {
                            kittest::Key::Alt => {
                                self.modifiers.alt = matches!(state, ElementState::Pressed);
                            }
                            kittest::Key::Command => {
                                self.modifiers.command = matches!(state, ElementState::Pressed);
                            }
                            kittest::Key::Control => {
                                self.modifiers.ctrl = matches!(state, ElementState::Pressed);
                            }
                            kittest::Key::Shift => {
                                self.modifiers.shift = matches!(state, ElementState::Pressed);
                            }
                            _ => {}
                        }
                        let key = kittest_key_to_egui(key);
                        if let Some(key) = key {
                            self.input.events.push(Event::Key {
                                key,
                                modifiers: self.modifiers,
                                pressed: matches!(state, ElementState::Pressed),
                                repeat: false,
                                physical_key: None,
                            });
                        }
                    }
                },
            }
        }

        let mut output = self.ctx.run(self.input.take(), self.update_fn.as_mut());
        self.kittest.update(
            output
                .platform_output
                .accesskit_update
                .take()
                .expect("AccessKit was disabled"),
        );
        self.texture_deltas
            .push(mem::take(&mut output.textures_delta));
        self.output = output;
    }

    /// Access the [`egui::RawInput`] for the next frame.
    pub fn input(&self) -> &egui::RawInput {
        &self.input
    }

    /// Access the [`egui::RawInput`] for the next frame mutably.
    pub fn input_mut(&mut self) -> &mut egui::RawInput {
        &mut self.input
    }

    /// Access the [`egui::FullOutput`] for the last frame.
    pub fn output(&self) -> &egui::FullOutput {
        &self.output
    }

    /// Access the [`kittest::State`].
    pub fn kittest_state(&self) -> &kittest::State {
        &self.kittest
    }
}

impl<'t, 'n, 'h> Queryable<'t, 'n> for Harness<'h>
where
    'n: 't,
{
    fn node(&'n self) -> Node<'t> {
        self.kittest_state().node()
    }
}
