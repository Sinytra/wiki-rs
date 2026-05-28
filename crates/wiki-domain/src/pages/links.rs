use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ResolvedLinkType {
    Docs,
    Vanilla,
    Content,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ResolvedLink {
    #[serde(rename = "type")]
    pub r#type: ResolvedLinkType,
    #[serde(rename = "ref")]
    pub r#ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}
