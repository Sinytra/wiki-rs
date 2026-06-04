use discord_message::{DiscordMessage, Embed, EmbedField, EmbedFooter};

use crate::error::ExternalResult;

const COLOR_CREATED: u32 = 0x00FF00;
const COLOR_DELETED: u32 = 0xFF0000;
const COLOR_REPORT: u32 = 0xFFA500;

pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub project_type: String,
    pub source_repo: String,
    pub source_branch: String,
    pub source_path: String,
    pub platforms: Vec<(String, String)>,
    pub user: String,
    pub created_at: String,
}

pub struct ReportNotification {
    pub report_type: String,
    pub reason: String,
    pub submitter_id: String,
    pub project_id: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct DiscordService {
    http: reqwest::Client,
    webhook_url: Option<String>,
}

impl DiscordService {
    pub fn new(http: reqwest::Client, webhook_url: Option<String>) -> Self {
        let webhook_url = webhook_url.filter(|u| !u.trim().is_empty());
        Self { http, webhook_url }
    }

    pub fn is_enabled(&self) -> bool {
        self.webhook_url.is_some()
    }

    pub async fn project_created(&self, project: &ProjectInfo) -> ExternalResult<()> {
        let embed = build_project_embed(project, ":book: Project created", COLOR_CREATED);
        self.send(embed).await
    }

    pub async fn project_deleted(&self, project: &ProjectInfo) -> ExternalResult<()> {
        let embed = build_project_embed(project, ":wastebasket: Project deleted", COLOR_DELETED);
        self.send(embed).await
    }

    pub async fn report_created(&self, report: &ReportNotification) -> ExternalResult<()> {
        let embed = build_report_embed(report);
        self.send(embed).await
    }

    async fn send(&self, embed: Embed) -> ExternalResult<()> {
        let Some(url) = self.webhook_url.as_deref() else {
            return Ok(());
        };

        let message = DiscordMessage {
            embeds: vec![embed],
            ..Default::default()
        };
        let body = message.to_json()?;

        self.http
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

fn build_report_embed(report: &ReportNotification) -> Embed {
    let fields = vec![
        EmbedField {
            title: "Type".to_owned(),
            value: report.report_type.clone(),
            inline: true,
        },
        EmbedField {
            title: "Reason".to_owned(),
            value: report.reason.clone(),
            inline: true,
        },
        EmbedField {
            title: "Submitter".to_owned(),
            value: format!("`{}`", report.submitter_id),
            inline: true,
        },
        EmbedField {
            title: "Project".to_owned(),
            value: format!("`{}`", report.project_id),
            inline: true,
        },
    ];

    Embed {
        title: ":triangular_flag_on_post: New report".to_owned(),
        description: String::new(),
        color: Some(COLOR_REPORT),
        fields: Some(fields),
        footer: Some(EmbedFooter {
            text: format!("Created at {}", report.created_at),
            icon_url: None,
        }),
        ..Default::default()
    }
}

fn build_project_embed(project: &ProjectInfo, title: &str, color: u32) -> Embed {
    let repo_link = project.source_repo.to_owned();
    let description = format!("**{}** (`{}`)\n{}", project.name, project.id, repo_link);

    let mut fields = vec![
        EmbedField {
            title: "Type".to_owned(),
            value: project.project_type.clone(),
            inline: true,
        },
        EmbedField {
            title: "User".to_owned(),
            value: format!("`{}`", project.user),
            inline: true,
        },
        EmbedField {
            title: "Branch".to_owned(),
            value: format!("`{}`", project.source_branch),
            inline: true,
        },
        EmbedField {
            title: "Path".to_owned(),
            value: format!("`{}`", project.source_path),
            inline: true,
        }
    ];

    if !project.platforms.is_empty() {
        let value = project
            .platforms
            .iter()
            .map(|(platform, id)| format!("{platform}: `{id}`"))
            .collect::<Vec<_>>()
            .join("\n");
        fields.push(EmbedField {
            title: "Platforms".to_owned(),
            value,
            inline: false,
        });
    }

    Embed {
        title: title.to_owned(),
        description,
        color: Some(color),
        fields: Some(fields),
        footer: Some(EmbedFooter {
            text: format!("Created at {}", project.created_at),
            icon_url: None,
        }),
        ..Default::default()
    }
}
