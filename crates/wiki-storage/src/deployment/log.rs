use std::cell::RefCell;
use std::io;
use std::path::PathBuf;

use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::{Id, Subscriber};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::fmt::format::{DefaultFields, Writer};
use tracing_subscriber::fmt::writer::{EitherWriter, MakeWriter};
use tracing_subscriber::fmt::{FormatFields, FormattedFields};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, fmt};

use crate::store;

pub const DEPLOYMENT_LOG_FILE: &str = "deployment.log";

pub const DEPLOYMENT_SPAN: &str = "deployment";

const LOG_FILE_PREFIX: &str = "deployment";
const LOG_FILE_SUFFIX: &str = "log";

pub const DEFAULT_LOG_FILTER: &str =
    "info,wiki_storage=debug,wiki_db=debug,wiki_external=debug,wiki_projects=debug";

thread_local! {
    static ACTIVE: RefCell<Vec<NonBlocking>> = const { RefCell::new(Vec::new()) };
}

pub fn layer<S>(storage_root: PathBuf) -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let fmt_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .fmt_fields(PlainFields::default())
        .with_writer(DeploymentWriter)
        .with_filter(EnvFilter::new(DEFAULT_LOG_FILTER));

    DeploymentSpanLayer { storage_root }.and_then(fmt_layer)
}

struct DeploymentSpanLayer {
    storage_root: PathBuf,
}

struct LogHandle {
    writer: NonBlocking,
    _guard: WorkerGuard,
}

impl<S> Layer<S> for DeploymentSpanLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().name() != DEPLOYMENT_SPAN {
            return;
        }

        let mut visitor = DeploymentVisitor::default();
        attrs.record(&mut visitor);
        let (Some(project_id), Some(deployment_id)) = (visitor.project_id, visitor.deployment_id)
        else {
            return;
        };

        let dir = store::deployment_root(&self.storage_root, &project_id, &deployment_id);
        let appender = RollingFileAppender::builder()
            .rotation(Rotation::NEVER)
            .filename_prefix(LOG_FILE_PREFIX)
            .filename_suffix(LOG_FILE_SUFFIX)
            .build(&dir);
        let Ok(appender) = appender else {
            return;
        };

        let (writer, guard) = tracing_appender::non_blocking(appender);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(LogHandle {
                writer,
                _guard: guard,
            });
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(writer) = span_writer(id, &ctx) {
            ACTIVE.with_borrow_mut(|active| active.push(writer));
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if span_writer(id, &ctx).is_some() {
            ACTIVE.with_borrow_mut(|active| active.pop());
        }
    }
}

fn span_writer<S>(id: &Id, ctx: &Context<'_, S>) -> Option<NonBlocking>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    ctx.span(id)?.scope().find_map(|span| {
        span.extensions()
            .get::<LogHandle>()
            .map(|handle| handle.writer.clone())
    })
}

#[derive(Default)]
struct PlainFields(DefaultFields);

impl<'writer> FormatFields<'writer> for PlainFields {
    fn format_fields<R: RecordFields>(
        &self,
        writer: Writer<'writer>,
        fields: R,
    ) -> std::fmt::Result {
        self.0.format_fields(writer, fields)
    }

    fn add_fields(
        &self,
        current: &'writer mut FormattedFields<Self>,
        fields: &tracing::span::Record<'_>,
    ) -> std::fmt::Result {
        let mut plain = FormattedFields::<DefaultFields>::new(current.fields.clone());
        self.0.add_fields(&mut plain, fields)?;
        current.fields = plain.fields;
        Ok(())
    }
}

struct DeploymentWriter;

impl<'a> MakeWriter<'a> for DeploymentWriter {
    type Writer = EitherWriter<NonBlocking, io::Sink>;

    fn make_writer(&'a self) -> Self::Writer {
        ACTIVE.with_borrow(|active| match active.last() {
            Some(writer) => EitherWriter::A(writer.clone()),
            None => EitherWriter::B(io::sink()),
        })
    }
}

#[derive(Default)]
struct DeploymentVisitor {
    project_id: Option<String>,
    deployment_id: Option<String>,
}

impl DeploymentVisitor {
    fn set(&mut self, name: &str, value: String) {
        match name {
            "project_id" => self.project_id = Some(value),
            "deployment_id" => self.deployment_id = Some(value),
            _ => {}
        }
    }
}

impl Visit for DeploymentVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.set(field.name(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.set(
            field.name(),
            format!("{value:?}").trim_matches('"').to_owned(),
        );
    }
}
