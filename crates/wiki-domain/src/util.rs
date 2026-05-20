use tracing::error;

// TODO Use everywhere
pub trait LogErr<T, E> {
    fn log_err(self, msg: &str);

    fn inspect_err_log(self, msg: &str) -> Result<T, E>;
}

impl<T, E: std::fmt::Display> LogErr<T, E> for Result<T, E> {
    fn log_err(self, msg: &str) {
        if let Err(e) = self {
            error!(error = %e, "{msg}");
        }
    }

    fn inspect_err_log(self, msg: &str) -> Result<T, E> {
        if let Err(e) = &self {
            error!(error = %e, "{msg}");
        }
        self
    }
}