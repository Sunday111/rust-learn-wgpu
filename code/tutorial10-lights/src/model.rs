use std::{
    collections::HashMap,
    io::{BufReader, Cursor},
    ops::Range,
    path::{Path, PathBuf},
};

use klgl::file_loader::FileDataHandle;
use tutorial_embedded_content::ILLUMINATI_PNG;
use wgpu::util::DeviceExt;

fn get_value_from_map<'map, Key, Value, Hasher, Query>(
    map: &'map HashMap<Key, Value, Hasher>,
    key: &Query,
) -> anyhow::Result<&'map Value>
where
    Key: Eq + std::hash::Hash,
    Hasher: std::hash::BuildHasher,
    Key: std::borrow::Borrow<Query>,
    Query: ?Sized + std::hash::Hash + std::cmp::Eq + std::fmt::Debug,
{
    map.get(key)
        .ok_or_else(|| anyhow::anyhow!("Could not find '{:?}' in the map", key))
}

fn to_posix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub trait Vertex {
    fn layout() -> wgpu::VertexBufferLayout<'static>;
}

#[allow(dead_code)]
pub struct Material {
    pub name: String,
    pub diffuse_texture: klgl::Texture,
    pub bind_group: wgpu::BindGroup,
}

#[allow(dead_code)]
pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
}

impl Vertex for ModelVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

impl Mesh {
    #[allow(dead_code)]
    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        camera_bind_group: &wgpu::BindGroup,
        material: &Material,
    ) {
        self.draw_instanced(render_pass, camera_bind_group, material, 0..1);
    }

    pub fn draw_instanced(
        &self,
        render_pass: &mut wgpu::RenderPass,
        camera_bind_group: &wgpu::BindGroup,
        material: &Material,
        instances: Range<u32>,
    ) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(0, &material.bind_group, &[]);
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        render_pass.draw_indexed(0..self.num_elements, 0, instances);
    }
}

impl Model {
    pub fn draw_instanced(
        &self,
        render_pass: &mut wgpu::RenderPass,
        camera_bind_group: &wgpu::BindGroup,
        instances: Range<u32>,
    ) {
        for mesh in &self.meshes {
            let material = &self.materials[mesh.material];
            mesh.draw_instanced(render_pass, camera_bind_group, material, instances.clone());
        }
    }

    pub fn load(
        obj_file_name: &str,
        file_map: &HashMap<String, FileDataHandle>,
        ctx: &klgl::RenderContext,
        layout: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<Model> {
        let obj_file_handle = get_value_from_map(file_map, obj_file_name)?;
        let obj_cursor = Cursor::new(&obj_file_handle.data);
        let mut obj_reader = BufReader::new(obj_cursor);

        let root_path = PathBuf::from({
            match obj_file_name.rfind('/') {
                Some(i) => String::from(&obj_file_name[0..i + 1]),
                None => String::new(),
            }
        });

        let (models, obj_materials) = tobj::load_obj_buf(
            &mut obj_reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |p| {
                let file_path = root_path.join(p);
                let file_path_str = to_posix_path(&file_path);
                match get_value_from_map(file_map, &file_path_str) {
                    Ok(file_data) => {
                        tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(&file_data.data)))
                    }
                    Err(err) => {
                        log::error!(
                            "Failed to find a file {} required for model {}. It was expected that is was already preloaded by this point. Error: {}",
                            file_path_str,
                            obj_file_name,
                            err
                        );
                        Err(tobj::LoadError::OpenFileFailed)
                    }
                }
            },
        )?;

        let mut materials = Vec::new();
        for m in obj_materials? {
            let diffuse_texture = {
                match &m.diffuse_texture {
                    Some(diffuse_texture_path) => {
                        let diffuse_texture_path = root_path.join(&diffuse_texture_path);
                        let diffuse_texture_path_str = to_posix_path(&diffuse_texture_path);
                        let diffuse_texture_file_handle =
                            get_value_from_map(file_map, &diffuse_texture_path_str)?;
                        klgl::Texture::from_bytes(
                            &ctx.device,
                            &ctx.queue,
                            &diffuse_texture_file_handle.data,
                            &diffuse_texture_path_str,
                        )?
                    }
                    None => {
                        log::warn!(
                            "obj file {} has a material {} without diffuse texture. Using placeholder",
                            obj_file_name,
                            m.name
                        );
                        klgl::Texture::from_bytes(
                            &ctx.device,
                            &ctx.queue,
                            &ILLUMINATI_PNG,
                            &"PLACEHOLDER",
                        )?
                    }
                }
            };
            let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                ],
                label: None,
            });

            materials.push(Material {
                name: m.name,
                diffuse_texture,
                bind_group,
            })
        }

        let meshes = models
            .into_iter()
            .map(|m| {
                let vertices = (0..m.mesh.positions.len() / 3)
                    .map(|i| {
                        if m.mesh.normals.is_empty() {
                            ModelVertex {
                                position: [
                                    m.mesh.positions[i * 3],
                                    m.mesh.positions[i * 3 + 1],
                                    m.mesh.positions[i * 3 + 2],
                                ],
                                tex_coords: [
                                    m.mesh.texcoords[i * 2],
                                    1.0 - m.mesh.texcoords[i * 2 + 1],
                                ],
                                normal: [0.0, 0.0, 0.0],
                            }
                        } else {
                            ModelVertex {
                                position: [
                                    m.mesh.positions[i * 3],
                                    m.mesh.positions[i * 3 + 1],
                                    m.mesh.positions[i * 3 + 2],
                                ],
                                tex_coords: [
                                    m.mesh.texcoords[i * 2],
                                    1.0 - m.mesh.texcoords[i * 2 + 1],
                                ],
                                normal: [
                                    m.mesh.normals[i * 3],
                                    m.mesh.normals[i * 3 + 1],
                                    m.mesh.normals[i * 3 + 2],
                                ],
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                let vertex_buffer =
                    ctx.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{:?} Vertex Buffer", obj_file_name)),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let index_buffer =
                    ctx.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{:?} Index Buffer", obj_file_name)),
                            contents: bytemuck::cast_slice(&m.mesh.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });

                Mesh {
                    name: obj_file_name.to_string(),
                    vertex_buffer,
                    index_buffer,
                    num_elements: m.mesh.indices.len() as u32,
                    material: m.mesh.material_id.unwrap_or(0),
                }
            })
            .collect::<Vec<_>>();

        Ok(Model { meshes, materials })
    }
}
