use crate::ServerState;
use anyhow::{bail, Context, Result};

use re_sdk::RecordingStream;
use re_types::{archetypes, datatypes};

use ndarray::Array3;
use tracing::debug;
use v4l::buffer::Type;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::Capture;
use v4l::FourCC;

struct LidarCapture;
pub struct DepthMap;

// TODO: maybe a more appropriate name
pub struct FrameCapture {
    pub rgb: Option<Array3<u8>>,
    pub depth: DepthMap,
    camera: Device,
    lidar: LidarCapture,
}

pub struct CameraSettings {
    pub device: usize,
}
pub struct LidarSettings;

impl Default for CameraSettings {
    fn default() -> Self {
        Self { device: 0 }
    }
}

impl FrameCapture {
    pub fn new(camera_settings: CameraSettings, lidar_settings: LidarSettings) -> Result<Self> {
        let mut dev =
            v4l::Device::new(camera_settings.device).context("failed to initialise camera")?;
        // Get the current format and then modify it to BGR3.
        let mut fmt = dev.format().context("failed to get format")?;
        fmt.fourcc = FourCC::new(b"BGR3");

        // try set the format, otherwise fail - should work since we are using libv4l
        let resulting_format = dev
            .set_format(&fmt)
            .context("failed to set camera format")?;
        if resulting_format.fourcc.repr != *b"BGR3" {
            bail!("failed to set pixel format to BGR3");
        }
        Ok(Self {
            rgb: None,
            depth: DepthMap {},
            camera: dev,
            lidar: LidarCapture {},
        })
    }
    /// Fetch data from the sensors
    pub fn fetch_frame(&mut self) -> Result<()> {
        // TODO: reuse stream
        let mut stream =
            v4l::io::mmap::Stream::with_buffers(&mut self.camera, Type::VideoCapture, 4)
                .context("failed to create buffer stream")?;

        // Capture one frame.
        let (buf, _meta) = stream.next().context("failed to capture frame")?;

        // Retrieve the current format to determine frame dimensions.
        let fmt = self.camera.format().context("failed to get format")?;
        let width = fmt.width as usize;
        let height = fmt.height as usize;
        let expected_size = width * height * 3; // 3 channels (RGB)
        if buf.len() != expected_size {
            anyhow::bail!("captured buffer is not the expected size");
        }

        // TODO: see if we can do without ndarray
        // Convert the raw buffer into an ndarray.
        // Note that ndarray’s shape is (height, width, channels).
        let arr = Array3::from_shape_vec((height, width, 3), buf[..expected_size].to_vec())
            .context("failed to create ndarray from frame data")?;
        self.rgb = Some(arr);
        Ok(())
    }

    /// Process the current frame
    pub fn process_frame(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn log(&self, rec: &RecordingStream) -> Result<()> {
        if let Some(ref rgb) = self.rgb {
            let image = archetypes::Image::from_color_model_and_tensor(
                datatypes::ColorModel::BGR,
                rgb.clone(),
            )
            .expect("failed to convert frame to archetype image");
            rec.log("world/camera/rgb", &image)?;
        }
        Ok(())
    }
}
