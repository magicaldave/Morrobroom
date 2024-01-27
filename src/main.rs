// First things first:
// 1. Open the file
// 2. Find a brush , according to bracketed bounds?
// 3. Entities, if they exist, will be defined in another block containing its brushes.
// 4.
use nalgebra::{DMatrix, DVector, Matrix3, Point3, Point4, Vector, Vector3};
use ordered_float::OrderedFloat;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
const EPSILON: f64 = 1e-10;
use delaunator::{triangulate, Point};

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
    vertices: Vec<(f64, f64, f64)>,
    plane_index: usize, // Index of the associated plane in the vector of planes
}

impl Face {
    fn new(vertices: Vec<(f64, f64, f64)>, plane_index: usize) -> Self {
        Face {
            vertices,
            plane_index,
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
                    vertices.push((
                        intersection_point.0,
                        intersection_point.2,
                        intersection_point.1,
                    ));
                }
            }
        }
    }

    let faces = associate_vertices_with_planes(&planes, &vertices);

    // Now `faces` contains the associations between vertices and planes
    for face in &faces {
        println!(
            "Face with Plane Index {}: {:?}",
            face.plane_index, face.vertices
        );
    }

    // triangulate_faces(&faces);
}

fn associate_vertices_with_planes(planes: &[Plane], vertices: &[(f64, f64, f64)]) -> Vec<Face> {
    planes
        .iter()
        .enumerate()
        .flat_map(|(plane_index, plane)| {
            let adjusted_vertices: Vec<(f64, f64, f64)> =
                vertices.iter().map(|&(x, y, z)| (x, z, y)).collect();

            let associated_vertices: Vec<(f64, f64, f64)> = adjusted_vertices
                .iter()
                .filter(|&vertex| plane.contains_point(*vertex))
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

// fn triangulate_faces(faces: &[Face]) {
//     for face in faces {
//         // Determine which dimensions vary among the vertices
//         let mut varying_dimensions: Vec<usize> = Vec::new();
//         for dim in 0..3 {
//             let values: Vec<f64> = face.vertices.iter().map(|&vertex| vertex[dim]).collect();
//             let unique_values: Vec<f64> = values.into_iter().collect();

//             if unique_values.len() > 1 {
//                 varying_dimensions.push(dim);
//             }
//         }

//         // Extract points for triangulation
//         let points: Vec<Point> = face
//             .vertices
//             .iter()
//             .map(|&(x, y, z)| Point {
//                 x: y, // Use the first varying dimension as x
//                 y: z, // Use the second varying dimension as y
//             })
//             .collect();

//         // Triangulate the face
//         let result = triangulate(&points);

//         // Print or use the triangles
//         println!("{:?}", result.triangles);
//     }
// }

// Print or use the hull
// println!("{:?}\n\n\n", result.hull);
// println!("Points: {:?}, Count: {}", points, points.len());
