use failure::Fail;

#[derive(Debug, Fail)]
pub enum LogError {
    #[fail(display = "Not a valid FD")]
    FdError,
    #[fail(display = "Log error")]
    LoggingError,
}
