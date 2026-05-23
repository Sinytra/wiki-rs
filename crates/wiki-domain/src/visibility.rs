use sea_orm::prelude::StringLen;
use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    AsRefStr,
    EnumIter,
    DeriveActiveEnum,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "snake_case"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ProjectVisibility {
    Public,
    Unlisted,
    Private,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, AsRefStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ProjectFlag {
    Unpublished,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, AsRefStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ProjectStatus {
    Loading,
    Healthy,
    AtRisk,
    Error,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    AsRefStr,
    EnumIter,
    DeriveActiveEnum,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "snake_case"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ReportStatus {
    New,
    Accepted,
    Dismissed,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, AsRefStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ReportResolution {
    Accept,
    Dismiss,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    AsRefStr,
    EnumString,
    EnumIter,
    DeriveActiveEnum,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "snake_case"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ReportType {
    Project,
    Docs,
    Content,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    AsRefStr,
    EnumString,
    EnumIter,
    DeriveActiveEnum,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "snake_case"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ReportReason {
    Spam,
    Copyright,
    ContentRules,
    Tos,
}
