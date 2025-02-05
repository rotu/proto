use indicatif::{ProgressBar, ProgressStyle};
use miette::IntoDiagnostic;
use proto_core::{
    load_schema_plugin_with_proto, load_tool_from_locator, load_tool_with_proto, Id,
    ProtoEnvironment, Tool, SCHEMA_PLUGIN_KEY,
};
use starbase::Resource;
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use std::time::Duration;

pub fn enable_progress_bars() {
    env::remove_var("PROTO_NO_PROGRESS");
}

pub fn disable_progress_bars() {
    env::set_var("PROTO_NO_PROGRESS", "1");
}

pub fn create_progress_bar<S: AsRef<str>>(start: S) -> ProgressBar {
    let pb = if env::var("PROTO_NO_PROGRESS").is_ok() {
        ProgressBar::hidden()
    } else {
        ProgressBar::new_spinner()
    };

    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message(start.as_ref().to_owned());
    pb.set_style(
        ProgressStyle::with_template("{spinner:.183} {msg}")
            .unwrap()
            .tick_strings(&[
                "━         ",
                "━━        ",
                "━━━       ",
                "━━━━      ",
                "━━━━━     ",
                "━━━━━━    ",
                "━━━━━━━   ",
                "━━━━━━━━  ",
                "━━━━━━━━━ ",
                "━━━━━━━━━━",
            ]),
    );
    pb
}

#[derive(Clone, Resource)]
pub struct ProtoResource {
    pub env: Arc<ProtoEnvironment>,
}

impl ProtoResource {
    pub fn new() -> miette::Result<Self> {
        Ok(Self {
            env: Arc::new(ProtoEnvironment::new()?),
        })
    }

    pub async fn load_tool(&self, id: &Id) -> miette::Result<Tool> {
        load_tool_with_proto(id, &self.env).await
    }

    pub async fn load_tools(&self) -> miette::Result<Vec<Tool>> {
        self.load_tools_with_filters(HashSet::new()).await
    }

    pub async fn load_tools_with_filters(&self, filter: HashSet<&Id>) -> miette::Result<Vec<Tool>> {
        let config = self.env.load_config()?;

        // Download the schema plugin before loading plugins.
        // We must do this here, otherwise when multiple schema
        // based tools are installed in parallel, they will
        // collide when attempting to download the schema plugin!
        load_schema_plugin_with_proto(&self.env).await?;

        let mut futures = vec![];
        let mut tools = vec![];

        for (id, locator) in &config.plugins {
            if !filter.is_empty() && !filter.contains(id) {
                continue;
            }

            // This shouldn't be treated as a "normal plugin"
            if id == SCHEMA_PLUGIN_KEY {
                continue;
            }

            let id = id.to_owned();
            let locator = locator.to_owned();
            let proto = Arc::clone(&self.env);

            futures.push(tokio::spawn(async move {
                load_tool_from_locator(id, proto, locator).await
            }));
        }

        for future in futures {
            tools.push(future.await.into_diagnostic()??);
        }

        Ok(tools)
    }
}
