use crate::command::ClipboardPayload;

pub trait ClipboardContext: Send + Sync {
    fn latest_payload(&self) -> Option<ClipboardPayload>;
}
