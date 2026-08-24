//! wgpu setup and per-frame drawing. Owns the GPU handles, the three static
//! meshes (cylinder/cuboid/sphere), and rebuilds per-instance buffers every
//! frame from whatever `instance::build_instances` produces for the current
//! `Scene` state.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
use wgpu::util::DeviceExt;

use crate::geometry::{self, Mesh};
use crate::instance::InstanceRaw;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl GpuMesh {
    fn upload(device: &wgpu::Device, mesh: &Mesh, label: &str) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} vertices")),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} indices")),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        }
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    cylinder_mesh: GpuMesh,
    cuboid_mesh: GpuMesh,
    sphere_mesh: GpuMesh,
    teapot_mesh: GpuMesh,
    scene_center: Vec3,
    scene_radius: f32,
    /// Set by `device.set_device_lost_callback` (registered once, in
    /// `new`) the moment the GPU device actually goes away — a driver
    /// reset/TDR, waking from sleep, etc. Real, hit-in-the-field failure
    /// mode (see docs/ROADMAP.md): wgpu's *default* behavior on any
    /// subsequent call into a lost device (e.g. `surface.get_current_texture`)
    /// is to panic via its internal uncaptured-error handler, not return a
    /// catchable `Result` — so the fix is to never make that call again
    /// once this flag is set, checked at the very top of `draw_frame`
    /// before touching the surface at all.
    device_lost: Arc<AtomicBool>,
}

const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<geometry::Vertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
};

const INSTANCE_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Instance,
    attributes: &wgpu::vertex_attr_array![
        2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x3,
    ],
};

impl Renderer {
    /// Generic over anything implementing the raw-window-handle traits wgpu
    /// needs for surface creation — not just `winit::window::Window` — so
    /// `pipes-xscreensaver` can reuse this exact renderer with a raw X11
    /// window handle instead of a winit-owned one. `size` is taken
    /// explicitly rather than queried from the window (e.g.
    /// `winit::window::Window::inner_size`) since a raw X11 handle has no
    /// such method; callers query it themselves however is native to their
    /// windowing setup.
    pub async fn new<W>(window: Arc<W>, size: (u32, u32), scene_bounds: (i32, i32, i32)) -> Self
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let (width, height) = size;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window)
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no compatible GPU adapter found");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("neo_win_pipes device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("failed to create device");

        let device_lost = Arc::new(AtomicBool::new(false));
        {
            let flag = device_lost.clone();
            device.set_device_lost_callback(move |reason, message| {
                tracing::error!(?reason, message, "wgpu device lost");
                flag.store(true, Ordering::SeqCst);
            });
        }

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_view = create_depth_view(&device, &config);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera uniform"),
            contents: bytemuck::cast_slice(&[CameraUniform {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pipes shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipes pipeline layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipes pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[VERTEX_LAYOUT, INSTANCE_LAYOUT],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // No backface culling: lighting uses each vertex's authored
                // normal directly (see shader.wgsl), not a winding-derived
                // face normal, so culling buys nothing but a very small
                // amount of fill-rate savings at this scene's tiny polygon
                // counts — and it isn't free to get right by hand for every
                // procedural mesh. Concretely: the teapot's lathed body/
                // spout came out with inconsistent winding relative to the
                // other meshes (only caught by actually launching it and
                // looking, not by any geometry unit test), and got silently
                // culled to a sliver. Rather than hand-verify winding for
                // every future mesh, disabling culling removes the whole
                // failure mode.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let cylinder_mesh = GpuMesh::upload(&device, &geometry::cylinder(1.0, 16), "cylinder");
        let cuboid_mesh = GpuMesh::upload(&device, &geometry::cuboid(1.0), "cuboid");
        let sphere_mesh = GpuMesh::upload(&device, &geometry::sphere(1.0, 12, 16), "sphere");
        let teapot_mesh = GpuMesh::upload(&device, &geometry::teapot(), "teapot");

        let (bw, bh, bd) = scene_bounds;
        let scene_center = Vec3::new(bw as f32, bh as f32, bd as f32) * 0.5;
        let scene_radius = (bw.max(bh).max(bd) as f32) * 0.9;

        Self {
            surface,
            device,
            queue,
            config,
            depth_view,
            pipeline,
            camera_buffer,
            camera_bind_group,
            cylinder_mesh,
            cuboid_mesh,
            sphere_mesh,
            teapot_mesh,
            scene_center,
            scene_radius,
            device_lost,
        }
    }

    /// True once `set_device_lost_callback` has fired — see the field doc
    /// on `device_lost`. Callers (`pipes-app`/`pipes-settings`'s event
    /// loops) can use this to stop calling `render`/`render_with`
    /// entirely and show/log something once instead of getting `Err`
    /// back every single frame forever.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost.load(Ordering::SeqCst)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || self.is_device_lost() {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        // Same reasoning as `draw_frame`: `is_device_lost()` above can
        // still read `false` on the exact call that would otherwise panic
        // (e.g. a resize event arriving right as the device dies, before
        // the callback fires), so this is caught here too rather than
        // trusted to the flag check alone.
        let result = crate::diagnostics::run_suppressing_fatal_dialog(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.surface.configure(&self.device, &self.config);
                create_depth_view(&self.device, &self.config)
            }))
        });
        match result {
            Ok(depth_view) => self.depth_view = depth_view,
            Err(_) => {
                self.device_lost.store(true, Ordering::SeqCst);
                tracing::error!(
                    "caught a panic from the GPU backend while resizing — treating the device as lost"
                );
            }
        }
    }

    /// `orbit_seconds` drives a slow camera drift around the scene (when
    /// `camera.orbit_enabled`), echoing the original screensaver's optional
    /// rotation — see `docs/RESEARCH.md`. Depends only on `self`/`camera`/
    /// `orbit_seconds`, so every `Renderer` built from the same sim bounds
    /// produces an identical view matrix — the property `MonitorMode::Span`
    /// relies on to give every monitor's tile the same eye/target.
    fn view_matrix(&self, orbit_seconds: f32, camera: &crate::config::CameraConfig) -> Mat4 {
        let angle = if camera.orbit_enabled {
            orbit_seconds * camera.orbit_speed
        } else {
            0.0
        };
        let eye = self.scene_center
            + Vec3::new(
                angle.cos() * self.scene_radius,
                self.scene_radius * 0.55,
                angle.sin() * self.scene_radius,
            );
        Mat4::look_at_rh(eye, self.scene_center, Vec3::Y)
    }

    fn write_camera_uniform(&self, view_proj: Mat4) {
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[CameraUniform {
                view_proj: view_proj.to_cols_array_2d(),
            }]),
        );
    }

    /// `(fov_y_radians, near, far)` for this scene's ordinary symmetric
    /// camera — the same three values a single, non-spanned window's
    /// projection is built from. Exposed so a `MonitorMode::Span` caller
    /// (`pipes-app`) can build a per-monitor
    /// [`tile projection`](crate::tile::tile_projection) that describes the
    /// exact same overall field of view, just sliced across displays,
    /// instead of guessing/duplicating these constants.
    pub fn frustum_params(&self) -> (f32, f32, f32) {
        (45f32.to_radians(), 0.5, self.scene_radius * 6.0)
    }

    /// `viewport_wh` is the pixel size of the region being rendered into
    /// (the full window, or a preview pane's sub-rect), used for a correct
    /// (non-stretched) aspect ratio.
    fn update_camera(
        &self,
        orbit_seconds: f32,
        camera: &crate::config::CameraConfig,
        viewport_wh: (u32, u32),
    ) {
        let view = self.view_matrix(orbit_seconds, camera);
        let (fov_y, near, far) = self.frustum_params();
        let (vw, vh) = viewport_wh;
        let aspect = vw as f32 / vh.max(1) as f32;
        let proj = Mat4::perspective_rh(fov_y, aspect, near, far);
        self.write_camera_uniform(proj * view);
    }

    fn update_camera_with_projection(
        &self,
        orbit_seconds: f32,
        camera: &crate::config::CameraConfig,
        projection: Mat4,
    ) {
        let view = self.view_matrix(orbit_seconds, camera);
        self.write_camera_uniform(projection * view);
    }

    /// Renders one frame. `viewport` is `(x, y, width, height)` in physical
    /// pixels to draw the 3D scene into — `None` means "the whole surface"
    /// (the screensaver's use case); `Some(rect)` lets a caller (the
    /// settings app) reserve the rest of the window for its own UI, drawn
    /// in a separate pass afterward without this renderer needing to know
    /// anything about it.
    pub fn render(
        &mut self,
        orbit_seconds: f32,
        camera: &crate::config::CameraConfig,
        viewport: Option<(u32, u32, u32, u32)>,
        instances: &crate::instance::InstanceSets,
    ) -> Result<(), wgpu::SurfaceError> {
        self.render_with(orbit_seconds, camera, viewport, instances, |_, _, _, _| {})
    }

    /// Like [`Self::render`], but for `MonitorMode::Span`: `tile_projection`
    /// is a pre-computed off-axis projection (see
    /// [`crate::tile::tile_projection`]) for just this monitor's slice of
    /// the full virtual desktop, used in place of the ordinary symmetric
    /// projection `render`/`render_with` compute internally. Every monitor
    /// sharing a span still gets the same view matrix (same orbiting eye —
    /// see `view_matrix`), so tiles rendered this way reconstruct one
    /// continuous wide scene when their windows sit edge to edge. Always
    /// renders into the whole surface (`viewport: None`'s behavior) since
    /// spanned mode has no reserved UI area to carve out.
    pub fn render_tile(
        &mut self,
        orbit_seconds: f32,
        camera: &crate::config::CameraConfig,
        tile_projection: Mat4,
        instances: &crate::instance::InstanceSets,
    ) -> Result<(), wgpu::SurfaceError> {
        self.update_camera_with_projection(orbit_seconds, camera, tile_projection);
        self.draw_frame(None, instances, |_, _, _, _| {})
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Same as [`Self::render`], but calls `extra` with the same device,
    /// queue, command encoder, and surface view right after the pipes pass
    /// and before submit/present — letting a caller (the settings app) add
    /// its own render pass (e.g. an egui overlay) into the exact same
    /// frame, sharing GPU resources, without this type needing to know
    /// anything about egui.
    pub fn render_with(
        &mut self,
        orbit_seconds: f32,
        camera: &crate::config::CameraConfig,
        viewport: Option<(u32, u32, u32, u32)>,
        instances: &crate::instance::InstanceSets,
        extra: impl FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> Result<(), wgpu::SurfaceError> {
        let (_, _, vw, vh) = viewport.unwrap_or((0, 0, self.config.width, self.config.height));
        self.update_camera(orbit_seconds, camera, (vw, vh));
        self.draw_frame(viewport, instances, extra)
    }

    /// The actual GPU submission shared by [`Self::render_with`] and
    /// [`Self::render_tile`] — everything downstream of "the camera uniform
    /// is already written", so the two callers only differ in how they
    /// compute that uniform (a symmetric projection from a viewport size,
    /// vs. a pre-computed off-axis tile projection).
    /// Thin wrapper around `draw_frame_inner` that's the actual fix for the
    /// "Parent device is lost" crash (see the `device_lost` field doc):
    /// `set_device_lost_callback` alone turned out not to be reliable
    /// enough on its own. It can fire asynchronously relative to whatever
    /// frame actually hits the dead device, so `is_device_lost()` can
    /// still read `false` on the exact frame that's about to panic —
    /// confirmed by actually calling `Device::destroy()` mid-run in a test
    /// build and watching it panic before the callback's own log line
    /// ever appeared. `catch_unwind` here is the part that's actually
    /// timing-independent: whatever caused wgpu to panic internally (a
    /// lost device being the known real-world case, but this guards any
    /// panic from this call), it's caught, the device is marked lost right
    /// here regardless of whether the "official" callback also ever
    /// fires, and the caller gets an ordinary `Err` back instead of a
    /// crashed process.
    fn draw_frame(
        &mut self,
        viewport: Option<(u32, u32, u32, u32)>,
        instances: &crate::instance::InstanceSets,
        extra: impl FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> Result<(), wgpu::SurfaceError> {
        if self.is_device_lost() {
            return Err(wgpu::SurfaceError::Lost);
        }

        let result = crate::diagnostics::run_suppressing_fatal_dialog(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.draw_frame_inner(viewport, instances, extra)
            }))
        });

        match result {
            Ok(inner_result) => inner_result,
            Err(_) => {
                self.device_lost.store(true, Ordering::SeqCst);
                tracing::error!(
                    "caught a panic from the GPU backend mid-frame — treating the device as lost"
                );
                Err(wgpu::SurfaceError::Lost)
            }
        }
    }

    fn draw_frame_inner(
        &mut self,
        viewport: Option<(u32, u32, u32, u32)>,
        instances: &crate::instance::InstanceSets,
        extra: impl FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> Result<(), wgpu::SurfaceError> {
        let (vx, vy, vw, vh) = viewport.unwrap_or((0, 0, self.config.width, self.config.height));

        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });

        let round_buf = self.instance_buffer(&instances.round_segments, "round instances");
        let square_buf = self.instance_buffer(&instances.square_segments, "square instances");
        let joint_buf = self.instance_buffer(&instances.joints, "joint instances");
        let teapot_buf = self.instance_buffer(&instances.teapots, "teapot instances");

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.03,
                            g: 0.035,
                            b: 0.06,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
            pass.set_scissor_rect(vx, vy, vw, vh);

            self.draw_mesh_instances(
                &mut pass,
                &self.cylinder_mesh,
                &round_buf,
                instances.round_segments.len() as u32,
            );
            self.draw_mesh_instances(
                &mut pass,
                &self.cuboid_mesh,
                &square_buf,
                instances.square_segments.len() as u32,
            );
            self.draw_mesh_instances(
                &mut pass,
                &self.sphere_mesh,
                &joint_buf,
                instances.joints.len() as u32,
            );
            self.draw_mesh_instances(
                &mut pass,
                &self.teapot_mesh,
                &teapot_buf,
                instances.teapots.len() as u32,
            );
        }

        extra(&self.device, &self.queue, &mut encoder, &view);

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    fn instance_buffer(&self, instances: &[InstanceRaw], label: &str) -> Option<wgpu::Buffer> {
        if instances.is_empty() {
            return None;
        }
        Some(
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(instances),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
        )
    }

    fn draw_mesh_instances<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        mesh: &'a GpuMesh,
        instance_buffer: &'a Option<wgpu::Buffer>,
        count: u32,
    ) {
        let Some(buf) = instance_buffer else { return };
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, buf.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..mesh.index_count, 0, 0..count);
    }
}

fn create_depth_view(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
