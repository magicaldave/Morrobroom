// use delaunator::{triangulate, Point};
use nalgebra::{matrix, vector, DMatrix, DVector, Vector3};
// use tes3::{
//     esp::*,
//     nif::{self, TextureSource},
// };

const EPSILON: f64 = 1e-10;

/// In effect, this struct represents each face of a brush.
/// It only stores the normal and distance, which are used later for vertex calculation
#[derive(Debug, Clone)]
struct Plane {
    normal: Vector3<f64>,
    distance: f64,
}

// For some reason this only works when switching the second and third set of points.
// Winding order?
// I'm not asking more questions unless it's broken.
impl Plane {
    fn new(points: [f64; 9]) -> Self {
        let a = Vector3::new(points[0], points[1], points[2]);
        let b = Vector3::new(points[6], points[7], points[8]);
        let c = Vector3::new(points[3], points[4], points[5]);

        let ab = b - a;
        let ac = c - a;

        let normal = ab.cross(&ac);
        let distance = normal.dot(&a);

        Plane { normal, distance }
    }
}

/// The purpose of this function is to solve the system of inequalities provided by three half-spaces.
/// In other words, this extracts a vertex from the intersection of three planes.
/// In order to use it properly, you must iterate through all possible combinations of three planes
/// and provide all three as a slice to this function.
fn solve_system_from_tuples(planes: &[&Plane]) -> Option<DVector<f64>> {
    // Define the half-space coefficients matrix A and the right-hand side vector b
    let a = matrix![
        planes[0].normal.x, planes[0].normal.y, planes[0].normal.z;
        planes[1].normal.x, planes[1].normal.y, planes[1].normal.z;
        planes[2].normal.x, planes[2].normal.y, planes[2].normal.z
    ];

    let b = DVector::from_iterator(3, planes.iter().map(|p| p.distance)); // right-hand side values

    // Solve for the point v using A^-1 * b
    a.try_inverse()
        .map(|a_inv| a_inv * b)
        .map(|v| DVector::from_iterator(v.len(), v.iter().cloned()))
}

/// This portion filters out vertices which are not exactly on the edge of every possible half-space.
/// In simpler terms, the objective is to remove any vertices which are either outside, or inside, of the edges.
/// Without this extra vertices will be generated which don't directly correlate to the shape.
fn filter_vertices_by_halfspaces(
    vertices: Vec<DVector<f64>>,
    halfspaces: &[Plane],
) -> Vec<Vector3<f64>> {
    let mut filtered_vertices = Vec::new();

    for vertex in vertices {
        // Check if the vertex is within any half-space
        if halfspaces
            .iter()
            .all(|plane| plane.normal.dot(&vertex.fixed_rows::<3>(0)) <= (plane.distance + EPSILON))
        {
            let vert = Vector3::new(vertex[0], vertex[1], vertex[2]);
            // Check if the vector is not already in the result
            if !filtered_vertices.contains(&vert) {
                filtered_vertices.push(vert);
            }
        }
    }

    filtered_vertices
}

fn main() {
    let mut planes = vec![];
    let mut verts = vec![];

    // First, assemble all planes of each brush into a vector
    // Afterward, we collect and triangulate its vertices
    planes.push(Plane::new([
        -64.0, -4.0, -4.0, -64.0, 0.0, -4.0, -75.75, -4.0, 0.0,
    ]));

    planes.push(Plane::new([
        -64.0, 0.0, -4.0, -64.0, 0.0, 0.0, -75.75, -4.0, 0.0,
    ]));

    planes.push(Plane::new([
        -75.75, -4.0, 0.0, 72.0, -4.0, 0.0, 72.0, -4.0, -4.0,
    ]));

    planes.push(Plane::new([
        72.0, -4.0, -4.0, 72.0, 0.0, -4.0, -64.0, 0.0, -4.0,
    ]));

    planes.push(Plane::new([
        -64.0, 0.0, 0.0, 72.0, 0.0, 0.0, 72.0, -4.0, 0.0,
    ]));

    planes.push(Plane::new([
        72.0, 0.0, -4.0, 72.0, 0.0, 0.0, -64.0, 0.0, 0.0,
    ]));

    planes.push(Plane::new([
        72.0, -4.0, 0.0, 72.0, 0.0, 0.0, 72.0, 0.0, -4.0,
    ]));

    // Loop over all combinations of three planes
    for i in 0..planes.len() {
        for j in (i + 1)..planes.len() {
            for k in (j + 1)..planes.len() {
                // Solve the system for the selected planes
                if let Some(vertex) =
                    solve_system_from_tuples(&[&planes[i], &planes[j], &planes[k]])
                {
                    verts.push(vertex);
                }
            }
        }
    }

    let filtered_verts: Vec<Vector3<f64>> = filter_vertices_by_halfspaces(verts, &planes);

    for vert in filtered_verts {
        // NOTE: ACTUAL OUTPUT VERTICES FROM THE CODE ARE NOT IN THIS FORMAT.
        // THIS IS SIMPLY REPRESENTATIVE OF WHAT THE VERTICES WILL LOOK LIKE IN THE FINAL OUTPUT MESH
        println!("VERTEX: {}, {}, {}", vert[0], vert[2], -vert[1]);
    }
}
