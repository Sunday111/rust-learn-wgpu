use std::{
    collections::HashMap,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
    rc::Rc,
};

use wgpu::util::DeviceExt;

pub trait Vertex {
    fn layout() -> wgpu::VertexBufferLayout<'static>;
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

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: klgl::Texture,
    pub bind_group: wgpu::BindGroup,
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

fn get_value_from_map<'a, K, V, S, Q>(map: &'a HashMap<K, V, S>, key: &Q) -> anyhow::Result<&'a V>
where
    K: Eq + std::hash::Hash,
    S: std::hash::BuildHasher,
    K: std::borrow::Borrow<Q>,
    Q: ?Sized + std::hash::Hash + std::cmp::Eq + std::fmt::Debug,
{
    map.get(key)
        .ok_or_else(|| anyhow::anyhow!("Key '{:?}' not found in the map", key))
}

pub fn load_model(
    file_name: &str,
    ctx: &klgl::RenderContext,
    layout: &wgpu::BindGroupLayout,
    file_data_map: &HashMap<PathBuf, klgl::file_loader::FileDataHandle>,
) -> anyhow::Result<Model> {
    let (models, obj_materials) = {
        let obj_file = get_value_from_map(file_data_map, Path::new(file_name))?;
        let obj_cursor = Cursor::new(&obj_file.data);
        let mut obj_reader = BufReader::new(obj_cursor);

        tobj::load_obj_buf(
            &mut obj_reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |p| match file_data_map.get(p) {
                Some(file_data) => {
                    tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(&file_data.data)))
                }
                None => Err(tobj::LoadError::OpenFileFailed),
            },
        )
    }?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        let diffuse_path = &m
            .diffuse_texture
            .ok_or_else(|| anyhow::anyhow!("Null diffuse texture for {}", file_name))?;
        let texture_file = get_value_from_map(file_data_map, Path::new(&diffuse_path))?.clone();
        let diffuse_texture =
            klgl::Texture::from_bytes(&ctx.device, &ctx.queue, &texture_file.data, &diffuse_path)?;
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

            let vertex_buffer = ctx
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Vertex Buffer", file_name)),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = ctx
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Index Buffer", file_name)),
                    contents: bytemuck::cast_slice(&m.mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(Model { meshes, materials })
}
