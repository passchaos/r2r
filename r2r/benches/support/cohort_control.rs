use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct Control {
    directory: PathBuf,
    label: String,
}
impl Control {
    pub fn load() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Some(directory) = std::env::var_os("R2R_COHORT_CONTROL") else {
            return Ok(None);
        };
        let label = std::env::var("R2R_COHORT_LABEL")?;
        if !matches!(label.as_str(), "baseline" | "rust") {
            return Err("invalid cohort control label".into());
        }
        Ok(Some(Self {
            directory: directory.into(),
            label,
        }))
    }
    fn wait(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(120);
        while !self.directory.join(name).exists() {
            if Instant::now() >= deadline {
                return Err("cohort control timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
    pub fn start(&self, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        self.wait(&format!("{}-{index}.go", self.label))
    }
    pub fn ready(&self, role: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(self.directory.join(format!("{}-{role}.ready", self.label)), [])?;
        Ok(())
    }
    pub fn done(&self, index: usize, role: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(
            self.directory
                .join(format!("{}-{index}-{role}.done", self.label)),
            [],
        )?;
        Ok(())
    }
    pub fn finish(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.wait("render.go")
    }
}
