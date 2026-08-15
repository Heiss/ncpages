//! Status reporting.
//!
//! Moving off a CI service costs a dashboard: it used to write failures in your
//! face, a dead background service says nothing at all. This is the channel that
//! reaches the operator when nobody is looking, which is the normal state of a
//! personal site that has been publishing fine for months.

use anyhow::Result;
use tracing::{debug, warn};

use crate::config::Config;
use crate::pipeline::Outcome;

/// Notify on anything that needs a human: a failed build, or a build the gate
/// refused. Successful publishes stay quiet on purpose — a notification per blog
/// post trains you to ignore the channel.
pub async fn notify(config: &Config, result: &Result<Outcome>) {
    let Some(topic) = config.report.ntfy_topic.as_deref() else {
        return;
    };

    let (title, priority, body) = match result {
        Err(e) => (
            "ncpages build failed",
            "high",
            format!("{e}\n\nThe live site is unchanged."),
        ),
        Ok(outcome) if !outcome.violations.is_empty() => (
            "ncpages gate refused a build",
            "high",
            format!(
                "trigger: {}\n{}\n\nThe live site is unchanged.",
                outcome.trigger,
                outcome.violations.join("\n")
            ),
        ),
        Ok(outcome) if !outcome.conflict_copies.is_empty() => (
            "ncpages found conflict copies",
            "default",
            format!(
                "trigger: {}\nExcluded from the build, and a sign that work is about to be lost:\n{}",
                outcome.trigger,
                outcome.conflict_copies.join("\n")
            ),
        ),
        Ok(_) => return,
    };

    if let Err(e) = post(topic, title, priority, &body).await {
        // A failed notification must never fail a build that already succeeded.
        warn!(error = %e, "could not send notification");
    } else {
        debug!(topic, "notification sent");
    }
}

async fn post(topic: &str, title: &str, priority: &str, body: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    client
        .post(topic)
        .header("Title", title)
        .header("Priority", priority)
        .body(body.to_string())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(topic: Option<&str>) -> Config {
        let mut config: Config = toml::from_str(
            r#"
            schema_version = 1
            [source]
            kind = "fs"
            path = "/tmp/vault"
            [build]
            kind = "local"
            command = ["true"]
            "#,
        )
        .unwrap();
        config.report.ntfy_topic = topic.map(str::to_string);
        config
    }

    fn outcome() -> Outcome {
        Outcome {
            trigger: "poll".into(),
            release: None,
            pages: 0,
            published: true,
            skipped: false,
            warnings: vec![],
            violations: vec![],
            conflict_copies: vec![],
        }
    }

    #[tokio::test]
    async fn without_a_topic_nothing_is_attempted() {
        notify(&config(None), &Ok(outcome())).await;
    }

    #[tokio::test]
    async fn a_successful_publish_is_not_announced() {
        // An unreachable topic would log a warning if a request were made; a
        // successful publish must return before that.
        notify(&config(Some("http://127.0.0.1:1/never")), &Ok(outcome())).await;
    }
}
