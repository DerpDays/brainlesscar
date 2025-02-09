use crate::ServerState;
use anyhow::{Context, Result};

use ndarray::Array3;
use opencv::{
    prelude::*,
    videoio::{self, VideoCapture},
};
use re_sdk::RecordingStream;
use re_types::{archetypes, datatypes};

struct LidarCapture;
pub struct DepthMap;

// TODO: maybe a more appropriate name
pub struct FrameCapture {
    pub rgb: Mat,
    pub depth: DepthMap,
    camera: VideoCapture,
    lidar: LidarCapture,
}

struct LoggableMat(Array3<u8>);
impl From<LoggableMat> for archetypes::Image {
    fn from(value: LoggableMat) -> Self {
        archetypes::Image::from_color_model_and_tensor(datatypes::ColorModel::BGR, value.0)
            .expect("image color model doesn't match data")
    }
}
// TODO: make this better by not getting the size every time
impl From<&Mat> for LoggableMat {
    fn from(value: &Mat) -> Self {
        let mat_size = value.mat_size();
        let mut size_it = mat_size.iter();
        let width = *size_it.next().expect("matrix cannot be 0d") as usize;
        let height = *size_it.next().expect("matrix cannot be 1d") as usize;
        let data = value.data_bytes().expect("camera matrix is not continous");
        Self(Array3::from_shape_vec((width, height, 3), data.to_vec()).expect("a"))
    }
}

pub struct CameraSettings {
    pub device: i32,
    /// OpenCV capture type i.e. videoio::CAP_V4L2
    pub cap: i32,
}
pub struct LidarSettings;

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            device: 0,
            cap: videoio::CAP_V4L2,
        }
    }
}

impl FrameCapture {
    pub fn new(camera_settings: CameraSettings, lidar_settings: LidarSettings) -> Result<Self> {
        let cam = VideoCapture::new(camera_settings.device, camera_settings.cap)
            .context("failed to initialise camera")?;
        // check the camera actually opened
        cam.is_opened()?
            .then_some(())
            .context("camera device failed to open")?;
        let rgb_mat = Mat::default();
        Ok(Self {
            rgb: rgb_mat,
            depth: DepthMap {},
            camera: cam,
            lidar: LidarCapture {},
        })
    }
    /// Fetch data from the sensors
    pub fn fetch_frame(&mut self) -> Result<()> {
        if !self
            .camera
            .read(&mut self.rgb)
            .context("failed to read from camera")?
        {
            anyhow::bail!("failed to read from camera");
        };

        Ok(())
    }
    /// Process the current frame
    pub fn process_frame(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn log(&self, rec: &RecordingStream) -> Result<()> {
        rec.log(
            "world/camera/rgb",
            &archetypes::Image::from(LoggableMat::from(&self.rgb)),
        )?;
        Ok(())
    }
}
