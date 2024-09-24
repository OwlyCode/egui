use crate::texture_to_bytes::texture_to_bytes;
use crate::Harness;
use egui_wgpu::wgpu::{Backends, InstanceDescriptor, StoreOp, TextureFormat};
use egui_wgpu::{wgpu, ScreenDescriptor};
use image::RgbaImage;
use std::iter::once;
use wgpu::Maintain;

impl Harness {
    pub fn image(&self, _renderer: TestRenderer) {}
}

pub struct TestRenderer {
    renderer: egui_wgpu::Renderer,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Default for TestRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRenderer {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(InstanceDescriptor::default());

        let adapters = instance.enumerate_adapters(Backends::all());
        let adapter = adapters.first().expect("No adapter found");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Egui Device"),
                memory_hints: Default::default(),
                required_limits: Default::default(),
                required_features: Default::default(),
            },
            None,
        ))
        .expect("Failed to create device");

        let renderer = egui_wgpu::Renderer::new(&device, TextureFormat::Rgba8Unorm, None, 1, true);

        Self {
            renderer,
            device,
            queue,
        }
    }

    pub fn render(&mut self, harness: &Harness) -> RgbaImage {
        for delta in &harness.texture_deltas {
            for (id, image_delta) in &delta.set {
                self.renderer
                    .update_texture(&self.device, &self.queue, *id, image_delta);
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Egui Command Encoder"),
            });

        let size = harness.ctx.screen_rect().size() * harness.ctx.pixels_per_point();
        let screen = ScreenDescriptor {
            pixels_per_point: harness.ctx.pixels_per_point(),
            size_in_pixels: [size.x as u32, size.y as u32],
        };

        let tesselated = harness.ctx.tessellate(
            harness.output().shapes.clone(),
            harness.ctx.pixels_per_point(),
        );

        let user_buffers = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &tesselated,
            &screen,
        );

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Egui Texture"),
            size: wgpu::Extent3d {
                width: size.x as u32,
                height: size.y as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Egui Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                })
                .forget_lifetime();

            self.renderer.render(&mut pass, &tesselated, &screen);
        }

        self.queue
            .submit(user_buffers.into_iter().chain(once(encoder.finish())));

        self.device.poll(Maintain::Wait);

        texture_to_bytes(&self.device, &self.queue, &texture)
    }
}