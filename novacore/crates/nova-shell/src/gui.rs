//! NovaCore's native GUI: a `winit` window + `wgpu` renderer that draws
//! structured values through the [`nova_gpu`] instanced glyph/rect pipeline.
//!
//! Headless note: this needs a real display + GPU, so it is validated by
//! compilation here and run on a desktop.

use std::sync::Arc;

use nova_engine::Engine;
use nova_gpu::{build_instances, Instance, SHADER};
use nova_ui::{compute_layout, paint, value_to_node, Color, Dim, Dir, Edges, Node, Rect};
use nova_value::{Value, View};
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowBuilder};

use crate::atlas::GlyphAtlas;

const FG: Color = Color::rgb(0xc0, 0xca, 0xf5);
const ACCENT: Color = Color::rgb(0x7a, 0xa2, 0xf7);
const BG: [u8; 3] = [0x1a, 0x1b, 0x26];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    screen: [f32; 2],
    _pad: [f32; 2],
}

struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    globals: wgpu::Buffer,
    instances: wgpu::Buffer,
    instance_cap: usize,
    atlas: GlyphAtlas,
}

impl Renderer {
    async fn new(window: Arc<Window>) -> Renderer {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).expect("create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no GPU adapter");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .expect("no device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Glyph atlas → R8 texture.
        let atlas = GlyphAtlas::build(18.0);
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d { width: atlas.width, height: atlas.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &atlas.data,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(atlas.width), rows_per_image: Some(atlas.height) },
            wgpu::Extent3d { width: atlas.width, height: atlas.height, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("nova-gpu shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Uint32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let instance_cap = 4096;
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (instance_cap * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Renderer { surface, device, queue, config, pipeline, bind_group, globals, instances, instance_cap, atlas }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Build the scene (header + value view + prompt) and draw it.
    fn render(&mut self, value: &Value, input: &str) {
        let w = self.config.width as f32;
        let h = self.config.height as f32;

        let body = value_to_node(value, View::Auto);
        let mut root = Node::panel()
            .dir(Dir::Col)
            .gap(6.0)
            .padding(Edges::all(10.0))
            .child(Node::text("NovaCore — NovaLang  (type a pipeline, Enter to run)", ACCENT).height(Dim::Fixed(22.0)))
            .child(body)
            .child(Node::text(format!("\u{276f} {input}"), FG).height(Dim::Fixed(22.0)));
        compute_layout(&mut root, Rect::new(0.0, 0.0, w, h));
        let list = paint(&root);
        let data = build_instances(&list, &self.atlas);

        // Grow the instance buffer if needed.
        if data.len() > self.instance_cap {
            self.instance_cap = data.len().next_power_of_two();
            self.instances = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: (self.instance_cap * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&data));
        self.queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals { screen: [w, h], _pad: [0.0, 0.0] }),
        );

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(srgb_clear(BG)), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if !data.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instances.slice(..));
                pass.draw(0..6, 0..data.len() as u32);
            }
        }
        self.queue.submit([enc.finish()]);
        frame.present();
    }
}

fn srgb_clear(rgb: [u8; 3]) -> wgpu::Color {
    let f = |c: u8| {
        let s = c as f64 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    wgpu::Color { r: f(rgb[0]), g: f(rgb[1]), b: f(rgb[2]), a: 1.0 }
}

/// Launch the native NovaCore window.
pub fn run() {
    let event_loop = EventLoop::new().expect("event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("NovaCore")
            .with_inner_size(LogicalSize::new(1100.0, 720.0))
            .build(&event_loop)
            .expect("window"),
    );

    let mut renderer = pollster::block_on(Renderer::new(window.clone()));
    let mut engine = Engine::new();
    let mut value = engine.eval("ls").unwrap_or(Value::Null);
    let mut input = String::new();

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);
            let Event::WindowEvent { event, window_id } = event else { return };
            if window_id != window.id() {
                return;
            }
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => {
                    renderer.resize(size);
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => renderer.render(&value, &input),
                WindowEvent::KeyboardInput { event: key_event, .. } if key_event.state == ElementState::Pressed => {
                    match key_event.logical_key {
                        Key::Named(NamedKey::Enter) => {
                            let line = input.trim().to_string();
                            if line == "exit" {
                                elwt.exit();
                            } else if !line.is_empty() {
                                value = engine.eval(&line).unwrap_or_else(|e| Value::Error(nova_value::ValueError {
                                    kind: "error".into(),
                                    message: e.to_string(),
                                }));
                                input.clear();
                            }
                        }
                        Key::Named(NamedKey::Backspace) => {
                            input.pop();
                        }
                        Key::Named(NamedKey::Space) => input.push(' '),
                        _ => {
                            if let Some(text) = key_event.text {
                                for c in text.chars() {
                                    if !c.is_control() {
                                        input.push(c);
                                    }
                                }
                            }
                        }
                    }
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .expect("event loop run");
}
