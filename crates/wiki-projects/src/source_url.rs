use url::Url;

const GIT_SUFFIX: &str = ".git";
const WWW_PREFIX: &str = "www.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceUrl {
    scheme: String,
    host: String,
    path: String,
}

impl SourceUrl {
    pub fn parse(raw: &str) -> Option<Self> {
        let url = Url::parse(raw.trim()).ok().filter(Url::has_host)?;
        let host = url
            .host_str()?
            .trim_start_matches(WWW_PREFIX)
            .to_lowercase();
        if host.is_empty() {
            return None;
        }

        let path = url
            .path_segments()?
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/")
            .to_lowercase();
        let path = match path.strip_suffix(GIT_SUFFIX) {
            Some(stripped) if !stripped.is_empty() => stripped,
            _ => &path,
        };
        if path.is_empty() {
            return None;
        }

        Some(Self {
            scheme: url.scheme().to_owned(),
            host,
            path: path.to_owned(),
        })
    }
}
