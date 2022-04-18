use bytemuck::{Pod, Zeroable};

#[derive(Clone)]
pub struct Vertices {
    vertices: Vec<Vertex>,
    indices: Vec<Index>,
}

impl Vertices {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<Index>) -> Self {
        Self { vertices, indices }
    }

    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn vertices(&self) -> &[Vertex] {
        self.vertices.as_slice()
    }

    pub fn indices(&self) -> &[Index] {
        self.indices.as_slice()
    }
}

pub type Index = u32;

#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}
