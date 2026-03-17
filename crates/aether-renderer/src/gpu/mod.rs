pub mod context;
pub mod material;
pub mod mesh;
pub mod pass;
pub mod pipeline;
pub mod shader;
pub mod shadow;

use context::{GpuContext, GpuError};
use material::{MaterialId, MaterialManager};
use mesh::{MeshId, MeshManager, Vertex};
use pass::{CameraUniforms, DrawCommand, FrameResources, ModelUniforms};
use pipeline::{PipelineLayouts, ShadowPipelineLayouts};
use shadow::{LightUniforms, ShadowPass};
use texture::TextureManager;

pub mod texture;

/// Maximum number of preallocated model uniform slots.
const DEFAULT_MAX_OBJECTS: usize = 256;

/// The top-level GPU renderer that ties together all GPU subsystems.
pub struct GpuRenderer {
    pub ctx: GpuContext,
    pub mesh_manager: MeshManager,
    pub texture_manager: TextureManager,
    pub material_manager: MaterialManager,
    pub forward_pipeline: wgpu::RenderPipeline,
    pub shadow_pipeline: wgpu::RenderPipeline,
    pub pipeline_layouts: PipelineLayouts,
    pub shadow_layouts: ShadowPipelineLayouts,
    pub shadow_pass: ShadowPass,
    pub frame_resources: FrameResources,
    pub light_bind_group: wgpu::BindGroup,
    pub msaa_texture: Option<(wgpu::Texture, wgpu::TextureView)>,
    pub depth_texture: (wgpu::Texture, wgpu::TextureView),
}

impl GpuRenderer {
    /// Create a new headless GPU renderer (no surface).
    pub async fn new_headless() -> Result<Self, GpuError> {
        let ctx = GpuContext::new_headless().await?;
        Self::from_context(ctx)
    }

    /// Create a GPU renderer with a surface.
    ///
    /// The `instance` must be the same one used to create the `surface`.
    pub async fn new_with_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let ctx = GpuContext::new_with_surface(instance, surface, width, height).await?;
        Self::from_context(ctx)
    }

    /// Build the renderer from an existing GpuContext.
    fn from_context(ctx: GpuContext) -> Result<Self, GpuError> {
        let device = &ctx.device;

        let pipeline_layouts = PipelineLayouts::new(device);
        let shadow_layouts = ShadowPipelineLayouts::new(device);

        let forward_pipeline = pipeline::create_forward_pipeline(
            device,
            &pipeline_layouts,
            ctx.surface_format,
            ctx.depth_format,
            ctx.msaa_samples,
        );

        let shadow_pipeline = pipeline::create_shadow_pipeline(
            device,
            &shadow_layouts,
            ShadowPass::depth_format(),
        );

        let shadow_pass = ShadowPass::new(device, &shadow_layouts.light_vp_layout);

        let frame_resources = FrameResources::new(
            device,
            &pipeline_layouts.camera_layout,
            &pipeline_layouts.model_layout,
            DEFAULT_MAX_OBJECTS,
        );

        let light_bind_group = pass::create_light_bind_group(
            device,
            &pipeline_layouts.light_layout,
            &shadow_pass.light_uniform_buffer,
            &shadow_pass.depth_view,
            &shadow_pass.comparison_sampler,
        );

        let (w, h) = ctx.surface_size();

        let msaa_texture = if ctx.msaa_samples > 1 {
            Some(texture::create_msaa_texture(
                device,
                w,
                h,
                ctx.msaa_samples,
                ctx.surface_format,
            ))
        } else {
            None
        };

        let depth_texture = texture::create_depth_texture(
            device,
            w,
            h,
            ctx.msaa_samples,
            ctx.depth_format,
        );

        Ok(Self {
            ctx,
            mesh_manager: MeshManager::new(),
            texture_manager: TextureManager::new(),
            material_manager: MaterialManager::new(),
            forward_pipeline,
            shadow_pipeline,
            pipeline_layouts,
            shadow_layouts,
            shadow_pass,
            frame_resources,
            light_bind_group,
            msaa_texture,
            depth_texture,
        })
    }

    /// Resize render targets when the surface size changes.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.ctx.resize(width, height);

        let device = &self.ctx.device;

        self.depth_texture = texture::create_depth_texture(
            device,
            width,
            height,
            self.ctx.msaa_samples,
            self.ctx.depth_format,
        );

        if self.ctx.msaa_samples > 1 {
            self.msaa_texture = Some(texture::create_msaa_texture(
                device,
                width,
                height,
                self.ctx.msaa_samples,
                self.ctx.surface_format,
            ));
        }
    }

    /// Update camera uniforms for the current frame.
    pub fn update_camera(&self, uniforms: &CameraUniforms) {
        self.frame_resources.update_camera(&self.ctx.queue, uniforms);
    }

    /// Update a model's transform uniforms.
    pub fn update_model(&self, index: usize, uniforms: &ModelUniforms) {
        self.frame_resources.update_model(&self.ctx.queue, index, uniforms);
    }

    /// Update light and shadow uniforms for the current frame.
    pub fn update_light(&self, uniforms: &LightUniforms) {
        self.shadow_pass.update_light_uniforms(&self.ctx.queue, uniforms);
    }

    /// Render a frame with the given draw commands.
    ///
    /// Returns `Ok(())` if the frame was submitted, or an error if the
    /// surface texture could not be acquired.
    pub fn render(&self, draw_commands: &[DrawCommand]) -> Result<(), GpuError> {
        // Acquire surface texture if we have a surface
        let surface_texture = if let Some(surface) = &self.ctx.surface {
            match surface.get_current_texture() {
                Ok(tex) => Some(tex),
                Err(e) => {
                    return Err(GpuError::SurfaceConfigFailed(format!(
                        "failed to acquire surface texture: {e}"
                    )));
                }
            }
        } else {
            None
        };

        let target_view = surface_texture.as_ref().map(|tex| {
            tex.texture
                .create_view(&wgpu::TextureViewDescriptor::default())
        });

        let mut encoder =
            self.ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("aether-frame-encoder"),
                });

        // --- Shadow passes ---
        for cascade in 0..ShadowPass::num_cascades() as usize {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("shadow-pass-cascade-{cascade}")),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_pass.cascade_views[cascade],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            shadow_pass.set_pipeline(&self.shadow_pipeline);
            shadow_pass.set_bind_group(0, &self.shadow_pass.cascade_bind_groups[cascade], &[]);

            for cmd in draw_commands {
                if let Some(gpu_mesh) = self.mesh_manager.get(cmd.mesh_id) {
                    if cmd.model_bind_group_index < self.frame_resources.model_bind_groups.len() {
                        shadow_pass.set_bind_group(
                            1,
                            &self.frame_resources.model_bind_groups[cmd.model_bind_group_index],
                            &[],
                        );
                        shadow_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                        shadow_pass.set_index_buffer(
                            gpu_mesh.index_buffer.slice(..),
                            gpu_mesh.index_format,
                        );
                        shadow_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..cmd.instance_count);
                    }
                }
            }
        }

        // --- Forward pass ---
        if let Some(target) = &target_view {
            let color_attachment = if let Some((_, msaa_view)) = &self.msaa_texture {
                wgpu::RenderPassColorAttachment {
                    view: msaa_view,
                    resolve_target: Some(target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                }
            } else {
                wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                }
            };

            let mut forward_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("forward-pass"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.1,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            forward_pass.set_pipeline(&self.forward_pipeline);
            forward_pass.set_bind_group(0, &self.frame_resources.camera_bind_group, &[]);
            forward_pass.set_bind_group(3, &self.light_bind_group, &[]);

            for cmd in draw_commands {
                if let Some(gpu_mesh) = self.mesh_manager.get(cmd.mesh_id) {
                    if let Some(gpu_mat) = self.material_manager.get(cmd.material_id) {
                        if cmd.model_bind_group_index
                            < self.frame_resources.model_bind_groups.len()
                        {
                            forward_pass.set_bind_group(
                                1,
                                &self.frame_resources.model_bind_groups
                                    [cmd.model_bind_group_index],
                                &[],
                            );
                            forward_pass.set_bind_group(2, &gpu_mat.bind_group, &[]);
                            forward_pass
                                .set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                            forward_pass.set_index_buffer(
                                gpu_mesh.index_buffer.slice(..),
                                gpu_mesh.index_format,
                            );
                            forward_pass.draw_indexed(
                                0..gpu_mesh.index_count,
                                0,
                                0..cmd.instance_count,
                            );
                        }
                    }
                }
            }
        }

        self.ctx.queue.submit(std::iter::once(encoder.finish()));

        if let Some(tex) = surface_texture {
            tex.present();
        }

        Ok(())
    }

    /// Upload a mesh and return its ID.
    pub fn upload_mesh(&mut self, vertices: &[Vertex], indices: &[u32]) -> MeshId {
        self.mesh_manager
            .upload(&self.ctx.device, &self.ctx.queue, vertices, indices)
    }

    /// Upload a material and return its ID.
    pub fn upload_material(
        &mut self,
        material: material::PbrMaterial,
    ) -> MaterialId {
        // Ensure default white texture exists before borrowing anything
        self.ensure_default_white_texture();

        // Determine which texture to use for albedo
        let tex_id = material
            .albedo_texture
            .filter(|id| self.texture_manager.get(*id).is_some())
            .unwrap_or(texture::TextureId(1));

        let gpu_tex = self
            .texture_manager
            .get(tex_id)
            .expect("texture must exist");
        let albedo_view = &gpu_tex.view as *const wgpu::TextureView;
        let albedo_sampler = &gpu_tex.sampler as *const wgpu::Sampler;

        // SAFETY: The texture manager owns these resources and they remain valid
        // for the duration of this method call. We use raw pointers to break
        // the borrow on `self` so we can call into material_manager.
        self.material_manager.upload(
            &self.ctx.device,
            &self.ctx.queue,
            &self.pipeline_layouts.material_layout,
            material,
            unsafe { &*albedo_view },
            unsafe { &*albedo_sampler },
        )
    }

    /// Ensure the default white texture exists.
    fn ensure_default_white_texture(&mut self) {
        if self.texture_manager.get(texture::TextureId(1)).is_none() {
            self.texture_manager
                .create_default_white(&self.ctx.device, &self.ctx.queue);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_max_objects_is_reasonable() {
        assert_eq!(DEFAULT_MAX_OBJECTS, 256);
    }

    #[test]
    fn draw_command_can_be_cloned() {
        let cmd = DrawCommand {
            mesh_id: MeshId(1),
            material_id: MaterialId(2),
            model_bind_group_index: 0,
            instance_count: 1,
        };
        let cloned = cmd.clone();
        assert_eq!(cloned.mesh_id, MeshId(1));
    }
}
