extern crate ape;
extern crate chrono;
#[macro_use]
extern crate clap;
extern crate cpd;
extern crate env_logger;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_json;
extern crate walkdir;

use ape::incl::{Inclination, Stats};
use ape::utils;
use chrono::{DateTime, TimeZone, Utc};
use clap::ArgMatches;
use walkdir::WalkDir;

lazy_static! {
    pub static ref SCANNER_SWAP: DateTime<Utc> = Utc.ymd(2016, 8, 12).and_hms(0, 0, 0);
}

#[allow(unused_variables)]
fn main() {
    env_logger::init().unwrap();

    use clap::App;

    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    if let Some(matches) = matches.subcommand_matches("cpd") {
        cpd(matches);
    } else if let Some(matches) = matches.subcommand_matches("magic-bucket-config") {
        magic_bucket_config(matches);
    } else if let Some(matches) = matches.subcommand_matches("incl") {
        if let Some(matches) = matches.subcommand_matches("extract") {
            #[cfg(feature = "scanlib")] incl_extract(matches);
            #[cfg(not(feature = "scanlib"))]
            panic!("ape-incl-extract not supported without scanlib");
        } else if let Some(matches) = matches.subcommand_matches("cat") {
            incl_cat(matches);
        } else if let Some(matches) = matches.subcommand_matches("stats") {
            incl_stats(matches);
        } else if let Some(matches) = matches.subcommand_matches("timeseries") {
            incl_timeseries(matches);
        }
    }
}

fn cpd(matches: &ArgMatches) {
    use std::fs::File;
    use cpd::{Normalize, Runner, utils};
    use std::io::Write;

    let fixed = utils::matrix_from_las_path(matches.value_of("FIXED").unwrap()).unwrap();
    let moving = utils::matrix_from_las_path(matches.value_of("MOVING").unwrap()).unwrap();
    let outfile = matches.value_of("OUTFILE").unwrap();
    let rigid = Runner::new()
        .normalize(Normalize::SameScale)
        .rigid()
        .scale(false)
        .allow_reflections(false);
    let run = rigid.register(&fixed, &moving).unwrap();
    if run.converged {
        let rotation = run.transform.rotation;
        let translation = run.transform.translation;
        let mut outfile = File::create(outfile).unwrap();
        for r in 0..3 {
            for c in 0..3 {
                write!(outfile, "{} ", rotation[(r, c)]).unwrap();
            }
            writeln!(outfile, "{}", translation[r]).unwrap();
        }
        writeln!(outfile, "0 0 0 1").unwrap();
    } else {
        panic!("cpd did not converge!");
    }
}

fn magic_bucket_config(matches: &ArgMatches) {
    use std::fs::File;
    use std::io::Read;

    let read = |name| {
        let mut matrix = String::new();
        let mut file = File::open(matches.value_of(name).unwrap()).unwrap();
        file.read_to_string(&mut matrix).unwrap();
        matrix
            .replace("\n", " ")
            .replace("\r", " ")
            .trim()
            .to_string()
    };

    let sop = read("SOP");
    let adjustment = read("ADJUSTMENT");
    let pop = read("POP");

    let config = json!({
        "filters": [
            {
                "type": "filters.transformation",
                "matrix": sop,
            },
            {
                "type": "filters.transformation",
                "matrix": adjustment,
            },
            {
                "type": "filters.transformation",
                "matrix": pop,
            },
            {
                "type": "filters.crop",
                "polygon": "POLYGON ((535508.04019199998584 7356923.27050799969584, 526852.992188 7363507.49072299990803, 533350.83911099995021 7365850.74902299977839, 541962.312012 7365547.070313, 545282.91503899998497 7360871.8720699995756, 542695.264648 7358447.21875, 537531.614136 7357506.45642099995166, 536543.26751699997112 7357541.5081789996475, 535508.04019199998584 7356923.27050799969584))"
            },
            {
                "type": "filters.range",
                "limits": "Z[0:250]",
            },
            {
                "type": "filters.outlier",
            },
            {
                "type": "filters.colorinterp",
                "ramp": "pestel_shades",
                "minimum": 0,
                "maximum": 250,
            }
        ],
        "output_ext": ".laz",
        "args": [
            "--writers.las.scale_x=0.0025",
            "--writers.las.scale_y=0.0025",
            "--writers.las.scale_z=0.0025",
            "--writers.las.offset_x=auto",
            "--writers.las.offset_y=auto",
            "--writers.las.offset_z=auto",
            "--writers.las.a_srs=EPSG:32624+5773",
        ]
    });
    println!("{}", serde_json::to_string_pretty(&config).unwrap());
}

#[cfg(feature = "scanlib")]
fn incl_extract(matches: &ArgMatches) {
    use ape::incl;
    let infile = matches.value_of("INFILE").unwrap();
    let outfile = matches.value_of("OUTFILE").unwrap();
    incl::linux::extract(infile, outfile, matches.is_present("sync-to-pps")).unwrap()
}

fn incl_cat(matches: &ArgMatches) {
    let inclinations = Inclination::vec_from_path(matches.value_of("INFILE").unwrap()).unwrap();
    println!("time,roll,pitch");
    for inclination in inclinations {
        println!(
            "{},{},{}",
            inclination.time,
            inclination.roll,
            inclination.pitch
        );
    }
}

fn incl_stats(matches: &ArgMatches) {
    let stats = Stats::from_path(matches.value_of("INFILE").unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&stats).unwrap());
}

fn incl_timeseries(matches: &ArgMatches) {
    use chrono::{Datelike, Timelike};
    let directory = matches.value_of("DIRECTORY").unwrap();
    println!("ordinal,year,hour,name,mean,stddev");
    for entry in WalkDir::new(directory) {
        let entry = entry.unwrap();
        if entry.path().extension().map(|e| e == "incl").unwrap_or(
            false,
        )
        {
            let stats = Stats::from_path(entry.path()).unwrap();
            let datetime = utils::riegl_datetime_from_path(entry.path()).unwrap();
            let hour = datetime.hour();
            if hour % 6 == 0 {
                let ordinal = datetime.ordinal();
                let year = datetime.year();
                let year = if year == 2016 {
                    if datetime < *SCANNER_SWAP {
                        "2016-a"
                    } else {
                        "2016-b"
                    }
                } else {
                    "2015"
                };
                println!(
                    "{},{},{},{},{},{}",
                    ordinal,
                    year,
                    hour,
                    "roll",
                    stats.roll.mean,
                    stats.roll.stddev
                );
                println!(
                    "{},{},{},{},{},{}",
                    ordinal,
                    year,
                    hour,
                    "pitch",
                    stats.pitch.mean,
                    stats.pitch.stddev
                );
            }
        }
    }
}
