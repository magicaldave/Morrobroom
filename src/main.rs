use delaunator::{triangulate, Point};
use nalgebra::{matrix, vector, DMatrix, DVector, Matrix3, Point3, Vector3, SVD};
use tes3::{
    esp::*,
    nif::{self, TextureSource},
};

const EPSILON: f64 = 1e-10;
const DECIMAL_PLACES: u32 = 6; // Set the desired precision

fn get_plane(points: &[Vector3<f64>; 3]) -> (Vector3<f64>, f64) {
    let a = points[0];
    let b = points[2];
    let c = points[1];

    let ab = b - a;
    let ac = c - a;

    let normal = ab.cross(&ac);
    let d = normal.dot(&a);

    (normal, d)
}

fn solve_system_from_tuples(half_spaces: &[(Vector3<f64>, f64)]) -> Option<DVector<f64>> {
    // Define the half-space coefficients matrix A and the right-hand side vector b
    let a = DMatrix::from_fn(3, 3, |i, j| match (i, j) {
        (0, 0) => half_spaces[0].0.x, // coefficients for the first half-space
        (0, 1) => half_spaces[0].0.y,
        (0, 2) => half_spaces[0].0.z,
        (1, 0) => half_spaces[1].0.x, // coefficients for the second half-space
        (1, 1) => half_spaces[1].0.y,
        (1, 2) => half_spaces[1].0.z,
        (2, 0) => half_spaces[2].0.x, // coefficients for the third half-space
        (2, 1) => half_spaces[2].0.y,
        (2, 2) => half_spaces[2].0.z,
        _ => 0.0,
    });

    let b = DVector::from_iterator(3, half_spaces.iter().map(|(_, d)| *d)); // right-hand side values

    // Solve for the point v using A^-1 * b
    a.try_inverse().map(|a_inv| a_inv * b)
}

fn filter_vertices_by_halfspaces(
    vertices: Vec<DVector<f64>>,
    halfspaces: &[(Vector3<f64>, f64)],
) -> Vec<Vector3<f64>> {
    let mut filtered_vertices = Vec::new();

    for vertex in vertices {
        // Check if the vertex is within any half-space
        if halfspaces.iter().all(|&(normal, distance)| {
            normal.dot(&vertex.fixed_rows::<3>(0)) <= (distance + EPSILON)
        }) {
            // Check if the vector is not already in the result
            if !filtered_vertices.contains(&Vector3::new(vertex[0], vertex[1], vertex[2])) {
                filtered_vertices.push(Vector3::new(vertex[0], vertex[1], vertex[2]));
            }
        }
    }

    filtered_vertices
}

fn main() {
    let mut planes = vec![];
    let mut verts = vec![];

    planes.push(get_plane(&[
        vector![-64.0, -4.0, -4.0],
        vector![-64.0, 0.0, -4.0],
        vector![-75.75, -4.0, 0.0],
    ]));

    planes.push(get_plane(&[
        vector![-64.0, 0.0, -4.0],
        vector![-64.0, 0.0, 0.0],
        vector![-75.75, -4.0, 0.0],
    ]));

    planes.push(get_plane(&[
        vector![-75.75, -4.0, 0.0],
        vector![72.0, -4.0, 0.0],
        vector![72.0, -4.0, -4.0],
    ]));

    planes.push(get_plane(&[
        vector![72.0, -4.0, -4.0],
        vector![72.0, 0.0, -4.0],
        vector![-64.0, 0.0, -4.0],
    ]));

    planes.push(get_plane(&[
        vector![-64.0, 0.0, 0.0],
        vector![72.0, 0.0, 0.0],
        vector![72.0, -4.0, 0.0],
    ]));

    planes.push(get_plane(&[
        vector![72.0, 0.0, -4.0],
        vector![72.0, 0.0, 0.0],
        vector![-64.0, 0.0, 0.0],
    ]));

    planes.push(get_plane(&[
        vector![72.0, -4.0, 0.0],
        vector![72.0, 0.0, 0.0],
        vector![72.0, 0.0, -4.0],
    ]));

    // Loop over all combinations of three planes
    for i in 0..planes.len() {
        for j in (i + 1)..planes.len() {
            for k in (j + 1)..planes.len() {
                // Solve the system for the selected planes
                if let Some(vertex) = solve_system_from_tuples(&[planes[i], planes[j], planes[k]]) {
                    verts.push(vertex);
                }
            }
        }
    }

    let filtered_verts: Vec<Vector3<f64>> = filter_vertices_by_halfspaces(verts, &planes);

    for vert in filtered_verts {
        // NOTE: ACTUAL OUTPUT VERTICES FROM THE CODE ARE NOT IN THIS FORMAT.
        // THIS IS SIMPLY REPRESENTATIVE OF WHAT THE VERTICES WILL LOOK LIKE IN THE FINAL OUTPUT MESH
        println!("VERTEX: {}, {}, {}", vert.x, vert.z, -vert.y);
    }
}
