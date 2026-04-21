use crate::ContainerError;

/// Type tags for task scheduling types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskType {
    #[default]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_type_from_u8_when_valid_tags_then_returns_variant() {
        assert_eq!(TaskType::from_u8(0).unwrap(), TaskType::Cyclic);
        assert_eq!(TaskType::from_u8(1).unwrap(), TaskType::Event);
        assert_eq!(TaskType::from_u8(2).unwrap(), TaskType::Freewheeling);
    }

    #[test]
    fn task_type_from_u8_when_invalid_tag_then_returns_error() {
        assert!(matches!(
            TaskType::from_u8(99),
            Err(ContainerError::InvalidTaskType(99))
        ));
    }
}
