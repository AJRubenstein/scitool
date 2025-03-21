use input::InputState;
use output::OutputState;
use smol::{io::AsyncBufReadExt, stream::StreamExt};

mod input;
mod output;

pub use input::{Input, ReaderInput};
pub use output::Output;

pub trait ProgressListener {
    fn on_progress(&mut self, done: bool, progress_info: Vec<(String, String)>);
}

pub struct NullProgressListener;

impl ProgressListener for NullProgressListener {
    fn on_progress(&mut self, _done: bool, _progress_info: Vec<(String, String)>) {}
}

pub struct FfmpegTool {
    binary_path: std::path::PathBuf,
}

impl FfmpegTool {
    pub fn from_path(path: std::path::PathBuf) -> Self {
        FfmpegTool { binary_path: path }
    }

    pub async fn convert<I, O>(
        &self,
        input: I,
        output: O,
        progress: &mut dyn ProgressListener,
    ) -> anyhow::Result<()>
    where
        I: Input,
        O: Output,
    {
        let mut command = smol::process::Command::new(&self.binary_path);
        let input_state = input.create_state().await?;
        let output_state = output.create_state().await?;
        let mut child = command
            .arg("-nostdin")
            .arg("-progress")
            .arg("pipe:1")
            .arg("-i")
            .arg(input_state.url())
            .arg(output_state.url())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let stdout = smol::io::BufReader::new(child.stdout.take().expect("Failed to create pipe."));
        let status = futures::join!(
            child.status(),
            input_state.wait(),
            output_state.wait(),
            async move {
                let mut lines = stdout.lines();
                let mut progress_info = Vec::new();
                while let Some(line) = lines.next().await {
                    let line = line.unwrap();
                    if let Some(eq_index) = line.find('=') {
                        let key = &line[..eq_index];
                        let value = &line[eq_index + 1..];
                        if key == "progress" {
                            let done = value.trim() == "end";
                            progress.on_progress(done, progress_info);
                            progress_info = Vec::new();
                        } else {
                            let value = value.trim().to_string();
                            progress_info.push((key.to_string(), value));
                        }
                    }
                }
            }
        )
        .0?;

        anyhow::ensure!(
            status.success(),
            "ffmpeg process exited with non-zero status: {}",
            status
        );

        Ok(())
    }
}
