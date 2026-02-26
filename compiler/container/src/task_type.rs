use crate::ContainerError;

/// Type tags for task scheduling types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskType {
    Cyclic = 0,
    Event = 1,
    Freewheeling = 2,
}

impl TaskType {
    pub(crate) fn from_u8(v: u8) -> Result<Self, ContainerError> {
        match v {
            0 => Ok(TaskType::Cyclic),
            1 => Ok(TaskType::Event),
            2 => Ok(TaskType::Freewheeling),
            _ => Err(ContainerError::InvalidTaskType(v)),
        }
    }

    /// Returns the human-readable name for this task type.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Cyclic => "Cyclic",
            TaskType::Event => "Event",
            TaskType::Freewheeling => "Freewheeling",
        }
    }
}
