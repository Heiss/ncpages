//! The pipeline end to end against a mock Nextcloud.
//!
//! The property under test throughout: **nothing irreversible happens unless a
//! verified build is actually live.** Everything else here exists to make that
//! claim falsifiable.

mod support;

use std::path::Path;
use std::sync::Arc;

use ncpages::config::Config;
use ncpages::pipeline;
use ncpages::publish;
use ncpages::source::Source;
use ncpages::state::State;
use support::MockNextcloud;

/// Records every phase it runs in, so ordering can be asserted afterwards.
fn install_recording_hook(etc: &Path) {
    let hooks = etc.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let script = hooks.join("record.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         echo \"$1 trigger=$NCPAGES_TRIGGER release=${NCPAGES_RELEASE_DIR:-none} prev=${NCPAGES_PREV_DIR:-none}\" \
         >> \"$NCPAGES_BUILD_DIR/../phases.log\"\n",
    )
    .unwrap();
    make_executable(&script);
}

/// A stand-in generator: one HTML page per markdown file.
fn install_generator(etc: &Path) {
    let script = etc.join("build.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nset -e\nmkdir -p site\n\
         for f in docs/*.md; do [ -e \"$f\" ] || continue; n=$(basename \"$f\" .md); \
         printf '<h1>%s</h1>' \"$n\" > \"site/$n.html\"; done\n\
         printf '<h1>home</h1>' > site/index.html\n",
    )
    .unwrap();
    make_executable(&script);
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn phases(work: &Path) -> String {
    std::fs::read_to_string(work.join("phases.log")).unwrap_or_default()
}

/// Config wired to the mock, with hooks in all three watcher-side phases.
fn config_with_hooks(mock: &MockNextcloud, work: &Path, min_pages: usize) -> Config {
    let mut config = mock.config(work);
    let extra = format!(
        r#"
        [assemble]
        overlay = ["build.sh"]
        source_subdir = "docs"

        [build]
        kind = "local"
        command = ["./build.sh"]

        [gate]
        min_pages = {min_pages}
        require_files = ["index.html"]

        [[hooks.pre_build]]
        run = "record.sh"
        args = ["pre_build"]

        [[hooks.post_build]]
        run = "record.sh"
        args = ["post_build"]

        [[hooks.post_publish]]
        run = "record.sh"
        args = ["post_publish"]
        "#
    );
    // Rebuild the config with the extra sections appended.
    let base = toml::to_string(&SourceOnly::from(&config)).unwrap();
    config = toml::from_str(&format!("{base}\n{extra}")).unwrap();
    config
}

/// Serialisable projection of the parts of the mock config that must survive
/// being re-parsed with extra sections appended.
#[derive(serde::Serialize)]
struct SourceOnly {
    schema_version: u32,
    source: SourceFields,
    paths: PathFields,
    publish: PublishFields,
}

#[derive(serde::Serialize)]
struct SourceFields {
    kind: String,
    url: String,
    host_header: String,
    path: String,
    user: String,
    password_file: String,
}

#[derive(serde::Serialize)]
struct PathFields {
    src: String,
    build: String,
    state: String,
    config_dir: String,
}

#[derive(serde::Serialize)]
struct PublishFields {
    root: String,
}

impl From<&Config> for SourceOnly {
    fn from(config: &Config) -> Self {
        Self {
            schema_version: config.schema_version,
            source: SourceFields {
                kind: "webdav".into(),
                url: config.source.url.clone().unwrap(),
                host_header: config.source.host_header.clone().unwrap(),
                path: config.source.path.clone(),
                user: config.source.user.clone().unwrap(),
                password_file: config
                    .source
                    .password_file
                    .clone()
                    .unwrap()
                    .display()
                    .to_string(),
            },
            paths: PathFields {
                src: config.paths.src.display().to_string(),
                build: config.paths.build.display().to_string(),
                state: config.paths.state.display().to_string(),
                config_dir: config.paths.config_dir.display().to_string(),
            },
            publish: PublishFields {
                root: config.publish.root.display().to_string(),
            },
        }
    }
}

fn setup(work: &Path) {
    let etc = work.join("etc");
    std::fs::create_dir_all(&etc).unwrap();
    install_generator(&etc);
    install_recording_hook(&etc);
}

#[tokio::test]
async fn a_vault_becomes_a_published_site() {
    let mock = MockNextcloud::start(&[("a.md", "one"), ("b.md", "two"), ("c.md", "three")]).await;
    let work = tempfile::tempdir().unwrap();
    setup(work.path());
    let config = Arc::new(config_with_hooks(&mock, work.path(), 2));
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let outcome = pipeline::run_once(config.clone(), &source, &mut state, "manual")
        .await
        .unwrap();

    assert!(outcome.published, "{outcome:?}");
    assert_eq!(outcome.pages, 4, "three notes plus index.html");

    let current = publish::current_release(&config.publish.root).unwrap();
    assert_eq!(
        std::fs::read_to_string(current.join("a.html")).unwrap(),
        "<h1>a</h1>"
    );

    let log = phases(work.path());
    let order: Vec<&str> = log
        .lines()
        .map(|l| l.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(
        order,
        vec!["pre_build", "post_build", "post_publish"],
        "phases must run in the documented order"
    );
}

#[tokio::test]
async fn a_refused_gate_never_reaches_post_publish() {
    // The whole four-phase structure exists for this: an irreversible step must
    // not announce a state that was never published.
    let mock = MockNextcloud::start(&[("a.md", "one")]).await;
    let work = tempfile::tempdir().unwrap();
    setup(work.path());
    let config = Arc::new(config_with_hooks(&mock, work.path(), 50));
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let outcome = pipeline::run_once(config.clone(), &source, &mut state, "manual")
        .await
        .unwrap();

    assert!(!outcome.published);
    assert!(!outcome.violations.is_empty());

    let log = phases(work.path());
    assert!(log.contains("pre_build"));
    assert!(log.contains("post_build"));
    assert!(
        !log.contains("post_publish"),
        "irreversible phase ran after a refused gate:\n{log}"
    );
}

#[tokio::test]
async fn a_collapsed_vault_leaves_the_live_site_untouched() {
    let mock = MockNextcloud::start(&[
        ("a.md", "1"),
        ("b.md", "2"),
        ("c.md", "3"),
        ("d.md", "4"),
        ("e.md", "5"),
    ])
    .await;
    let work = tempfile::tempdir().unwrap();
    setup(work.path());
    let config = Arc::new(config_with_hooks(&mock, work.path(), 2));
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    pipeline::run_once(config.clone(), &source, &mut state, "manual")
        .await
        .unwrap();
    let published = publish::current_release(&config.publish.root).unwrap();

    // A sync accident on a phone: everything but one note disappears.
    for name in ["b.md", "c.md", "d.md", "e.md"] {
        mock.delete(name);
    }

    let outcome = pipeline::run_once(config.clone(), &source, &mut state, "poll")
        .await
        .unwrap();

    assert!(!outcome.published);
    assert!(
        outcome.violations.iter().any(|v| v.contains("dropped")),
        "{outcome:?}"
    );
    assert_eq!(
        publish::current_release(&config.publish.root).unwrap(),
        published,
        "the live site was replaced by a collapsed build"
    );
}

#[tokio::test]
async fn hooks_see_the_previous_release_for_diffing() {
    let mock = MockNextcloud::start(&[("a.md", "one"), ("b.md", "two")]).await;
    let work = tempfile::tempdir().unwrap();
    setup(work.path());
    let config = Arc::new(config_with_hooks(&mock, work.path(), 1));
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    pipeline::run_once(config.clone(), &source, &mut state, "manual")
        .await
        .unwrap();
    let first = publish::current_release(&config.publish.root).unwrap();

    mock.write("c.md", "three");
    pipeline::run_once(config.clone(), &source, &mut state, "poll")
        .await
        .unwrap();

    let log = phases(work.path());
    let last = log
        .lines()
        .filter(|l| l.starts_with("post_publish"))
        .next_back()
        .unwrap();
    assert!(
        last.contains(&format!("prev={}", first.display())),
        "a webmention diff needs the previous release; got: {last}"
    );
}

#[tokio::test]
async fn an_aborting_pre_build_hook_stops_before_anything_is_published() {
    let mock = MockNextcloud::start(&[("a.md", "one"), ("b.md", "two")]).await;
    let work = tempfile::tempdir().unwrap();
    setup(work.path());
    let config = Arc::new(config_with_hooks(&mock, work.path(), 1));
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    pipeline::run_once(config.clone(), &source, &mut state, "manual")
        .await
        .unwrap();
    let published = publish::current_release(&config.publish.root).unwrap();

    // Exit code 2 is "abort" in the hook contract.
    let hook = work.path().join("etc/hooks/record.sh");
    std::fs::write(
        &hook,
        "#!/bin/sh\necho 'nav aggregation failed' >&2\nexit 2\n",
    )
    .unwrap();
    make_executable(&hook);
    mock.write("c.md", "three");

    let error = pipeline::run_once(config.clone(), &source, &mut state, "poll")
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("aborted"), "{error}");
    assert_eq!(
        publish::current_release(&config.publish.root).unwrap(),
        published,
        "a failed hook must not change what is being served"
    );
}

#[tokio::test]
async fn retention_bounds_the_release_directory() {
    let mock = MockNextcloud::start(&[("a.md", "one"), ("b.md", "two")]).await;
    let work = tempfile::tempdir().unwrap();
    setup(work.path());
    let mut config = config_with_hooks(&mock, work.path(), 1);
    config.publish.keep_releases = 2;
    let config = Arc::new(config);
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    for i in 0..4 {
        mock.write(&format!("n{i}.md"), &format!("note {i}"));
        pipeline::run_once(config.clone(), &source, &mut state, "manual")
            .await
            .unwrap();
        // Release ids have one-second resolution.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }

    let releases: Vec<_> = std::fs::read_dir(publish::releases_dir(&config.publish.root))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        releases.len() <= 3,
        "retention did not bound the directory: {} entries",
        releases.len()
    );
    assert!(
        publish::current_release(&config.publish.root).is_some(),
        "the served release must survive retention"
    );
}
