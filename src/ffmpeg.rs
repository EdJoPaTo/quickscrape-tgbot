use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use lazy_regex::regex;

pub struct VideoStats {
    pub height: u32,
    pub width: u32,
    /// In seconds
    pub duration: u32,
}

impl VideoStats {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let output = Command::new("ffprobe")
            .arg("-hide_banner")
            .arg(path.as_os_str())
            .output()
            .expect("failed to execute ffprobe");
        let output = String::from_utf8(output.stderr).expect("ffprobe provided non utf8 output");

        let duration = {
            let captures = regex!(r"Duration: (\d{2}):(\d{2}):(\d{2})\.")
                .captures(&output)
                .context("duration not found in ffprobe output")?;
            let hours = captures.get(1).unwrap().as_str().parse::<u32>().unwrap();
            let minutes = captures.get(2).unwrap().as_str().parse::<u32>().unwrap();
            let seconds = captures.get(3).unwrap().as_str().parse::<u32>().unwrap();
            (((hours * 60) + minutes) * 60) + seconds
        };

        let (width, height) = {
            let captures = regex!(r", (\d+)x(\d+) \[")
                .captures(&output)
                .context("resolution not found in ffprobe output")?;
            let width = captures.get(1).unwrap().as_str().parse().unwrap();
            let height = captures.get(2).unwrap().as_str().parse().unwrap();
            (width, height)
        };

        Ok(Self {
            height,
            width,
            duration,
        })
    }
}
