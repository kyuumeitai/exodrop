use crate::hud::{HudDrawList, HudVertex};
use crate::shaders::{BLOOM_WGSL, FX_WGSL, HUD_WGSL, PBR_WGSL, SHADOW_WGSL, SKY_WGSL, TONEMAP_WGSL};
use bytemuck::{Pod, Zeroable};
use exo_assets::{AssetCatalog, MeshData, MeshId, PbrVertex};
use exo_core::BiomeConfig;
use glam::{Mat4, Vec3, Vec4};
use std::path::Path;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

const SHADOW_SIZE: u32 = 2048;
const MAX_INSTANCES: usize = 4096;
const MAX_FX_VERTS: usize = 24_000;
const MAX_HUD_VERTS: usize = 48_000;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y_rad: f32,
    pub near: f32,
    pub far: f32,
    pub roll: f32,
}

impl Camera {
    pub fn view_matrix(&self) -> Mat4 {
        let forward = (self.target - self.eye).normalize_or_zero();
        let right = forward.cross(self.up).normalize_or_zero();
        let up_base = right.cross(forward).normalize_or_zero();
        let rolled_up = up_base * self.roll.cos() + right * self.roll.sin();
        Mat4::look_at_rh(self.eye, self.target, rolled_up)
    }

    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y_rad, aspect.max(0.1), self.near, self.far)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj_matrix(aspect) * self.view_matrix()
    }

    /// Project a 3D world position into 2D screen pixel coordinates. Returns `None` if behind camera.
    pub fn project_to_screen(&self, world_pos: Vec3, width: f32, height: f32) -> Option<(Vec3, bool)> {
        let vp = self.view_proj(width / height.max(1.0));
        let clip = vp * world_pos.extend(1.0);
        if clip.w <= 0.01 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        let sx = (ndc.x * 0.5 + 0.5) * width;
        let sy = (0.5 - ndc.y * 0.5) * height;
        let on_screen = ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && ndc.z >= 0.0 && ndc.z <= 1.0;
        Some((Vec3::new(sx, sy, ndc.z), on_screen))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub model: [[f32; 4]; 4],
    pub tint: [f32; 4],
    pub emissive_boost: [f32; 4], // rgb = additive emissive, a = multiplier for vertex emissive
}

impl InstanceData {
    pub fn new(transform: Mat4, tint: Vec3) -> Self {
        Self {
            model: transform.to_cols_array_2d(),
            tint: [tint.x, tint.y, tint.z, 1.0],
            emissive_boost: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn with_emissive(transform: Mat4, tint: Vec3, emissive_add: Vec3, mult: f32) -> Self {
        Self {
            model: transform.to_cols_array_2d(),
            tint: [tint.x, tint.y, tint.z, 1.0],
            emissive_boost: [emissive_add.x, emissive_add.y, emissive_add.z, mult],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub position: Vec3,
    pub radius: f32,
    pub color: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FxVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Complete scene frame submitted from gameplay to the custom renderer.
pub struct SceneFrame<'a> {
    pub camera: Camera,
    pub shadow_focus: Vec3,
    pub biome: &'a BiomeConfig,
    pub time: f32,
    pub boost_fx: f32,
    pub damage_fx: f32,
    pub instances: &'a [Vec<InstanceData>], // indexed by MeshId as usize
    pub lights: &'a [PointLight],
    pub fx_vertices: &'a [FxVertex],
    pub hud: &'a HudDrawList,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PointLightGpu {
    pos_radius: [f32; 4],
    color_intensity: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniformGpu {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    sky_zenith: [f32; 4],
    sky_horizon: [f32; 4],
    ground_bounce: [f32; 4],
    fog_params: [f32; 4],
    planet_color: [f32; 4],
    ring_color: [f32; 4],
    post_params: [f32; 4],
    lights: [PointLightGpu; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowUniformGpu {
    light_view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BloomParamsGpu {
    dir: [f32; 2],
    threshold: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostUniformGpu {
    boost_fx: f32,
    damage_fx: f32,
    time: f32,
    exposure: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HudUniformGpu {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

struct GpuMesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    index_count: u32,
}

struct ScreenTargets {
    hdr_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    bloom_a_view: wgpu::TextureView,
    bloom_b_view: wgpu::TextureView,
    bright_bg: wgpu::BindGroup,
    blur_h_bg: wgpu::BindGroup,
    blur_v_bg: wgpu::BindGroup,
    tonemap_bg: wgpu::BindGroup,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    output_format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,

    // Meshes
    catalog_meshes: Vec<GpuMesh>,
    terrain_mesh: Option<GpuMesh>,

    // Dynamic buffers
    frame_ub: wgpu::Buffer,
    shadow_ub: wgpu::Buffer,
    post_ub: wgpu::Buffer,
    hud_ub: wgpu::Buffer,
    bloom_bright_ub: wgpu::Buffer,
    bloom_h_ub: wgpu::Buffer,
    bloom_v_ub: wgpu::Buffer,
    instance_vb: wgpu::Buffer,
    terrain_instance_vb: wgpu::Buffer,
    fx_vb: wgpu::Buffer,
    hud_vb: wgpu::Buffer,

    // Static shadow map & samplers
    shadow_view: wgpu::TextureView,
    linear_sampler: wgpu::Sampler,

    // Bind group layouts & bind groups
    frame_bg: wgpu::BindGroup,
    shadow_pass_bg: wgpu::BindGroup,
    shadow_sample_bg: wgpu::BindGroup,
    hud_bg: wgpu::BindGroup,
    bloom_bgl: wgpu::BindGroupLayout,
    tonemap_bgl: wgpu::BindGroupLayout,

    // Pipelines
    shadow_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    pbr_pipeline: wgpu::RenderPipeline,
    fx_pipeline: wgpu::RenderPipeline,
    bright_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    tonemap_pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,

    targets: ScreenTargets,
}

impl Renderer {
    pub fn new_windowed(window: Arc<Window>, width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window).expect("create wgpu surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("request GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("exo-gfx-device"),
            ..Default::default()
        }))
        .expect("request GPU device");

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
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Self::init_internal(device, queue, Some(surface), Some(config), format, width.max(1), height.max(1))
    }

    pub fn new_headless(width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("request headless GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("exo-gfx-headless"),
            ..Default::default()
        }))
        .expect("request headless GPU device");

        Self::init_internal(
            device,
            queue,
            None,
            None,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            width.max(1),
            height.max(1),
        )
    }

    fn init_internal(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        surface_config: Option<wgpu::SurfaceConfiguration>,
        output_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let catalog = AssetCatalog::generate_all();
        let catalog_meshes: Vec<GpuMesh> = catalog
            .meshes
            .iter()
            .map(|m| upload_mesh(&device, m))
            .collect();

        let frame_ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_ub"),
            size: std::mem::size_of::<FrameUniformGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow_ub"),
            size: std::mem::size_of::<ShadowUniformGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let post_ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post_ub"),
            size: std::mem::size_of::<PostUniformGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud_ub"),
            size: std::mem::size_of::<HudUniformGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloom_bright_ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloom_bright_ub"),
            size: std::mem::size_of::<BloomParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloom_h_ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloom_h_ub"),
            size: std::mem::size_of::<BloomParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloom_v_ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloom_v_ub"),
            size: std::mem::size_of::<BloomParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance_vb"),
            size: (MAX_INSTANCES * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let identity_inst = [InstanceData::new(Mat4::IDENTITY, Vec3::ONE)];
        let terrain_instance_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_instance_vb"),
            contents: bytemuck::cast_slice(&identity_inst),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let fx_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx_vb"),
            size: (MAX_FX_VERTS * std::mem::size_of::<FxVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud_vb"),
            size: (MAX_HUD_VERTS * std::mem::size_of::<HudVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shadow map texture & comparison sampler
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            size: wgpu::Extent3d {
                width: SHADOW_SIZE,
                height: SHADOW_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Bind group layouts
        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let shadow_sample_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_sample_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let bloom_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let tonemap_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tonemap_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame_bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_ub.as_entire_binding(),
            }],
        });
        let shadow_pass_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_pass_bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_ub.as_entire_binding(),
            }],
        });
        let shadow_sample_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_sample_bg"),
            layout: &shadow_sample_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });
        let hud_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud_bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: hud_ub.as_entire_binding(),
            }],
        });

        // Vertex layouts
        let pbr_vb_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PbrVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 24, shader_location: 2 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 36, shader_location: 3 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 40, shader_location: 4 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 52, shader_location: 5 },
            ],
        };
        let inst_vb_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 6 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 7 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 8 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 9 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 10 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 80, shader_location: 11 },
            ],
        };

        // Compile all WGSL modules
        let shadow_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_wgsl"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_WGSL.into()),
        });
        let sky_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky_wgsl"),
            source: wgpu::ShaderSource::Wgsl(SKY_WGSL.into()),
        });
        let pbr_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pbr_wgsl"),
            source: wgpu::ShaderSource::Wgsl(PBR_WGSL.into()),
        });
        let fx_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_wgsl"),
            source: wgpu::ShaderSource::Wgsl(FX_WGSL.into()),
        });
        let bloom_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom_wgsl"),
            source: wgpu::ShaderSource::Wgsl(BLOOM_WGSL.into()),
        });
        let tonemap_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tonemap_wgsl"),
            source: wgpu::ShaderSource::Wgsl(TONEMAP_WGSL.into()),
        });
        let hud_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud_wgsl"),
            source: wgpu::ShaderSource::Wgsl(HUD_WGSL.into()),
        });

        let hdr_format = wgpu::TextureFormat::Rgba16Float;

        // 1. Shadow pipeline
        let shadow_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_pl"),
            bind_group_layouts: &[&frame_bgl],
            push_constant_ranges: &[],
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_pipeline"),
            layout: Some(&shadow_pl_layout),
            vertex: wgpu::VertexState {
                module: &shadow_mod,
                entry_point: Some("vs_main"),
                buffers: &[pbr_vb_layout.clone(), inst_vb_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 2. Sky pipeline
        let sky_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky_pl"),
            bind_group_layouts: &[&frame_bgl],
            push_constant_ranges: &[],
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_pipeline"),
            layout: Some(&sky_pl_layout),
            vertex: wgpu::VertexState {
                module: &sky_mod,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_mod,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 3. PBR pipeline
        let pbr_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pbr_pl"),
            bind_group_layouts: &[&frame_bgl, &shadow_sample_bgl],
            push_constant_ranges: &[],
        });
        let pbr_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pbr_pipeline"),
            layout: Some(&pbr_pl_layout),
            vertex: wgpu::VertexState {
                module: &pbr_mod,
                entry_point: Some("vs_main"),
                buffers: &[pbr_vb_layout, inst_vb_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &pbr_mod,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 4. Additive FX particles & beams pipeline
        let fx_vb_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FxVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 12, shader_location: 1 },
            ],
        };
        let fx_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fx_pipeline"),
            layout: Some(&sky_pl_layout),
            vertex: wgpu::VertexState {
                module: &fx_mod,
                entry_point: Some("vs_main"),
                buffers: &[fx_vb_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fx_mod,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 5. Bloom pipelines
        let bloom_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom_pl"),
            bind_group_layouts: &[&bloom_bgl],
            push_constant_ranges: &[],
        });
        let make_bloom_pipe = |entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&bloom_pl_layout),
                vertex: wgpu::VertexState {
                    module: &bloom_mod,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &bloom_mod,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let bright_pipeline = make_bloom_pipe("fs_bright");
        let blur_pipeline = make_bloom_pipe("fs_blur");

        // 6. Tonemap pipeline
        let tonemap_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tonemap_pl"),
            bind_group_layouts: &[&tonemap_bgl],
            push_constant_ranges: &[],
        });
        let tonemap_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tonemap_pipeline"),
            layout: Some(&tonemap_pl_layout),
            vertex: wgpu::VertexState {
                module: &tonemap_mod,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &tonemap_mod,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 7. 2D HUD overlay pipeline
        let hud_vb_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<HudVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 8, shader_location: 1 },
            ],
        };
        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud_pipeline"),
            layout: Some(&sky_pl_layout),
            vertex: wgpu::VertexState {
                module: &hud_mod,
                entry_point: Some("vs_main"),
                buffers: &[hud_vb_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_mod,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let targets = create_screen_targets(
            &device,
            &queue,
            width,
            height,
            &linear_sampler,
            &bloom_bgl,
            &tonemap_bgl,
            &bloom_bright_ub,
            &bloom_h_ub,
            &bloom_v_ub,
            &post_ub,
        );

        Self {
            device,
            queue,
            surface,
            surface_config,
            output_format,
            width,
            height,
            catalog_meshes,
            terrain_mesh: None,
            frame_ub,
            shadow_ub,
            post_ub,
            hud_ub,
            bloom_bright_ub,
            bloom_h_ub,
            bloom_v_ub,
            instance_vb,
            terrain_instance_vb,
            fx_vb,
            hud_vb,
            shadow_view,
            linear_sampler,
            frame_bg,
            shadow_pass_bg,
            shadow_sample_bg,
            hud_bg,
            bloom_bgl,
            tonemap_bgl,
            shadow_pipeline,
            sky_pipeline,
            pbr_pipeline,
            fx_pipeline,
            bright_pipeline,
            blur_pipeline,
            tonemap_pipeline,
            hud_pipeline,
            targets,
        }
    }

    pub fn set_terrain_mesh(&mut self, mesh: &MeshData) {
        self.terrain_mesh = Some(upload_mesh(&self.device, mesh));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        if let (Some(surface), Some(cfg)) = (&self.surface, &mut self.surface_config) {
            cfg.width = width;
            cfg.height = height;
            surface.configure(&self.device, cfg);
        }
        self.targets = create_screen_targets(
            &self.device,
            &self.queue,
            width,
            height,
            &self.linear_sampler,
            &self.bloom_bgl,
            &self.tonemap_bgl,
            &self.bloom_bright_ub,
            &self.bloom_h_ub,
            &self.bloom_v_ub,
            &self.post_ub,
        );
    }

    pub fn render_to_surface(&mut self, frame: &SceneFrame<'_>) -> Result<(), wgpu::SurfaceError> {
        let surface = match &self.surface {
            Some(s) => s,
            None => return Ok(()),
        };
        let output = surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.encode_and_submit(frame, &view);
        output.present();
        Ok(())
    }

    /// Offscreen render straight into a PNG file for automated visual validation & gallery capture!
    pub fn render_to_png(&mut self, frame: &SceneFrame<'_>, path: &Path) -> Result<(), String> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless_out"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.output_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.encode_and_submit(frame, &view);

        let bytes_per_pixel = 4u32;
        let unpadded = self.width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;

        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("png_readback"),
            size: (padded * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback_enc"),
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(enc.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        let _ = self.device.poll(wgpu::PollType::Wait);
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("map_async: {e:?}"))?;

        let data = slice.get_mapped_range();
        let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in 0..self.height as usize {
            let start = row * padded as usize;
            rgba.extend_from_slice(&data[start..start + unpadded as usize]);
        }
        drop(data);
        readback.unmap();

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(file, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn encode_and_submit(&mut self, frame: &SceneFrame<'_>, out_view: &wgpu::TextureView) {
        let aspect = self.width as f32 / self.height.max(1) as f32;
        let vp = frame.camera.view_proj(aspect);
        let inv_vp = vp.inverse();

        // Directional sun orthographic shadow frustum centered around player/focus
        let sun_dir = frame.biome.sun_dir.normalize();
        let focus = Vec3::new(frame.shadow_focus.x, frame.shadow_focus.y.max(8.0), frame.shadow_focus.z);
        let light_pos = focus + sun_dir * 165.0;
        let light_view = Mat4::look_at_rh(light_pos, focus, Vec3::Y);
        let light_proj = Mat4::orthographic_rh(-125.0, 125.0, -125.0, 125.0, 10.0, 360.0);
        let light_vp = light_proj * light_view;

        let mut gpu_lights = [PointLightGpu {
            pos_radius: [0.0; 4],
            color_intensity: [0.0; 4],
        }; 16];
        let light_count = frame.lights.len().min(16);
        for (i, l) in frame.lights.iter().take(light_count).enumerate() {
            gpu_lights[i] = PointLightGpu {
                pos_radius: [l.position.x, l.position.y, l.position.z, l.radius],
                color_intensity: [l.color.x, l.color.y, l.color.z, 1.0],
            };
        }

        let b = frame.biome;
        let frame_u = FrameUniformGpu {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: inv_vp.to_cols_array_2d(),
            light_view_proj: light_vp.to_cols_array_2d(),
            camera_pos: [frame.camera.eye.x, frame.camera.eye.y, frame.camera.eye.z, 1.0],
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, 0.0],
            sun_color: [b.sun_color.x, b.sun_color.y, b.sun_color.z, 1.0],
            sky_zenith: [b.sky_zenith.x, b.sky_zenith.y, b.sky_zenith.z, 1.0],
            sky_horizon: [b.sky_horizon.x, b.sky_horizon.y, b.sky_horizon.z, 1.0],
            ground_bounce: [b.ground_bounce.x, b.ground_bounce.y, b.ground_bounce.z, 1.0],
            fog_params: [b.fog_color.x, b.fog_color.y, b.fog_color.z, b.fog_density],
            planet_color: [b.planet_color.x, b.planet_color.y, b.planet_color.z, b.aurora_strength],
            ring_color: [b.ring_color.x, b.ring_color.y, b.ring_color.z, frame.time],
            post_params: [frame.boost_fx, frame.damage_fx, light_count as f32, aspect],
            lights: gpu_lights,
        };
        self.queue.write_buffer(&self.frame_ub, 0, bytemuck::bytes_of(&frame_u));

        let shadow_u = ShadowUniformGpu {
            light_view_proj: light_vp.to_cols_array_2d(),
        };
        self.queue.write_buffer(&self.shadow_ub, 0, bytemuck::bytes_of(&shadow_u));

        let post_u = PostUniformGpu {
            boost_fx: frame.boost_fx,
            damage_fx: frame.damage_fx,
            time: frame.time,
            exposure: 1.08,
        };
        self.queue.write_buffer(&self.post_ub, 0, bytemuck::bytes_of(&post_u));

        let hud_u = HudUniformGpu {
            screen_size: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        self.queue.write_buffer(&self.hud_ub, 0, bytemuck::bytes_of(&hud_u));

        // Pack all instanced draws contiguously
        let mut flat_instances = Vec::with_capacity(512);
        let mut draw_ranges: Vec<(usize, u32, u32)> = Vec::with_capacity(MeshId::COUNT);
        for (mesh_idx, list) in frame.instances.iter().enumerate().take(self.catalog_meshes.len()) {
            if list.is_empty() {
                continue;
            }
            let start = flat_instances.len() as u32;
            let remaining = MAX_INSTANCES.saturating_sub(flat_instances.len());
            let count = list.len().min(remaining);
            if count == 0 {
                break;
            }
            flat_instances.extend_from_slice(&list[..count]);
            draw_ranges.push((mesh_idx, start, start + count as u32));
        }
        if !flat_instances.is_empty() {
            self.queue.write_buffer(&self.instance_vb, 0, bytemuck::cast_slice(&flat_instances));
        }

        let fx_count = frame.fx_vertices.len().min(MAX_FX_VERTS);
        if fx_count > 0 {
            self.queue
                .write_buffer(&self.fx_vb, 0, bytemuck::cast_slice(&frame.fx_vertices[..fx_count]));
        }

        let hud_count = frame.hud.vertices.len().min(MAX_HUD_VERTS);
        if hud_count > 0 {
            self.queue
                .write_buffer(&self.hud_vb, 0, bytemuck::cast_slice(&frame.hud.vertices[..hud_count]));
        }

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("exo_frame_enc"),
        });

        // --- PASS 1: DIRECTIONAL SHADOW MAP ---
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.shadow_pipeline);
            rp.set_bind_group(0, &self.shadow_pass_bg, &[]);
            if let Some(tm) = &self.terrain_mesh {
                rp.set_vertex_buffer(0, tm.vb.slice(..));
                rp.set_vertex_buffer(1, self.terrain_instance_vb.slice(..));
                rp.set_index_buffer(tm.ib.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..tm.index_count, 0, 0..1);
            }
            if !draw_ranges.is_empty() {
                rp.set_vertex_buffer(1, self.instance_vb.slice(..));
                for &(mesh_idx, start, end) in &draw_ranges {
                    // Skip thruster plumes & shock rings from casting opaque shadows
                    if mesh_idx == MeshId::ThrusterPlume as usize
                        || mesh_idx == MeshId::GlowSphere as usize
                        || mesh_idx == MeshId::ShockRing as usize
                    {
                        continue;
                    }
                    let gm = &self.catalog_meshes[mesh_idx];
                    rp.set_vertex_buffer(0, gm.vb.slice(..));
                    rp.set_index_buffer(gm.ib.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(0..gm.index_count, 0, start..end);
                }
            }
        }

        // --- PASS 2: HDR SKY + PBR SCENE + ADDITIVE FX ---
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hdr_scene_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 2a. Atmospheric Planetary Sky Dome
            rp.set_pipeline(&self.sky_pipeline);
            rp.set_bind_group(0, &self.frame_bg, &[]);
            rp.draw(0..3, 0..1);

            // 2b. PBR Terrain & Instanced Models
            rp.set_pipeline(&self.pbr_pipeline);
            rp.set_bind_group(0, &self.frame_bg, &[]);
            rp.set_bind_group(1, &self.shadow_sample_bg, &[]);
            if let Some(tm) = &self.terrain_mesh {
                rp.set_vertex_buffer(0, tm.vb.slice(..));
                rp.set_vertex_buffer(1, self.terrain_instance_vb.slice(..));
                rp.set_index_buffer(tm.ib.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..tm.index_count, 0, 0..1);
            }
            if !draw_ranges.is_empty() {
                rp.set_vertex_buffer(1, self.instance_vb.slice(..));
                for &(mesh_idx, start, end) in &draw_ranges {
                    let gm = &self.catalog_meshes[mesh_idx];
                    rp.set_vertex_buffer(0, gm.vb.slice(..));
                    rp.set_index_buffer(gm.ib.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(0..gm.index_count, 0, start..end);
                }
            }

            // 2c. Additive HDR Energy Beams & Particles
            if fx_count > 0 {
                rp.set_pipeline(&self.fx_pipeline);
                rp.set_bind_group(0, &self.frame_bg, &[]);
                rp.set_vertex_buffer(0, self.fx_vb.slice(..));
                rp.draw(0..fx_count as u32, 0..1);
            }
        }

        // --- PASS 3: MULTI-PASS HDR BLOOM (BRIGHT -> BLUR H -> BLUR V) ---
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom_bright"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.bloom_a_view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.bright_pipeline);
            rp.set_bind_group(0, &self.targets.bright_bg, &[]);
            rp.draw(0..3, 0..1);
        }
        for _ in 0..2 {
            {
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bloom_blur_h"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets.bloom_b_view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&self.blur_pipeline);
                rp.set_bind_group(0, &self.targets.blur_h_bg, &[]);
                rp.draw(0..3, 0..1);
            }
            {
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bloom_blur_v"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets.bloom_a_view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&self.blur_pipeline);
                rp.set_bind_group(0, &self.targets.blur_v_bg, &[]);
                rp.draw(0..3, 0..1);
            }
        }

        // --- PASS 4: ACES FILMIC TONEMAP + 2D SCI-FI HUD OVERLAY ---
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tonemap_and_hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: out_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.tonemap_pipeline);
            rp.set_bind_group(0, &self.targets.tonemap_bg, &[]);
            rp.draw(0..3, 0..1);

            if hud_count > 0 {
                rp.set_pipeline(&self.hud_pipeline);
                rp.set_bind_group(0, &self.hud_bg, &[]);
                rp.set_vertex_buffer(0, self.hud_vb.slice(..));
                rp.draw(0..hud_count as u32, 0..1);
            }
        }

        self.queue.submit(Some(enc.finish()));
    }
}

fn upload_mesh(device: &wgpu::Device, mesh: &MeshData) -> GpuMesh {
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mesh_vb"),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mesh_ib"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    GpuMesh {
        vb,
        ib,
        index_count: mesh.indices.len() as u32,
    }
}

#[allow(clippy::too_many_arguments)]
fn create_screen_targets(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    linear_sampler: &wgpu::Sampler,
    bloom_bgl: &wgpu::BindGroupLayout,
    tonemap_bgl: &wgpu::BindGroupLayout,
    bloom_bright_ub: &wgpu::Buffer,
    bloom_h_ub: &wgpu::Buffer,
    bloom_v_ub: &wgpu::Buffer,
    post_ub: &wgpu::Buffer,
) -> ScreenTargets {
    let hdr_format = wgpu::TextureFormat::Rgba16Float;
    let hdr_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr_color"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: hdr_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let hdr_view = hdr_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene_depth"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let bloom_w = (width / 2).max(1);
    let bloom_h = (height / 2).max(1);
    let make_bloom_tex = |label: &str| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: bloom_w, height: bloom_h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: hdr_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let bloom_a_view = make_bloom_tex("bloom_a");
    let bloom_b_view = make_bloom_tex("bloom_b");

    queue.write_buffer(
        bloom_bright_ub,
        0,
        bytemuck::bytes_of(&BloomParamsGpu { dir: [0.0, 0.0], threshold: 1.05, _pad: 0.0 }),
    );
    queue.write_buffer(
        bloom_h_ub,
        0,
        bytemuck::bytes_of(&BloomParamsGpu { dir: [1.0 / bloom_w as f32, 0.0], threshold: 0.0, _pad: 0.0 }),
    );
    queue.write_buffer(
        bloom_v_ub,
        0,
        bytemuck::bytes_of(&BloomParamsGpu { dir: [0.0, 1.0 / bloom_h as f32], threshold: 0.0, _pad: 0.0 }),
    );

    let bright_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bright_bg"),
        layout: bloom_bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hdr_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(linear_sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: bloom_bright_ub.as_entire_binding() },
        ],
    });
    let blur_h_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blur_h_bg"),
        layout: bloom_bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&bloom_a_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(linear_sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: bloom_h_ub.as_entire_binding() },
        ],
    });
    let blur_v_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blur_v_bg"),
        layout: bloom_bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&bloom_b_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(linear_sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: bloom_v_ub.as_entire_binding() },
        ],
    });
    let tonemap_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tonemap_bg"),
        layout: tonemap_bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hdr_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&bloom_a_view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(linear_sampler) },
            wgpu::BindGroupEntry { binding: 3, resource: post_ub.as_entire_binding() },
        ],
    });

    ScreenTargets {
        hdr_view,
        depth_view,
        bloom_a_view,
        bloom_b_view,
        bright_bg,
        blur_h_bg,
        blur_v_bg,
        tonemap_bg,
    }
}

/// Helper to push a glowing 3D billboard quad or beam ribbon into `fx_vertices`.
pub fn push_fx_billboard(out: &mut Vec<FxVertex>, center: Vec3, cam_right: Vec3, cam_up: Vec3, radius: f32, color: Vec4) {
    let r = cam_right * radius;
    let u = cam_up * radius;
    let p0 = center - r - u;
    let p1 = center + r - u;
    let p2 = center + r + u;
    let p3 = center - r + u;
    let col = color.to_array();
    out.push(FxVertex { position: p0.to_array(), color: col });
    out.push(FxVertex { position: p1.to_array(), color: col });
    out.push(FxVertex { position: p2.to_array(), color: col });
    out.push(FxVertex { position: p0.to_array(), color: col });
    out.push(FxVertex { position: p2.to_array(), color: col });
    out.push(FxVertex { position: p3.to_array(), color: col });
}

pub fn push_fx_beam(out: &mut Vec<FxVertex>, a: Vec3, b: Vec3, cam_eye: Vec3, width: f32, color_a: Vec4, color_b: Vec4) {
    let dir = (b - a).normalize_or_zero();
    let to_cam = (cam_eye - (a + b) * 0.5).normalize_or_zero();
    let side = dir.cross(to_cam).normalize_or_zero() * (width * 0.5);
    let ca = color_a.to_array();
    let cb = color_b.to_array();
    let p0 = (a - side).to_array();
    let p1 = (a + side).to_array();
    let p2 = (b + side).to_array();
    let p3 = (b - side).to_array();
    out.push(FxVertex { position: p0, color: ca });
    out.push(FxVertex { position: p1, color: ca });
    out.push(FxVertex { position: p2, color: cb });
    out.push(FxVertex { position: p0, color: ca });
    out.push(FxVertex { position: p2, color: cb });
    out.push(FxVertex { position: p3, color: cb });
}
