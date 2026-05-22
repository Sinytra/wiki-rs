use tracing::error;

pub const BUILTIN_PROJECT_ID: &str = "minecraft";

pub trait LogErr<T, E> {
    fn log_err(self, msg: &str);

    fn inspect_err_log(self, msg: &str) -> Result<T, E>;

    fn map_err_log<U, F>(self, msg: &str, f: F) -> Result<T, U>
    where
        F: FnOnce(E) -> U;
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

    #[inline]
    fn map_err_log<U, F>(self, msg: &str, f: F) -> Result<T, U>
    where
        F: FnOnce(E) -> U,
    {
        self.map_err(|e| {
            error!(error = %e, "{msg}");
            f(e)
        })
    }
}
