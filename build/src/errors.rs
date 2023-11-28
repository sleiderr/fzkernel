use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct BuildError(pub Option<String>);

impl std::error::Error for BuildError {}

impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(msg) = &self.0 {
            write!(f, "{}", msg.as_str());
        }
        Ok(())
    }
}
