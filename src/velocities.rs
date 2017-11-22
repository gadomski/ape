use chrono::{DateTime, Utc};
use cpd::{Matrix, Normalize, Runner, U3};
use failure::Error;
use las::{Point, Reader};
use nalgebra::Point3;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const GRID_SIZE: i64 = 100;

#[derive(Debug, Fail)]
#[fail(display = "No moving path for path: {}", _0)]
struct NoMovingPath(String);

pub fn velocities<P: AsRef<Path>>(path: P) -> Result<Vec<Velocity>, Error> {
    let fixed = Grid::from_path(&path)?;
    let moving_path = moving_path(path)?;
    let moving = Grid::from_path(moving_path)?;
    let rigid = Runner::new()
        .normalize(Normalize::SameScale)
        .rigid()
        .scale(false);
    let mut velocities = Vec::new();
    for (&(r, c), fixed) in &fixed.map {
        if let Some(moving) = moving.map.get(&(r, c)) {
            if fixed.len() < 1000 || moving.len() < 1000 {
                continue;
            }
            println!(
                "Running grid cell ({}, {}) with {} fixed points and {} moving points",
                r,
                c,
                fixed.len(),
                moving.len()
            );
            let fixed = points_to_matrix(fixed);
            let moving = points_to_matrix(moving);
            let run = rigid.register(&fixed, &moving)?;
            if run.converged {
                let point = center_of_gravity(&fixed);
                let moved_point = run.transform.as_transform3() * point;
                let velocity = (moved_point - point) / 6.;
                velocities.push(Velocity {
                    x: point.coords[0],
                    y: point.coords[1],
                    z: point.coords[2],
                    vx: velocity[0],
                    vy: velocity[1],
                    vz: velocity[2],
                });
            }
        }
    }
    Ok(velocities)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Velocity {
    x: f64,
    y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
}

struct Grid {
    map: HashMap<(i64, i64), Vec<Point>>,
}

impl NoMovingPath {
    fn new<P: AsRef<Path>>(path: P) -> NoMovingPath {
        NoMovingPath(path.as_ref().display().to_string())
    }
}

impl Grid {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Grid, Error> {
        let mut map = HashMap::new();
        for point in Reader::from_path(path)?.points() {
            let point = point?;
            let c = point.x as i64 / GRID_SIZE;
            let r = point.y as i64 / GRID_SIZE;
            map.entry((r, c)).or_insert_with(Vec::new).push(point);
        }
        Ok(Grid { map: map })
    }
}

fn moving_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, Error> {
    let fixed = datetime_from_path(path.as_ref())?;
    let error = NoMovingPath::new(path.as_ref());
    path.as_ref()
        .parent()
        .and_then(|parent| parent.read_dir().ok())
        .and_then(|read_dir| {
            read_dir.filter_map(|r| r.ok()).find(|dir_entry| {
                is_the_moving_path(fixed, dir_entry.path())
            })
        })
        .map(|dir_entry| dir_entry.path().to_path_buf())
        .ok_or(error.into())
}

fn datetime_from_path<P: AsRef<Path>>(path: P) -> Result<DateTime<Utc>, Error> {
    use chrono::TimeZone;

    if let Some(file_name) = path.as_ref().file_name().and_then(|f| f.to_str()) {
        let datetime = Utc.datetime_from_str(&file_name[0..13], "%y%m%d_%H%M%S")?;
        Ok(datetime)
    } else {
        Err(NoMovingPath::new(path.as_ref()).into())
    }
}

fn is_the_moving_path<P: AsRef<Path>>(fixed: DateTime<Utc>, path: P) -> bool {
    use chrono::Duration;
    datetime_from_path(path)
        .map(|moving| {
            let duration = moving.signed_duration_since(fixed);
            duration > Duration::hours(0) && duration < Duration::hours(7)
        })
        .unwrap_or(false)
}

fn points_to_matrix(points: &Vec<Point>) -> Matrix<U3> {
    let mut matrix = Matrix::<U3>::zeros(points.len());
    for (i, point) in points.iter().enumerate() {
        matrix[(i, 0)] = point.x;
        matrix[(i, 1)] = point.y;
        matrix[(i, 2)] = point.z;
    }
    matrix
}

fn center_of_gravity(matrix: &Matrix<U3>) -> Point3<f64> {
    use cpd::Vector;
    let mut point = Vector::<U3>::zeros();
    for d in 0..3 {
        point[d] = matrix.column(d).iter().sum::<f64>() / matrix.nrows() as f64;
    }
    Point3::from_coordinates(point)
}