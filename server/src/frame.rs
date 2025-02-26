use std::io::Cursor;

use anyhow::{bail, Context, Result};
use ndarray::Array3;
use re_sdk::{external::arrow::datatypes::ToByteSlice, RecordingStream};
use re_types::{archetypes, datatypes};
use tracing::{debug, instrument, trace};
use v4l::buffer::Type;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::Capture;
use v4l::Device;

// Import libbayer types and functions.
use bayer::{run_demosaic, BayerDepth, Demosaic, RasterMut, CFA};

struct LidarCapture;
pub struct DepthMap;

pub struct FrameCapture {
    pub rgb: Option<Array3<u8>>,
    pub depth: DepthMap,
    camera: Device,
    lidar: LidarCapture,
}

#[derive(Clone, Copy, Debug)]
pub struct CameraSettings {
    pub device: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct LidarSettings;

impl Default for CameraSettings {
    fn default() -> Self {
        Self { device: 0 }
    }
}

impl FrameCapture {
    #[instrument]
    pub fn new(camera_settings: CameraSettings, _lidar_settings: LidarSettings) -> Result<Self> {
        debug!("initialising camera");
        // Open the camera; we expect it to output raw Bayer (e.g. "RG10") data.
        let dev =
            v4l::Device::new(camera_settings.device).context("failed to initialise camera")?;
        // For raw Bayer processing, we leave the native format untouched.
        Ok(Self {
            rgb: None,
            depth: DepthMap {},
            camera: dev,
            lidar: LidarCapture {},
        })
    }

    /// Fetch a frame and run demosaicing using libbayer.
    #[instrument(skip_all)]
    pub fn fetch_frame(&mut self) -> Result<()> {
        trace!("fetching frame");
        // Create a memory-mapped stream.
        let mut stream =
            v4l::io::userptr::Stream::with_buffers(&mut self.camera, Type::VideoCapture, 4)
                .context("failed to create buffer stream")?;

        // Capture one frame.
        let (buf, _meta) = stream.next().context("failed to capture frame")?;

        // Retrieve the camera’s current format to get dimensions.
        let fmt = self.camera.format().context("failed to get format")?;
        let width = fmt.width as usize;
        let height = fmt.height as usize;

        // For raw Bayer (10-bit) data stored as 16-bit words, we expect the buffer length to be:
        // width * height * 2 bytes.
        if buf.len() != width * height * 2 {
            bail!(
                "captured buffer size ({}) does not match expected raw Bayer size ({} bytes)",
                buf.len(),
                width * height * 2
            );
        }

        // Interpret the raw bytes as u16 values.
        let raw_pixels: &[u16] =
            unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u16, buf.len() / 2) };

        // Convert the 10-bit values into 8-bit values.
        // (A simple way is to shift right by 2 bits.)
        let input_data: Vec<u8> = raw_pixels.iter().map(|&x| (x >> 2) as u8).collect();

        // Prepare an output buffer for the demosaiced image (3 channels, 8 bits each).
        let mut demosaic_buf = vec![0u8; 3 * width * height];
        let mut dst = RasterMut::new(width, height, bayer::RasterDepth::Depth8, &mut demosaic_buf);

        // Run the demosaicing algorithm.
        // Here we use a linear demosaicing method on an 8-bit input with an RGGB CFA.
        run_demosaic(
            &mut Cursor::new(input_data),
            BayerDepth::Depth8,
            CFA::RGGB,
            Demosaic::Linear,
            &mut dst,
        )
        .context("demosaicing failed")?;

        // Convert the output buffer into an ndarray with shape (height, width, 3).
        let arr = Array3::from_shape_vec((height, width, 3), demosaic_buf)
            .context("failed to create ndarray from demosaiced data")?;
        self.rgb = Some(arr);
        Ok(())
    }

    /// Process the current frame (placeholder for further processing).
    #[instrument(skip_all)]
    pub fn process_frame(&mut self) -> Result<()> {
        Ok(())
    }

    /// Log the current frame via the recording stream.
    #[instrument(skip_all)]
    pub fn log(&self, rec: &RecordingStream) -> Result<()> {
        if let Some(ref rgb) = self.rgb {
            // Convert the ndarray to an archetype image (expecting BGR color order).
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
