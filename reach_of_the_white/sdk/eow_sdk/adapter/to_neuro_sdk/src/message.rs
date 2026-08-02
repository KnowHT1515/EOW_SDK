#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeuroBatch {
    pub sdk_id: String,
    pub messages: Vec<String>,
}

impl NeuroBatch {
    pub(crate) fn new(sdk_id: &str, messages: Vec<String>) -> Self {
        Self {
            sdk_id: sdk_id.to_owned(),
            messages,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}
