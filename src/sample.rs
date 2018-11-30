use cpd::{rigid::Transform, Rigid, Run, Runner};
use failure::Error;
use nalgebra::U3;
use std::f64::consts::PI;
use {Config, Point, RTree};

/// A sample of the glacier's velocity.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Sample {
    x: f64,
    y: f64,
    fixed_density: f64,
    moving_density: f64,
    cpd: Option<Cpd>,
}

/// A CPD run.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cpd {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    run: Run<U3, Transform<U3>>,
}

impl Sample {
    /// Samples the data at the provided point.
    pub fn new(
        config: Config,
        fixed: &RTree,
        moving: &RTree,
        point: Point,
    ) -> Result<Option<Sample>, Error> {
        let mut sample = Sample {
            x: point.x(),
            y: point.y(),
            ..Default::default()
        };
        let radius2 = config.step as f64 * config.step as f64;
        let fixed_in_circle = fixed.lookup_in_circle(&point, &radius2).len();
        let moving_in_circle = fixed.lookup_in_circle(&point, &radius2).len();
        if fixed_in_circle == 0 || moving_in_circle == 0 {
            return Ok(None);
        }
        let area = PI * radius2;
        sample.fixed_density = fixed_in_circle as f64 / area;
        sample.moving_density = moving_in_circle as f64 / area;
        if sample.fixed_density < config.min_density || sample.moving_density < config.min_density {
            return Ok(Some(sample));
        }
        let fixed = config.nearest_neighbors(fixed, &point);
        let moving = config.nearest_neighbors(moving, &point);
        let mut runner = Runner::new();
        if let Some(max_iterations) = config.max_iterations {
            runner.max_iterations = max_iterations;
        }
        let rigid = Rigid::new();
        sample.cpd = Some(Cpd {
            xmin: fixed.column(0).amin().min(moving.column(0).amin()),
            xmax: fixed.column(0).amax().max(moving.column(0).amax()),
            ymin: fixed.column(1).amin().min(moving.column(1).amin()),
            ymax: fixed.column(1).amax().max(moving.column(1).amax()),
            run: runner.run(&rigid, &fixed, &moving)?,
        });
        Ok(Some(sample))
    }
}