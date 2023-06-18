use thiserror::Error;

use crate::unit::Msg;

#[derive(Error, Debug)]
pub enum ApgpkError {
    #[error("PGP lib Error")]
    PgpLibError(#[from] pgp::errors::Error),
    #[error("IO Error")]
    IoError(#[from] std::io::Error),
    #[error("MPSC Error")]
    MpscError(#[from] std::sync::mpsc::SendError<Msg>),
    #[error("Other Error: {0}")]
    Other(String),
}
