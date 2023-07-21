use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    EchoSent,
    ReadySent,
    DeliveredWithReadySent,
    Delivered,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::EchoSent => write!(f, "EchoSent"),
            Self::ReadySent => write!(f, "ReadySent"),
            Self::DeliveredWithReadySent => write!(f, "DeliveredWithReadySent"),
            Self::Delivered => write!(f, "Delivered"),
        }
    }
}

impl Status {
    pub(crate) fn is_ready_sent(&self) -> bool {
        matches!(self, Self::ReadySent) || matches!(self, Self::DeliveredWithReadySent)
    }

    pub(crate) fn is_delivered(&self) -> bool {
        matches!(self, Self::Delivered) || matches!(self, Self::DeliveredWithReadySent)
    }

    pub(crate) fn ready_sent(self) -> Self {
        match self {
            Self::EchoSent => Self::ReadySent,
            Self::Delivered => Self::DeliveredWithReadySent,
            _ => self,
        }
    }

    pub(crate) fn delivered(self) -> Self {
        match self {
            Self::ReadySent => Self::DeliveredWithReadySent,
            _ => Self::Delivered,
        }
    }
}
