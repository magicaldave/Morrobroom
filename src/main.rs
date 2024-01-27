use delaunator::{triangulate, Point};
use nalgebra::Vector3;
use tes3::{
    esp::*,
    nif::{self, TextureSource},
};

const EPSILON: f64 = 1e-10;

// This will probably be necessary later, as tes3 lib uses hashsets for basically everything
// use std::collections::{HashMap, HashSet};

// In effect, this struct represents each line of a brush.
// The stored data is derived from the contents of the map file and differs quite a bit, however
// In the .map a plane is represented by a set of three dimensional points which correlate to its two triangles
// This is not, however, actually the same as the tris which are drawn on faces
// as those are derived from the actual vertex coordinates
#[derive(Debug)]
struct Plane {
    normal: (f64, f64, f64),
    distance: f64,
}

impl Plane {
    fn from_points(p1: (f64, f64, f64), p2: (f64, f64, f64), p3: (f64, f64, f64)) -> Self {
        let v1 = (p2.0 - p1.0, p2.1 - p1.1, p2.2 - p1.2);
        let v2 = (p3.0 - p1.0, p3.1 - p1.1, p3.2 - p1.2);

        let normal = (
            v1.1 * v2.2 - v1.2 * v2.1,
            v1.2 * v2.0 - v1.0 * v2.2,
            v1.0 * v2.1 - v1.1 * v2.0,
        );

        let magnitude = (normal.0.powi(2) + normal.1.powi(2) + normal.2.powi(2)).sqrt();
        let normal = (
            normal.0 / magnitude,
            normal.1 / magnitude,
            normal.2 / magnitude,
        );

        let distance = -(normal.0 * p1.0 + normal.1 * p1.1 + normal.2 * p1.2);

        Plane { normal, distance }
    }

    fn contains_point(&self, point: (f64, f64, f64)) -> bool {
        self.normal.0 * point.0 + self.normal.1 * point.1 + self.normal.2 * point.2 + self.distance
            > 0.0
    }
}

// This is a literal representation of the face of a brush.
// There could be an arbitrary number of vertices here so long as all the planes intersect.
// However, each face is representative of one plane, so its index is stored here.
// This has nothing to do with the plane itself, but rather, is later used for deriving texture info.
#[derive(Debug, Clone)]
struct Face {
    vertices: Vec<Vector3<f64>>,
    plane_index: usize, // Index of the associated plane in the vector of planes, used for texture gathering
    triangles: Vec<Vec<usize>>,
}

impl Face {
    fn new(vertices: Vec<Vector3<f64>>, plane_index: usize) -> Self {
        Face {
            vertices,
            plane_index,
            triangles: Vec::new(),
        }
    }
}

fn find_intersection_point(
    plane1: &Plane,
    plane2: &Plane,
    plane3: &Plane,
) -> Option<(f64, f64, f64)> {
    // Check for non-parallel planes
    let det = plane1.normal.0
        * (plane2.normal.1 * plane3.normal.2 - plane3.normal.1 * plane2.normal.2)
        - plane1.normal.1 * (plane2.normal.0 * plane3.normal.2 - plane3.normal.0 * plane2.normal.2)
        + plane1.normal.2 * (plane2.normal.0 * plane3.normal.1 - plane3.normal.0 * plane2.normal.1);

    if det.abs() < EPSILON {
        // The planes are parallel or nearly parallel
        return None;
    }

    // Solve for variables
    let det_x = plane1.distance
        * (plane2.normal.1 * plane3.normal.2 - plane3.normal.1 * plane2.normal.2)
        - plane1.normal.1 * (plane2.distance * plane3.normal.2 - plane3.distance * plane2.normal.2)
        + plane1.normal.2 * (plane2.distance * plane3.normal.1 - plane3.distance * plane2.normal.1);

    let det_y = plane1.normal.0
        * (plane2.distance * plane3.normal.2 - plane3.distance * plane2.normal.2)
        - plane1.distance * (plane2.normal.0 * plane3.normal.2 - plane3.normal.0 * plane2.normal.2)
        + plane1.normal.2 * (plane2.normal.0 * plane3.distance - plane3.normal.0 * plane2.distance);

    let det_z = plane1.normal.0
        * (plane2.normal.1 * plane3.distance - plane3.normal.1 * plane2.distance)
        - plane1.normal.1 * (plane2.normal.0 * plane3.distance - plane3.normal.0 * plane2.distance)
        + plane1.distance * (plane2.normal.0 * plane3.normal.1 - plane3.normal.0 * plane2.normal.1);

    // Calculate intersection point
    let x = det_x / det;
    let y = det_y / det;
    let z = det_z / det;

    Some((x, y, z))
}

fn main() {
    let mut planes = vec![];
    let mut vertices = vec![];

    // First, you must collect every plane from the set
    // The resulting object contains the plane normal and distance
    planes.push(Plane::from_points(
        (-64.0, -64.0, -16.0),
        (-64.0, -63.0, -16.0),
        (-64.0, -64.0, -15.0),
    ));

    planes.push(Plane::from_points(
        (-64.0, -64.0, -16.0),
        (-64.0, -64.0, -15.0),
        (-63.0, -64.0, -16.0),
    ));

    planes.push(Plane::from_points(
        (-64.0, -64.0, -16.0),
        (-63.0, -64.0, -16.0),
        (-64.0, -63.0, -16.0),
    ));

    planes.push(Plane::from_points(
        (64.0, 64.0, 16.0),
        (64.0, 65.0, 16.0),
        (65.0, 64.0, 16.0),
    ));

    planes.push(Plane::from_points(
        (64.0, 64.0, 16.0),
        (65.0, 64.0, 16.0),
        (64.0, 64.0, 17.0),
    ));

    planes.push(Plane::from_points(
        (64.0, 64.0, 16.0),
        (64.0, 64.0, 17.0),
        (64.0, 65.0, 16.0),
    ));

    // Then, to collect vertices, find all
    // intersection points between all possible combinations of three planes (3-dimensional space)
    for i in 0..planes.len() {
        for j in i + 1..planes.len() {
            for k in j + 1..planes.len() {
                if let Some(intersection_point) =
                    find_intersection_point(&planes[i], &planes[j], &planes[k])
                {
                    // It's important to flip y/z order as map uses y-up, but we need z-up
                    // Every one of the resulting points is an actual three-dimensional point
                    // making up the shape of the mesh.
                    println!(
                        "Intersection Point: X:{}, Y:{}, Z:{}",
                        intersection_point.0, intersection_point.2, intersection_point.1
                    );
                    vertices.push(Vector3::new(
                        intersection_point.0,
                        intersection_point.2,
                        intersection_point.1,
                    ));
                }
            }
        }
    }

    let mut faces = associate_vertices_with_planes(&planes, &vertices, |v| (v.x, v.y, v.z));

    triangulate_faces(&mut faces, &vertices);

    let mut shape = nif::NiTriShape::default();
    let mut shape_data = nif::NiTriShapeData::default();

    for vertex in vertices {
        shape_data
            .vertices
            .push([vertex[0] as f32, vertex[2] as f32, vertex[1] as f32].into());
    }

    println!("{:?}", shape_data.vertices);

    // Now `faces` contains the associations between vertices and planes
    for face in &faces {
        println!(
            "Face with Plane Index {}: {:?}, {:?}",
            face.plane_index, face.vertices, face.triangles
        );
        for tri_indices in &face.triangles {
            let triangle: [u16; 3] = [
                tri_indices[0] as u16,
                tri_indices[1] as u16,
                tri_indices[2] as u16,
            ];
            shape_data.triangles.push(triangle);
        }
    }

    let mut ni_stream = nif::NiStream::default();
    let shape_index = ni_stream.insert(shape);
    ni_stream.roots = vec![shape_index.cast()];
    let shape_data_index = ni_stream.insert(shape_data);

    // "tx_b_n_wood elf_m_h02"

    assign_texture(
        &mut ni_stream,
        shape_index.into(),
        "tx_b_n_wood elf_m_h02.dds",
    );

    if let Some(shape) = ni_stream.get_mut(shape_index) {
        shape.geometry_data = shape_data_index.cast(); // downcasts to NiLink<NiTriShapeData> to NiLink<GeometryData>
    };

    ni_stream.save_path("test.nif");
}

fn associate_vertices_with_planes<T>(
    planes: &[Plane],
    vertices: &[T],
    extract_coords: impl Fn(&T) -> (f64, f64, f64),
) -> Vec<Face> {
    planes
        .iter()
        .enumerate()
        .flat_map(|(plane_index, plane)| {
            let adjusted_vertices: Vec<Vector3<f64>> = vertices
                .iter()
                .map(|vertex| {
                    let (x, y, z) = extract_coords(vertex);
                    Vector3::new(x, z, y)
                })
                .collect();

            let associated_vertices: Vec<Vector3<f64>> = adjusted_vertices
                .iter()
                .filter(|&vertex| plane.contains_point((vertex.x, vertex.y, vertex.z)))
                .cloned()
                .collect();

            if !associated_vertices.is_empty() {
                Some(Face::new(associated_vertices, plane_index))
            } else {
                None
            }
        })
        .collect()
}

fn triangulate_faces(faces: &mut Vec<Face>, vertices: &[Vector3<f64>]) {
    for face in faces.iter_mut() {
        // Determine which dimensions vary among the vertices
        let mut varying_dimensions: Vec<usize> = Vec::new();
        for dim in 0..3 {
            let values: Vec<f64> = face.vertices.iter().map(|vertex| vertex[dim]).collect();

            // Check if all values are not the same
            if values.windows(2).any(|w| w[0] != w[1]) {
                varying_dimensions.push(dim);
            }
        }

        // Extract points for triangulation, including the vertex, and its original index
        let points: Vec<(Point, usize)> = face
            .vertices
            .iter()
            .map(|vertex| {
                let adjusted_vertex = Vector3::new(
                    vertex[0], // Use the first varying dimension as x
                    vertex[2], // Use the second varying dimension as z (adjusting for y and z flip)
                    vertex[1], // Use the third varying dimension as y (adjusting for y and z flip)
                );
                let index_in_vertices =
                    vertices.iter().position(|v| *v == adjusted_vertex).unwrap();
                (
                    Point {
                        x: vertex[varying_dimensions[0]],
                        y: vertex[varying_dimensions[1]],
                    },
                    index_in_vertices,
                )
            })
            .collect();

        // Extract the points from the tuple and convert to Vec<Point>
        let point_vec: Vec<Point> = points.clone().into_iter().map(|(point, _)| point).collect();

        // Triangulate the face
        let result = triangulate(&point_vec);

        // Translate Delaunator indices to original vertex indices
        let original_triangles: Vec<Vec<usize>> = result
            .triangles
            .chunks_exact(3)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|delaunator_index| points[*delaunator_index].1)
                    .collect()
            })
            .collect();

        face.triangles = original_triangles;
    }
}

fn assign_texture(
    stream: &mut nif::NiStream,
    object: nif::NiLink<nif::NiTriShape>,
    file_path: &str,
) {
    // Create and insert a NiTexturingProperty and NiSourceTexture.
    let tex_prop_link = stream.insert(nif::NiTexturingProperty::default());
    let texture_link = stream.insert(nif::NiSourceTexture::default());

    // Update the base map texture.
    let tex_prop = stream.get_mut(tex_prop_link).unwrap();
    tex_prop.texture_maps.resize(7, None); // not sure why
    let mut base_map = nif::Map::default();
    base_map.texture = texture_link.cast();
    tex_prop.texture_maps[0] = Some(nif::TextureMap::Map(base_map));

    // Update the texture source path.
    let texture = stream.get_mut(texture_link).unwrap();
    texture.source = nif::TextureSource::External(file_path.into());

    // Assign the tex prop to the target object
    let object = stream.get_mut(object).unwrap();
    object.properties.push(tex_prop_link.cast());
}
