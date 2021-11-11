use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::Result;
use thiserror::Error;

pub(crate) struct PolarService {
    polars_dir: PathBuf,
    archived_dir: PathBuf,
}

impl PolarService {

    fn create_dir(dir: &PathBuf) {
        if !dir.exists() {
            if let Err(e) = fs::create_dir_all(&dir) {
                panic!("Error creating dir {:?} : {}", dir, e);
            }
        } else if !dir.is_dir() {
            panic!("{:?} is not a directory", dir);
        }
    }

    pub(crate) fn new<P: Into<PathBuf>, Q: Into<PathBuf>>(polars_dir: P, archived_dir: Q) -> Self {
        let polars_dir: PathBuf = polars_dir.into();
        let archived_dir: PathBuf = archived_dir.into();
        Self::create_dir(&polars_dir);
        Self::create_dir(&archived_dir);
        PolarService { polars_dir, archived_dir }
    }

    pub(crate) async fn list(&self, archived: Option<bool>) -> Result<Vec<Polar>> {
        let mut res = Vec::new();

        let (dir, archived) = if let Some(true) = archived {
            (&self.archived_dir, true)
        } else {
            (&self.polars_dir, false)
        };

        let paths = fs::read_dir(dir)?;

        for entry in paths {
            if let Ok(entry) = entry {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        println!("entry {:?}", entry.path());
                        if let Some(ext) = entry.path().extension() {
                            if ext == OsStr::new("yaml") {
                                let file = File::open(entry.path()).unwrap();
                                let reader = BufReader::new(file);

                                // Read the JSON contents of the file as an instance of `AppInfo`.
                                match serde_yaml::from_reader(reader) {
                                    Ok(polar) => {
                                        let mut polar: Polar = polar;
                                        polar.id = Some(entry.path().file_prefix().unwrap().to_string_lossy().to_string());
                                        polar.archived = archived;
                                        res.push(polar);
                                    },
                                    Err(e) => {
                                        println!("Error reading file {:?} : {:?}", entry, e);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    println!("Couldn't get metadata for {:?}", entry.path());
                }
            }
        }

        Ok(res)
    }

    pub(crate) async fn get(&self, polar_id: String) -> Result<Option<Polar>> {

        let mut path = self.polars_dir.join(format!("{}.yaml", polar_id));
        let mut archived = false;
        if !path.exists() {
            path = self.archived_dir.join(format!("{}.yaml", polar_id));
            archived = true;
            if !path.exists() {
                return Ok(None)
            }
        }

        let reader = BufReader::new(File::open(&path)?);

        // Read the JSON contents of the file as an instance of `AppInfo`.
        let polar: Option<Polar> = serde_yaml::from_reader(reader)?;
        let polar = polar.map(|mut r: Polar| {
            r.id = Some(polar_id);
            r.archived = archived;
            r
        });
        Ok(polar)
    }

    pub(crate) async fn find_by_polar_id(&self, polar_id: u8) -> Result<Option<Polar>> {

        match self.list(None).await?.into_iter().find(|x| x.polar_id == polar_id) {
            Some(polar) => Ok(Some(polar)),
            None => {
                Ok(self.list(Some(true)).await?.into_iter().find(|x| x.polar_id == polar_id))
            },
        }
    }

    fn get_id(&self, polar: &Polar) -> Result<String> {
        match &polar.id {
            Some(id) => {
                Ok(id.clone())
            }
            None => {
                Err(PolarError::IdIsMandatory().into())
            }
        }
    }

    pub(crate) async fn create(&self, polar: &Polar) -> Result<()> {
        let id = self.get_id(polar)?;
        let path = self.polars_dir.join(format!("{}.yaml", id));
        if path.exists() {
            Err(PolarError::AlreadyExists(id).into())
        } else {
            match self.save_polar(&path, polar) {
                Ok(()) => Ok(()),
                Err(e) => {
                    println!("Error saving polar {:?} : {}", path, e);
                    Err(e.into())
                }
            }
        }
    }

    pub(crate) async fn update(&self, polar_id: String, polar: &Polar) -> Result<()> {
        let mut path = self.polars_dir.join(format!("{}.yaml", polar_id));
        if !path.exists() {
            return Err(PolarError::NotFound(polar_id).into())
        } else {

            if let Some(id) = &polar.id {
                if id != &polar_id {
                    // the id change. must remove old file and create new one.
                    match fs::remove_file(&path) {
                        Ok(_) => {},
                        Err(e) => {
                            println!("Error removing file {:?} : {}", path, e);
                            return Err(e.into());
                        }
                    }
                    path = self.polars_dir.join(format!("{}.yaml", id))
                }
            }

            match self.save_polar(&path, polar) {
                Ok(()) => Ok(()),
                Err(e) => {
                    println!("Error saving polar {:?} : {}", path, e);
                    Err(e.into())
                }
            }
        }
    }

    pub(crate) async fn delete(&self, polar_id: String) -> Result<()> {
        let mut path = self.polars_dir.join(format!("{}.yaml", polar_id));
        if !path.exists() {
            path = self.archived_dir.join(format!("{}.yaml", polar_id));
            if !path.exists() {
                return Err(PolarError::NotFound(polar_id).into())
            }
        }

        match fs::remove_file(&path) {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("Error removing file {:?} : {}", path, e);
                Err(e.into())
            }
        }
    }

    pub(crate) async fn archive(&self, polar_id: String) -> Result<()> {
        let path = self.polars_dir.join(format!("{}.yaml", polar_id));
        if !path.exists() {
            Err(PolarError::NotFound(polar_id).into())
        } else {
            let archived = self.archived_dir.join(format!("{}.yaml", polar_id));
            Self::rename(&path, &archived)
        }
    }

    pub(crate) async fn restore(&self, polar_id: String) -> Result<()> {
        let archived = self.archived_dir.join(format!("{}.yaml", polar_id));
        if !archived.exists() {
            Err(PolarError::NotFound(polar_id).into())
        } else {
            let path = self.polars_dir.join(format!("{}.yaml", polar_id));
            if path.exists() {
                Err(PolarError::AlreadyExists(polar_id).into())
            } else {
                Self::rename(&archived, &path)
            }
        }
    }

    fn rename(from: &Path, to: &Path) -> Result<()> {
        match fs::rename(from, to) {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("Error moving file {:?} to {:?} : {}", from, to, e);
                Err(e.into())
            }
        }
    }

    fn save_polar(&self, path: &Path, polar: &Polar) -> Result<()> {

        let f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        serde_yaml::to_writer(f, polar)?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum PolarError {
    #[error("Polar {0} already exists.")]
    AlreadyExists(String),
    #[error("Polar {0} does not exist.")]
    NotFound(String),
    #[error("Id is mandatory")]
    IdIsMandatory(),
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Polar {
    pub(crate) id: Option<String>,
    #[serde(rename = "_id")]
    pub(crate) polar_id: u8,
    #[serde(default, skip_serializing)]
    pub(crate) archived: bool,
    pub(crate) label: String,
    pub(crate) global_speed_ratio: f64,
    pub(crate) ice_speed_ratio: f64,
    pub(crate) auto_sail_change_tolerance: f64,
    pub(crate) bad_sail_tolerance: f64,
    pub(crate) max_speed: f64,
    pub(crate) foil: Foil,
    pub(crate) hull: Hull,
    pub(crate) winch: Winch,
    pub(crate) tws: Vec<u8>,
    pub(crate) twa: Vec<u8>,
    pub(crate) sail: Vec<Sail>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Foil {
    pub(crate) speed_ratio: f64,
    pub(crate) twa_min: f64,
    pub(crate) twa_max: f64,
    pub(crate) twa_merge: f64,
    pub(crate) tws_min: f64,
    pub(crate) tws_max: f64,
    pub(crate) tws_merge: f64,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Hull {
    pub(crate) speed_ratio: f64,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Winch {
    pub(crate) tack: PenaltyCase,
    pub(crate) gybe: PenaltyCase,
    pub(crate) sail_change: PenaltyCase,
    pub(crate) lws: u8,
    pub(crate) hws: u8,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenaltyCase {
    pub(crate) std_timer_sec: u16,
    pub(crate) std_ratio: f64,
    pub(crate) pro_timer_sec: u16,
    pub(crate) pro_ratio: f64,
    pub(crate) std: PenaltyBoundaries
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenaltyBoundaries {
    pub(crate) lw: Penalty,
    pub(crate) hw: Penalty,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Penalty {
    pub(crate) ratio: f64,
    pub(crate) timer: u16
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sail {
    pub(crate) id: u8,
    pub(crate) name: String,
    pub(crate) speed: Vec<Vec<f64>>
}