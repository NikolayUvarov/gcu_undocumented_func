use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockDescriptor {
    pub kind: String,
    pub source_activation: String,
    pub dtype: String,
    pub layout: String,
    pub state: String,
    pub parameter_sharing: String,
}

impl BlockDescriptor {
    pub fn check(&self, device: &str) -> Result<()> {
        for (field, actual, supported) in [
            ("block kind", self.kind.as_str(), "gated-feed-forward/0"),
            ("source activation", self.source_activation.as_str(), "silu"),
            ("dtype", self.dtype.as_str(), "f64"),
            ("layout", self.layout.as_str(), "row-major"),
            ("state", self.state.as_str(), "stateless"),
            (
                "parameter sharing",
                self.parameter_sharing.as_str(),
                "independent",
            ),
            ("device", device, "cpu-scalar"),
        ] {
            if actual != supported {
                return Err(format!(
                    "unsupported {field}: {actual}; this profile requires {supported}"
                ));
            }
        }
        Ok(())
    }
}
