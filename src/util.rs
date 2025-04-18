//! Convenience functions for usage

use once_cell::sync::Lazy;
use tracing_subscriber::{
    fmt::{self, format::Writer, FmtContext, FormatEvent, FormatFields},
    registry::LookupSpan,
};

use tracing_log::NormalizeEvent;

use eyre::WrapErr;
use fmt::{time::FormatTime, FormattedFields};
use owo_colors::OwoColorize;
use std::{borrow::Cow, fmt::Write};
use tracing::{
    field::{Field, Visit},
    Level, Subscriber,
};

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

pub static LONG_VERSION: Lazy<String> = Lazy::new(|| {
    let version = if let Some(hash) = built_info::GIT_COMMIT_HASH {
        if let Some(true) = built_info::GIT_DIRTY {
            format!(
                "{} ({}*)",
                built_info::PKG_VERSION,
                &hash.get(..8).unwrap_or(hash)
            )
        } else {
            format!(
                "{} ({})",
                built_info::PKG_VERSION,
                &hash.get(..8).unwrap_or(hash)
            )
        }
    } else {
        built_info::PKG_VERSION.to_string()
    };
    format!(
        "{version}\nbuilt with {}\nbuild timestamp: {}",
        built_info::RUSTC_VERSION,
        built_info::BUILT_TIME_UTC
    )
});

struct ColorLevel<'a>(&'a Level);

impl core::fmt::Display for ColorLevel<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self.0 {
            Level::TRACE => "TRACE".purple().to_string(),
            Level::DEBUG => "DEBUG".blue().to_string(),
            Level::INFO => "INFO ".green().to_string(),
            Level::WARN => "WARN ".yellow().to_string(),
            Level::ERROR => "ERROR".red().to_string(),
        }
        .fmt(f)
    }
}

struct FullCtx<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    ctx: &'a FmtContext<'a, S, N>,
    span: Option<&'a tracing::span::Id>,
}

impl<'a, S, N: 'a> FullCtx<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    pub(crate) fn new(ctx: &'a FmtContext<'a, S, N>, span: Option<&'a tracing::span::Id>) -> Self {
        Self { ctx, span }
    }
}

// TODO: This should maybe be a FormatFields instead?
struct EventFieldVisitor {
    message: String,
    message_visited: MessageState,
}

#[derive(Debug, PartialEq, Eq)]
enum MessageState {
    /// We just saw the message, so pass a "-"
    JustVisited,
    /// We haven't seen the message, so just format like usual.
    NotSeen,
    /// We have seen the message, but we've already added the "-"
    Processed,
}
impl EventFieldVisitor {
    fn record_message(&mut self, value: String) {
        if self.message.is_empty() {
            self.message = value;
            self.message_visited = MessageState::JustVisited;
        } else {
            let m = self.message.clone();
            self.message = format!("{} - {}", value, m);
            self.message_visited = MessageState::Processed;
        }
    }
}

impl Visit for EventFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
        if field.name() == "message" {
            self.record_message(format!("{:?}", value));
            return;
        }
        let s = &mut self.message;
        if !s.is_empty() {
            // message
            if self.message_visited == MessageState::JustVisited {
                let _ = write!(s, " - ");
                self.message_visited = MessageState::Processed;
            } else {
                let _ = write!(s, ", ");
            }
        }

        let _ = write!(
            s,
            "{field}: {value:?}",
            field = field.name().color(owo_colors::Rgb(128, 128, 128)),
            value = value.color(owo_colors::Rgb(128, 128, 128)).dimmed(),
        );
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() != "log.line" {
            self.record_debug(field, &value)
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        // The `Visit` API erases the string slice's lifetime. However, we
        // know it is part of the `Event` struct with a lifetime of `'a`. If
        // (and only if!) this `LogVisitor` was constructed with the same
        // lifetime parameter `'a` as the event in question, it's safe to
        // cast these string slices to the `'a` lifetime.
        match field.name() {
            "message" => self.record_message(value.to_string()),
            "log.target" | "log.module_path" | "log.file" | "log.line" => {}
            _ => self.record_debug(field, &value),
        }
    }
}

const MARKDOWN_TOKENS: &[char] = &['_', '*', '`', '~', '#', '+', '-'];

pub trait Sanitize {
    fn sanitize(&self) -> Cow<'_, str>;
}

impl<T> Sanitize for T
where
    T: AsRef<str>,
{
    fn sanitize(&self) -> Cow<'_, str> {
        let this = self.as_ref();
        if this.contains(MARKDOWN_TOKENS) {
            let mut string = this.to_string();
            for token in MARKDOWN_TOKENS {
                // FIXME: Don't replace `\{token}`
                string = string.replace(*token, &format!("\\{}", token));
            }
            Cow::Owned(string)
        } else {
            Cow::Borrowed(this)
        }
    }
}

impl<S, N> core::fmt::Display for FullCtx<'_, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut seen = false;

        let span = self
            .span
            .and_then(|id| self.ctx.span(id))
            .or_else(|| self.ctx.lookup_current());

        #[allow(deprecated)]
        let scope = span.into_iter().flat_map(|span| span.scope().from_root());

        for span in scope {
            write!(f, "{}", span.metadata().name().bold())?;
            seen = true;

            let ext = span.extensions();
            let fields = &ext
                .get::<FormattedFields<N>>()
                .expect("Unable to find FormattedFields in extensions; this is a bug");
            if !fields.is_empty() {
                write!(f, "{}{}{}", "{".bold(), fields, "}".bold())?;
            }
            f.write_char(':')?;
        }

        if seen {
            f.write_char(' ')?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Formatter;

impl<S, N> FormatEvent<S, N> for Formatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer,
        event: &tracing::Event<'_>,
    ) -> core::fmt::Result {
        // Aug 27 13:18:21.944 DEBG Getting broadcaster status, channel_id:
        tracing_subscriber::fmt::time::SystemTime.format_time(&mut writer)?;
        write!(
            writer,
            " {} {} ",
            ColorLevel(event.metadata().level()),
            "|:".black()
        )?;
        let normalized_meta = event.normalized_metadata();
        let event_meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());
        if std::path::Path::new(event_meta.file().unwrap_or("/")).is_relative() {
            write!(
                writer,
                "{}:{} ",
                event_meta.file().unwrap_or("<unknown>"),
                event_meta
                    .line()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| String::from("?"))
            )?;
        }

        write!(writer, "{} ", event_meta.target())?;
        write!(writer, "|: ")?;
        let full_ctx = FullCtx::new(ctx, event.parent());
        write!(writer, "{}\n└─\t", full_ctx)?;
        let mut fields = EventFieldVisitor {
            message: String::new(),
            message_visited: MessageState::NotSeen,
        };
        event.record(&mut fields);
        write!(writer, "{}", fields.message)?;
        //ctx.format_fields(writer, event)?;
        writeln!(writer)
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> tracing_subscriber::Layer<S> for Formatter {}
/// Build a logger that does file and term logging.
pub fn build_logger() -> Result<(), eyre::Report> {
    use tracing_subscriber::prelude::__tracing_subscriber_field_MakeExt as _;

    tracing_log::log_tracer::Builder::new()
        .init()
        .wrap_err("when building tracing builder")?;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        .add_directive("hyper=off".parse()?)
        .add_directive("sqlx=warn".parse()?)
        .add_directive("want=info".parse()?)
        .add_directive("tokio_tungstenite=info".parse()?)
        .add_directive("tungstenite=info".parse()?)
        .add_directive("reqwest=info".parse()?)
        .add_directive("mio=off".parse()?);
    let field_formatter = tracing_subscriber::fmt::format::debug_fn(|writer, field, value| {
        write!(writer, "{}: {:?}", field.name().yellow().dimmed(), value)
    })
    // Use the `tracing_subscriber::MakeFmtExt` trait to wrap the
    // formatter so that a delimiter is added between fields.
    .delimited(", ");

    let subscriber = tracing_subscriber::fmt::fmt()
        .with_target(true)
        .with_env_filter(filter)
        .event_format(Formatter)
        .fmt_fields(field_formatter)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .wrap_err("could not set global tracing logger")?;
    Ok(())
}
