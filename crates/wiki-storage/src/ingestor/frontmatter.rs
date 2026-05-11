use std::path::Path;

use serde::Deserialize;
use wiki_domain::project::Frontmatter;

// TODO Support TOML and +++
const DELIMITER: &str = "---";

#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing closing delimiter (frontmatter block not terminated with `---`)")]
    NoClosingDelimiter,
    #[error("invalid YAML at line {line}, col {column}: {message}")]
    Yaml { message: String, line: usize, column: usize },
}

#[derive(Debug, Deserialize)]
struct RawFrontmatter {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

pub fn read_frontmatter(path: &Path) -> Result<Option<Frontmatter>, FrontmatterError> {
    let text = std::fs::read_to_string(path)?;
    parse_frontmatter(&text)
}

pub fn parse_frontmatter(content: &str) -> Result<Option<Frontmatter>, FrontmatterError> {
    let mut lines = content.lines();

    let Some(first) = lines.next() else {
        return Ok(None);
    };
    if first.trim_end() != DELIMITER {
        return Ok(None);
    }

    let mut body = String::new();
    let mut closed = false;
    for line in lines {
        if line.trim_end() == DELIMITER {
            closed = true;
            break;
        }
        body.push_str(line);
        body.push('\n');
    }

    if !closed {
        return Err(FrontmatterError::NoClosingDelimiter);
    }

    let raw: RawFrontmatter = serde_yml::from_str(&body).map_err(|e| {
        let loc = e.location();
        FrontmatterError::Yaml {
            message: e.to_string(),
            line: loc.as_ref().map(|l| l.line() + 1).unwrap_or(0),
            column: loc.as_ref().map(|l| l.column()).unwrap_or(0),
        }
    })?;

    Ok(Some(Frontmatter {
        id: raw.id.unwrap_or_default(),
        title: raw.title.unwrap_or_default(),
        icon: raw.icon,
    }))
}
