use fj_math::{Aabb, Point};
use std::mem::size_of;
use wgpu::util::DeviceExt;

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use vger3d::{
    geometries::Geometries,
    uniforms::{Rect, Transforms},
    vertices::{Vertex, Vertices},
};

mod path;
use path::*;

mod scene;
use scene::*;

mod prim;
use prim::*;

pub mod defs;
use defs::*;

mod paint;
use paint::*;

mod gpu_vec;
use gpu_vec::*;

pub mod color;
pub use color::Color;

mod atlas;

mod glyphs;
use glyphs::GlyphCache;

pub mod vger3d;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
struct Uniforms {
    size: [f32; 2],
}

#[derive(Copy, Clone, Debug)]
pub struct PaintIndex {
    index: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct LineMetrics {
    pub glyph_start: usize,
    pub glyph_end: usize,
    pub bounds: LocalRect,
}

pub struct Vger {
    scenes: [Scene; 3],
    cur_scene: usize,
    cur_layer: usize,
    tx_stack: Vec<LocalToWorld>,
    device_px_ratio: f32,
    screen_size: ScreenSize,
    paint_count: usize,
    pipeline: wgpu::RenderPipeline,
    uniform_bind_group: wgpu::BindGroup,
    uniforms: GPUVec<Uniforms>,
    xform_count: usize,
    path_scanner: PathScanner,
    pen: LocalPoint,
    glyph_cache: GlyphCache,
    layout: Layout,

    // 3d
    // pub viewport3d: Rect,              // TODO: do we need this ???
    pub transforms3d: Transforms, // TODO: store it in a map keyed by ViewId
    transforms3d_buffer: wgpu::Buffer, // TODO: store it in a map keyed by ViewId
    bind_group3d: wgpu::BindGroup,
    pipeline3d: wgpu::RenderPipeline,
    pub mesh: Vertices, // TODO: store it in a map keyed by ViewId
    pub aabb: Aabb<3>,  // TOOD: store it in a map keyed by ViewId
}

impl Vger {
    /// Create a new renderer given a device and output pixel format.
    pub fn new(
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
        // texture_format3d: wgpu::TextureFormat, // TODO: do we need this ?
    ) -> Self {
        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shader.wgsl"
            ))),
        });

        let scenes = [
            Scene::new(&device),
            Scene::new(&device),
            Scene::new(&device),
        ];

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("uniform_bind_group_layout"),
            });

        let glyph_cache = GlyphCache::new(device);

        let texture_view = glyph_cache.create_view();

        let uniforms = GPUVec::new_uniforms(device, "uniforms");

        let glyph_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                uniforms.bind_group_entry(0),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&glyph_sampler),
                },
            ],
            label: Some("vger bind group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &Scene::bind_group_layout(&device),
                &uniform_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let blend_comp = wgpu::BlendComponent {
            operation: wgpu::BlendOperation::Add,
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: texture_format,
                    blend: Some(wgpu::BlendState {
                        color: blend_comp,
                        alpha: blend_comp,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let layout = Layout::new(CoordinateSystem::PositiveYUp);

        ////////// 3d

        let shader3d = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "vger3d/shader.wgsl"
            ))),
        });

        // let viewport3d = Rect::default();
        let transforms3d = Transforms::default();
        let transforms3d_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[transforms3d]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout3d =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Transforms>() as u64),
                    },
                    count: None,
                }],
                label: None,
            });

        let bind_group3d = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout3d,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &transforms3d_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
            label: None,
        });

        let pipeline3d_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout3d],
            push_constant_ranges: &[],
        });
        let pipeline3d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline3d_layout),
            vertex: wgpu::VertexState {
                module: &shader3d,
                entry_point: "vs_main",
                // buffers: &[],
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3,
                        1 => Float32x3,
                        2 => Float32x4,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader3d,
                //entry_point: "fs_main",
                // entry_point: "frag_model",
                entry_point: "frag_mesh",
                // targets: &[texture_format3d.into()],
                targets: &[texture_format.into()],
                // TODO: check wether we need the one below
                // targets: &[wgpu::ColorTargetState {
                //     format: texture_format3d,
                //     blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                //     write_mask: wgpu::ColorWrites::ALL,
                // }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                // polygon_mode: wgpu::PolygonMode::Fill,
                polygon_mode: wgpu::PolygonMode::Line,
                conservative: false,
            },
            depth_stencil: None, // if we do not disable: ONLY with z==0 visible
            // depth_stencil: Some(wgpu::DepthStencilState {
            //     format: DEPTH_FORMAT,
            //     depth_write_enabled: true,
            //     depth_compare: wgpu::CompareFunction::LessEqual,
            //     stencil: wgpu::StencilState {
            //         front: wgpu::StencilFaceState::IGNORE,
            //         back: wgpu::StencilFaceState::IGNORE,
            //         read_mask: 0,
            //         write_mask: 0,
            //     },
            //     // stencil: Default::default(),
            //     bias: wgpu::DepthBiasState::default(),
            // }),
            multisample: wgpu::MultisampleState::default(),
            // multisample: wgpu::MultisampleState {
            //     count: 1,
            //     mask: !0,
            //     alpha_to_coverage_enabled: false,
            // },
            multiview: None,
        });

        let geometries = Geometries::new(
            &device,
            &Vertices::empty(),
            &Vertices::empty(),
            Aabb {
                min: Point::from([0.0, 0.0, 0.0]),
                max: Point::from([0.0, 0.0, 0.0]),
            },
        );

        Self {
            scenes,
            cur_scene: 0,
            cur_layer: 0,
            tx_stack: vec![],
            device_px_ratio: 1.0,
            screen_size: ScreenSize::new(512.0, 512.0),
            paint_count: 0,
            pipeline,
            uniforms,
            uniform_bind_group,
            xform_count: 0,
            path_scanner: PathScanner::new(),
            pen: LocalPoint::zero(),
            glyph_cache,
            layout,

            // 3d:
            // viewport3d,
            transforms3d,
            transforms3d_buffer,
            bind_group3d,
            pipeline3d,
            mesh: Vertices::empty(),
            aabb: Aabb {
                min: Point::from([0.0, 0.0, 0.0]),
                max: Point::from([0.0, 0.0, 0.0]),
            },
        }
    }

    /// Begin rendering.
    pub fn begin(&mut self, window_width: f32, window_height: f32, device_px_ratio: f32) {
        self.device_px_ratio = device_px_ratio;
        self.cur_layer = 0;
        self.screen_size = ScreenSize::new(window_width, window_height);
        self.uniforms.data.clear();
        self.uniforms.data.push(Uniforms {
            size: [window_width, window_height],
        });
        self.cur_scene = (self.cur_scene + 1) % 3;
        self.scenes[self.cur_scene].clear();
        self.tx_stack.clear();
        self.tx_stack.push(LocalToWorld::identity());
        self.paint_count = 0;
        self.xform_count = 0;
        self.pen = LocalPoint::zero();
    }

    /// Saves rendering state (just the current transform).
    pub fn save(&mut self) {
        self.tx_stack.push(*self.tx_stack.last().unwrap())
    }

    /// Restores rendering state (just the current transform).
    pub fn restore(&mut self) {
        self.tx_stack.pop();
    }

    // TODO: allow to update self.mesh and self.aabb
    // pub fn update_geometry(&mut self, mesh: Vertices, lines: Vertices, aabb: Aabb<3>) {
    //     self.geometries = Geometries::new(&self.device, &mesh, &lines, aabb);
    // }

    // TODO: add depth_view handling somewhere
    // pub fn handle_resize(&mut self, size: PhysicalSize<u32>) {
    //     self.surface_config.width = size.width;
    //     self.surface_config.height = size.height;
    //
    //     self.surface.configure(&self.device, &self.surface_config);
    //
    //     let depth_view =
    //         Self::create_depth_buffer(&self.device, &self.surface_config);
    //     self.depth_view = depth_view;
    // }

    /// Encode all rendering to a command buffer.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        render_pass: &wgpu::RenderPassDescriptor,
        queue: &wgpu::Queue,
    ) {
        self.scenes[self.cur_scene].update(queue);
        self.uniforms.update(queue);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vger encoder"),
        });

        self.glyph_cache.update(device, &mut encoder);

        // 3d uniforms
        queue.write_buffer(
            &self.transforms3d_buffer,
            0,
            bytemuck::cast_slice(&[self.transforms3d]),
        );
        // TODO: refactor to: self.transforms.update(queue);

        let geometries = Geometries::new(&device, &self.mesh, &Vertices::empty(), self.aabb);

        {
            let mut rpass = encoder.begin_render_pass(render_pass);

            // 3d pipeline
            rpass.set_pipeline(&self.pipeline3d);
            rpass.set_bind_group(0, &self.bind_group3d, &[]);
            rpass.set_vertex_buffer(0, geometries.mesh.vertex_buffer.slice(..));
            rpass.set_index_buffer(
                geometries.mesh.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            rpass.draw_indexed(0..geometries.mesh.num_indices, 0, 0..1);

            // 2d pipeline
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(
                0,
                &self.scenes[self.cur_scene].bind_groups[self.cur_layer],
                &[], // dynamic offsets
            );
            rpass.set_bind_group(1, &self.uniform_bind_group, &[]);
            let n = self.scenes[self.cur_scene].prims[self.cur_layer].data.len();
            // println!("encoding {:?} prims", n);
            rpass.draw(/*vertices*/ 0..4, /*instances*/ 0..(n as u32))
        }
        queue.submit(Some(encoder.finish()));

        // If we're getting close to full, reset the glyph cache.
        let usage = self.glyph_cache.usage();
        // println!("glyph cache usage {}", usage);
        if usage > 0.7 {
            // println!("clearing glyph cache");
            self.glyph_cache.clear();
        }
    }

    fn render(&mut self, prim: Prim) {
        self.scenes[self.cur_scene].prims[self.cur_layer]
            .data
            .push(prim);
    }

    /// Fills a circle.
    pub fn fill_circle<Pt: Into<LocalPoint>>(
        &mut self,
        center: Pt,
        radius: f32,
        paint_index: PaintIndex,
    ) {
        let mut prim = Prim::default();
        prim.prim_type = PrimType::Circle as u32;
        let c: LocalPoint = center.into();
        prim.cvs[0] = c.x;
        prim.cvs[1] = c.y;
        prim.radius = radius;
        prim.paint = paint_index.index as u32;
        prim.quad_bounds = [c.x - radius, c.y - radius, c.x + radius, c.y + radius];
        prim.tex_bounds = prim.quad_bounds;
        prim.xform = self.add_xform() as u32;

        self.render(prim);
    }

    /// Strokes an arc.
    pub fn stroke_arc<Pt: Into<LocalPoint>>(
        &mut self,
        center: Pt,
        radius: f32,
        width: f32,
        rotation: f32,
        aperture: f32,
        paint_index: PaintIndex,
    ) {
        let mut prim = Prim::default();
        prim.prim_type = PrimType::Arc as u32;
        prim.radius = radius;
        let c: LocalPoint = center.into();
        prim.cvs = [
            c.x,
            c.y,
            rotation.sin(),
            rotation.cos(),
            aperture.sin(),
            aperture.cos(),
        ];
        prim.width = width;
        prim.paint = paint_index.index as u32;
        prim.quad_bounds = [
            c.x - radius - width,
            c.y - radius - width,
            c.x + radius + width,
            c.y + radius + width,
        ];
        prim.tex_bounds = prim.quad_bounds;
        prim.xform = self.add_xform() as u32;

        self.render(prim);
    }

    /// Fills a rectangle.
    pub fn fill_rect<Rect: Into<LocalRect>>(
        &mut self,
        rect: Rect,
        radius: f32,
        paint_index: PaintIndex,
    ) {
        let mut prim = Prim::default();
        prim.prim_type = PrimType::Rect as u32;
        let r: LocalRect = rect.into();
        let min = r.min();
        let max = r.max();
        prim.cvs[0] = min.x;
        prim.cvs[1] = min.y;
        prim.cvs[2] = max.x;
        prim.cvs[3] = max.y;
        prim.radius = radius;
        prim.paint = paint_index.index as u32;
        prim.quad_bounds = [min.x, min.y, max.x, max.y];
        prim.tex_bounds = prim.quad_bounds;
        prim.xform = self.add_xform() as u32;

        self.render(prim);
    }

    /// Strokes a rectangle.
    pub fn stroke_rect(
        &mut self,
        min: LocalPoint,
        max: LocalPoint,
        radius: f32,
        width: f32,
        paint_index: PaintIndex,
    ) {
        let mut prim = Prim::default();
        prim.prim_type = PrimType::RectStroke as u32;
        prim.cvs[0] = min.x;
        prim.cvs[1] = min.y;
        prim.cvs[2] = max.x;
        prim.cvs[3] = max.y;
        prim.radius = radius;
        prim.width = width;
        prim.paint = paint_index.index as u32;
        prim.quad_bounds = [min.x - width, min.y - width, max.x + width, max.y + width];
        prim.tex_bounds = prim.quad_bounds;
        prim.xform = self.add_xform() as u32;

        self.render(prim);
    }

    /// Strokes a line segment.
    pub fn stroke_segment<Pt: Into<LocalPoint>>(
        &mut self,
        a: Pt,
        b: Pt,
        width: f32,
        paint_index: PaintIndex,
    ) {
        let mut prim = Prim::default();
        prim.prim_type = PrimType::Segment as u32;
        let ap: LocalPoint = a.into();
        let bp: LocalPoint = b.into();
        prim.cvs[0] = ap.x;
        prim.cvs[1] = ap.y;
        prim.cvs[2] = bp.x;
        prim.cvs[3] = bp.y;
        prim.width = width;
        prim.paint = paint_index.index as u32;
        prim.quad_bounds = [
            ap.x.min(bp.x),
            ap.y.min(bp.y),
            ap.x.max(bp.x),
            ap.y.max(bp.y),
        ];
        prim.tex_bounds = prim.quad_bounds;
        prim.xform = self.add_xform() as u32;

        self.render(prim);
    }

    /// Strokes a quadratic bezier segment.
    pub fn stroke_bezier<Pt: Into<LocalPoint>>(
        &mut self,
        a: Pt,
        b: Pt,
        c: Pt,
        width: f32,
        paint_index: PaintIndex,
    ) {
        let mut prim = Prim::default();
        prim.prim_type = PrimType::Bezier as u32;
        let ap: LocalPoint = a.into();
        let bp: LocalPoint = b.into();
        let cp: LocalPoint = c.into();
        prim.cvs[0] = ap.x;
        prim.cvs[1] = ap.y;
        prim.cvs[2] = bp.x;
        prim.cvs[3] = bp.y;
        prim.cvs[4] = cp.x;
        prim.cvs[5] = cp.y;
        prim.width = width;
        prim.paint = paint_index.index as u32;
        prim.quad_bounds = [
            ap.x.min(bp.x).min(cp.x) - width,
            ap.y.min(bp.y).min(cp.y) - width,
            ap.x.max(bp.x).max(cp.x) + width,
            ap.y.max(bp.y).max(cp.y) + width,
        ];
        prim.tex_bounds = prim.quad_bounds;
        prim.xform = self.add_xform() as u32;

        self.render(prim);
    }

    /// Move the pen to a point (path fills only)
    pub fn move_to<Pt: Into<LocalPoint>>(&mut self, p: Pt) {
        self.pen = p.into();
    }

    /// Makes a quadratic curve to a point (path fills only)
    pub fn quad_to<Pt: Into<LocalPoint>>(&mut self, b: Pt, c: Pt) {
        let cp: LocalPoint = c.into();
        self.path_scanner
            .segments
            .push(PathSegment::new(self.pen, b.into(), cp));
        self.pen = cp;
    }

    fn add_cv<Pt: Into<LocalPoint>>(&mut self, p: Pt) {
        self.scenes[self.cur_scene].cvs.data.push(p.into())
    }

    /// Fills a path.
    pub fn fill(&mut self, paint_index: PaintIndex) {
        let xform = self.add_xform();

        self.path_scanner.init();

        while self.path_scanner.next() {
            let mut prim = Prim::default();
            prim.prim_type = PrimType::PathFill as u32;
            prim.paint = paint_index.index as u32;
            prim.xform = xform as u32;
            prim.start = self.scenes[self.cur_scene].cvs.data.len() as u32;

            let mut x_interval = Interval {
                a: std::f32::MAX,
                b: std::f32::MIN,
            };

            let mut index = self.path_scanner.first;
            while let Some(a) = index {
                for i in 0..3 {
                    let p = self.path_scanner.segments[a].cvs[i];
                    self.add_cv(p);
                    x_interval.a = x_interval.a.min(p.x);
                    x_interval.b = x_interval.b.max(p.x);
                }
                prim.count += 1;

                index = self.path_scanner.segments[a].next;
            }

            prim.quad_bounds[0] = x_interval.a;
            prim.quad_bounds[1] = self.path_scanner.interval.a;
            prim.quad_bounds[2] = x_interval.b;
            prim.quad_bounds[3] = self.path_scanner.interval.b;
            prim.tex_bounds = prim.quad_bounds;

            self.render(prim);
        }
    }

    fn setup_layout(&mut self, text: &str, size: u32, max_width: Option<f32>) {
        let scale = self.device_px_ratio;

        self.layout.reset(&LayoutSettings {
            max_width: max_width.map(|w| w * scale),
            ..LayoutSettings::default()
        });

        let scaled_size = size as f32 * scale;

        self.layout.append(
            &[&self.glyph_cache.font],
            &TextStyle::new(text, scaled_size, 0),
        );
    }

    /// Renders text.
    pub fn text(&mut self, text: &str, size: u32, color: Color, max_width: Option<f32>) {
        self.setup_layout(text, size, max_width);

        let padding = 2.0 as f32;

        let scale = self.device_px_ratio;
        let scaled_size = size as f32 * scale;

        let paint = self.color_paint(color);
        let xform = self.add_xform() as u32;

        let mut i = 0;
        let mut prims = vec![];
        for glyph in self.layout.glyphs() {
            let c = text.chars().nth(i).unwrap();
            // println!("glyph {:?}", c);
            let info = self.glyph_cache.get_glyph(c, scaled_size);

            if let Some(rect) = info.rect {
                let mut prim = Prim::default();
                prim.prim_type = PrimType::Glyph as u32;
                prim.xform = xform;
                assert!(glyph.width == rect.width as usize);
                assert!(glyph.height == rect.height as usize);

                prim.quad_bounds = [
                    (glyph.x - padding) / scale,
                    (glyph.y - padding) / scale,
                    (glyph.x + padding + glyph.width as f32) / scale,
                    (glyph.y + padding + glyph.height as f32) / scale,
                ];
                // println!("quad_bounds: {:?}", prim.quad_bounds);

                // The extra +/- 1.0 offset ensures we cover the same
                // number of pixels when rasterizing as the glyph
                // in the texture.
                prim.tex_bounds = [
                    rect.x as f32 - padding,
                    (rect.y + rect.height) as f32 + padding,
                    (rect.x + rect.width) as f32 + padding + 1.0,
                    rect.y as f32 - padding - 1.0,
                ];
                prim.paint = paint.index as u32;
                // println!("tex_bounds: {:?}", prim.tex_bounds);

                prims.push(prim);
            }

            i += 1;
        }

        for prim in prims {
            self.render(prim);
        }
    }

    /// Calculates the bounds for text.
    pub fn text_bounds(&mut self, text: &str, size: u32, max_width: Option<f32>) -> LocalRect {
        self.setup_layout(text, size, max_width);

        let mut min = LocalPoint::new(f32::MAX, f32::MAX);
        let mut max = LocalPoint::new(f32::MIN, f32::MIN);

        let scale = self.device_px_ratio;

        for glyph in self.layout.glyphs() {
            min = min.min([glyph.x / scale, glyph.y / scale].into());
            max = max.max(
                [
                    (glyph.x + glyph.width as f32) / scale,
                    (glyph.y + glyph.height as f32) / scale,
                ]
                .into(),
            );
        }

        LocalRect::new(min, (max - min).into())
    }

    /// Returns local coordinates of glyphs.
    pub fn glyph_positions(
        &mut self,
        text: &str,
        size: u32,
        max_width: Option<f32>,
    ) -> Vec<LocalRect> {
        let mut rects = vec![];
        rects.reserve(text.len());

        self.setup_layout(text, size, max_width);

        let s = 1.0 / self.device_px_ratio;

        for glyph in self.layout.glyphs() {
            rects.push(
                LocalRect::new(
                    [glyph.x, glyph.y].into(),
                    [glyph.width as f32, glyph.height as f32].into(),
                )
                .scale(s, s),
            )
        }

        rects
    }

    pub fn line_metrics(
        &mut self,
        text: &str,
        size: u32,
        max_width: Option<f32>,
    ) -> Vec<LineMetrics> {
        self.setup_layout(text, size, max_width);
        let s = 1.0 / self.device_px_ratio;

        let mut rects = vec![];
        rects.reserve(text.len());

        let glyphs = self.layout.glyphs();

        if let Some(lines) = self.layout.lines() {
            for line in lines {
                let mut rect = LocalRect::zero();

                for i in line.glyph_start..line.glyph_end {
                    let glyph = glyphs[i];
                    rect = rect.union(&LocalRect::new(
                        [glyph.x, glyph.y].into(),
                        [glyph.width as f32, glyph.height as f32].into(),
                    ));
                }
                rects.push(LineMetrics {
                    glyph_start: line.glyph_start,
                    glyph_end: line.glyph_end,
                    bounds: rect.scale(s, s),
                });
            }
        }

        rects
    }

    fn add_xform(&mut self) -> usize {
        if self.xform_count < MAX_PRIMS {
            let m = *self.tx_stack.last().unwrap();
            self.scenes[self.cur_scene]
                .xforms
                .data
                .push(m.to_3d().to_array());
            let n = self.xform_count;
            self.xform_count += 1;
            return n;
        }
        0
    }

    /// Translates the coordinate system.
    pub fn translate<Vec: Into<LocalVector>>(&mut self, offset: Vec) {
        if let Some(m) = self.tx_stack.last_mut() {
            *m = (*m).pre_translate(offset.into());
        }
    }

    fn add_paint(&mut self, paint: Paint) -> PaintIndex {
        if self.paint_count < MAX_PRIMS {
            self.scenes[self.cur_scene].paints.data.push(paint);
            self.paint_count += 1;
            return PaintIndex {
                index: self.paint_count - 1,
            };
        }
        PaintIndex { index: 0 }
    }

    pub fn color_paint(&mut self, color: Color) -> PaintIndex {
        self.add_paint(Paint::solid_color(color))
    }

    pub fn linear_gradient<Pt: Into<LocalPoint>>(
        &mut self,
        start: Pt,
        end: Pt,
        inner_color: Color,
        outer_color: Color,
        glow: f32,
    ) -> PaintIndex {
        self.add_paint(Paint::linear_gradient(
            start.into(),
            end.into(),
            inner_color,
            outer_color,
            glow,
        ))
    }
}
