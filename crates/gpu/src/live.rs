//! The live transform's GPU side: the moving layer's pixels drawn through a mesh
//! ([`Kernel::Mesh`], `live.wgsl`) in the layer's place in the stack, so a transform previews with
//! the layer's blend mode, opacity, fill and clipping (`photocraft_compose::live`).

use super::convert::{W::F, f32_to_f16, words};
use std::num::NonZeroU64;
use std::sync::Arc;

use photocraft_doc::DocId;
use photocraft_geom::Rect;
use photocraft_raster::Surface;

use super::{Compositor, Unsupported};

/// The live transform's mesh pass (`live.wgsl`), exposed for validation in tests.
pub const LIVE_SHADER: &str = include_str!("live.wgsl");
/// Bytes of the mesh pass's uniform (`Live` in live.wgsl).
const LIVE_UNIFORM: u64 = 32;

/// Pipeline and layout of [`super::Kernel::Mesh`].
#[derive(Clone)]
pub(crate) struct MeshKit {
    pub(crate) pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Picks the nearest texel (Preferences › Interpolation: Nearest).
    nearest: wgpu::Sampler,
}

/// A live transform's source texture and mesh on the GPU, rebuilt when either changes.
pub(crate) struct LiveGpu {
    /// What they were built from (identity: the `Arc`s are replaced, never mutated).
    source: Arc<Surface>,
    mesh: Arc<photocraft_compose::live::LiveMesh>,
    nearest: bool,
    _tex: wgpu::Texture,
    pub(crate) group: wgpu::BindGroup,
    pub(crate) vertices: Option<wgpu::Buffer>,
    pub(crate) count: u32,
}

/// Rows of a live transform's source read per band: a band's floats stay small whatever the
/// layer's size.
const LIVE_BAND: u32 = 64;

/// `region` of `source` as premultiplied RGBA16F texels for the mesh pass, converted band by band
/// (in parallel on native targets) so no full-size float copy of a large layer is made.
pub(crate) fn live_texels(source: &Surface, region: Rect) -> Result<Vec<u8>, Unsupported> {
    let fmt = source.format();
    let n = fmt.channels().max(1);
    let row_bytes = region.width() as usize * 8;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(row_bytes.saturating_mul(region.height() as usize))
        .map_err(|_| Unsupported("not enough memory for the live transform source".into()))?;
    bytes.resize(row_bytes * region.height() as usize, 0);
    // The CMYK space the encode set is thread-local: the bands take it along.
    let space = photocraft_color::convert::active_cmyk_space();
    let band = |i: usize, out: &mut [u8]| {
        let y0 = region.y0.saturating_add((i as u32 * LIVE_BAND) as i32);
        let rows = (out.len() / row_bytes.max(1)) as i32;
        let r = Rect::new(region.x0, y0, region.x1, y0.saturating_add(rows));
        photocraft_color::convert::with_cmyk_space(space.as_ref(), || {
            let raw = source.read_region(r);
            for (px, o) in raw.chunks_exact(n).zip(out.as_chunks_mut::<8>().0) {
                let [red, g, b, a] = photocraft_raster::to_rgba(&fmt, px);
                let a = a.clamp(0.0, 1.0);
                for (k, v) in [red * a, g * a, b * a, a].into_iter().enumerate() {
                    o[k * 2..k * 2 + 2].copy_from_slice(&f32_to_f16(v).to_le_bytes());
                }
            }
        });
    };
    let chunk = row_bytes * LIVE_BAND as usize;
    if chunk == 0 {
        return Ok(bytes);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        bytes.par_chunks_mut(chunk).enumerate().for_each(|(i, out)| band(i, out));
    }
    #[cfg(target_arch = "wasm32")]
    for (i, out) in bytes.chunks_mut(chunk).enumerate() {
        band(i, out);
    }
    Ok(bytes)
}

/// The mesh pass's pipeline (`live.wgsl`): chunk uniforms in group 0 (as every pass), the source
/// texture, its bilinear sampler and the `Live` uniform in group 1.
pub(crate) fn mesh_kit(device: &wgpu::Device, bgl0: &wgpu::BindGroupLayout, format: wgpu::TextureFormat) -> Option<MeshKit> {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pc_live"), source: wgpu::ShaderSource::Wgsl(LIVE_SHADER.into()) });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pc_live"),
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
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(LIVE_UNIFORM),
                },
                count: None,
            },
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pc_live"),
        bind_group_layouts: &[Some(bgl0), Some(&bgl)],
        immediate_size: 0,
    });
    let attributes = [
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 8, shader_location: 1 },
    ];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pc_live_mesh"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_mesh"),
            buffers: &[Some(wgpu::VertexBufferLayout { array_stride: 16, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_mesh"),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("pc_live"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("pc_live_nearest"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    Some(MeshKit { pipeline, bgl, sampler, nearest })
}

impl Compositor {
    /// Show `live` moving a layer of document `doc` (`photocraft_compose::live`): renders of that
    /// document then draw the layer's pixels through the mesh, composited in its place with its
    /// blend mode, opacity, fill, clipping and the layers above it. The document passed to
    /// [`Self::render`] must be [`photocraft_compose::live::prepare`]d for it. `None` ends it.
    pub fn set_live(&mut self, live: Option<(DocId, photocraft_compose::live::LiveTransform)>) {
        if live.is_none() {
            self.live_gpu = None;
        }
        self.live = live;
    }

    /// The live transform set for `doc`, if any.
    pub fn live(&self, doc: DocId) -> Option<&photocraft_compose::live::LiveTransform> {
        self.live.as_ref().filter(|(d, _)| *d == doc).map(|(_, l)| l)
    }

    /// Upload `live`'s source and mesh where they changed.
    pub(crate) fn sync_live(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, live: &photocraft_compose::live::LiveTransform) -> Result<(), Unsupported> {
        let kit = self.kit.mesh.clone().ok_or_else(|| Unsupported("no live transform pipeline on this device".into()))?;
        let same_source = self.live_gpu.as_ref().is_some_and(|g| Arc::ptr_eq(&g.source, &live.source) && g.nearest == live.nearest);
        if !same_source {
            self.live_gpu = None;
            // The texels the mesh can read, with a transparent texel around them.
            let region = live.source.content_bounds().inflate(1);
            let region = if region.width() == 0 || region.height() == 0 { Rect::new(0, 0, 1, 1) } else { region };
            if region.width() > self.max_dim || region.height() > self.max_dim {
                return Err(Unsupported(format!("live transform source of {}x{} px is over the texture limit", region.width(), region.height())));
            }
            let bytes = live_texels(&live.source, region)?;
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pc_live_source"),
                size: wgpu::Extent3d { width: region.width(), height: region.height(), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(region.width() * 8), rows_per_image: Some(region.height()) },
                wgpu::Extent3d { width: region.width(), height: region.height(), depth_or_array_layers: 1 },
            );
            let view = tex.create_view(&Default::default());
            let uniform = words(&[
                F(self.chunk as f32),
                F(self.chunk as f32),
                F(region.x0 as f32),
                F(region.y0 as f32),
                F(region.width() as f32),
                F(region.height() as f32),
                F(0.0),
                F(0.0),
            ]);
            let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pc_live_uniform"),
                size: LIVE_UNIFORM,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&ubuf, 0, &uniform);
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pc_live"),
                layout: &kit.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(if live.nearest { &kit.nearest } else { &kit.sampler }) },
                    wgpu::BindGroupEntry { binding: 2, resource: ubuf.as_entire_binding() },
                ],
            });
            self.live_gpu = Some(LiveGpu {
                source: live.source.clone(),
                mesh: Arc::new(Default::default()),
                nearest: live.nearest,
                _tex: tex,
                group,
                vertices: None,
                count: 0,
            });
        }
        let Some(g) = self.live_gpu.as_mut() else { return Ok(()) };
        if !Arc::ptr_eq(&g.mesh, &live.mesh) {
            let mut v: Vec<f32> = Vec::new();
            for tri in live.mesh.triangles() {
                for (p, uv) in tri {
                    v.extend_from_slice(&[p[0], p[1], uv[0], uv[1]]);
                }
            }
            let bytes: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
            g.count = (v.len() / 4) as u32;
            g.vertices = (!bytes.is_empty()).then(|| {
                let b = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("pc_live_mesh"),
                    size: bytes.len() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                queue.write_buffer(&b, 0, &bytes);
                b
            });
            g.mesh = live.mesh.clone();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{ACC_FORMAT, CHUNK_UNIFORM, OP_UNIFORM};
    use crate::kit::uniform_entry;
    use crate::paging::first_error;

    #[test]
    fn live_shader_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(LIVE_SHADER).unwrap_or_else(|e| panic!("{}", e.emit_to_string(LIVE_SHADER)));
        wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
        for e in ["vs_mesh", "fs_mesh"] {
            assert!(module.entry_points.iter().any(|ep| ep.name == e), "missing entry point {e}");
        }
    }

    /// The mesh pass's pipeline builds on a real device (none: skipped).
    #[test]
    fn live_pipeline_builds() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else { return };
        let Ok((device, _)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) else { return };
        let bgl0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[uniform_entry(0, CHUNK_UNIFORM), uniform_entry(1, OP_UNIFORM)],
        });
        let (kit, err) = first_error(&device, || mesh_kit(&device, &bgl0, ACC_FORMAT));
        assert!(err.is_none(), "{err:?}");
        assert!(kit.is_some());
    }
}
