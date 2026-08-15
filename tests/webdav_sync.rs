//! Synchronisation against a mock Nextcloud.
//!
//! These cover the claims the design makes about change detection, which unit
//! tests cannot reach: that an unchanged vault costs one request, that only
//! changed files are fetched, and that the two HTTP failure modes behave
//! differently on purpose.

mod support;

use ncpages::source::Source;
use ncpages::state::State;
use support::{MockNextcloud, Mode};

fn workdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("etc")).unwrap();
    dir
}

#[tokio::test]
async fn initial_sync_downloads_every_file() {
    let mock = MockNextcloud::start(&[
        ("index.md", "home"),
        ("bounded-context.md", "ddd"),
        ("assets/logo.svg", "<svg/>"),
    ])
    .await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert!(report.changed);
    assert_eq!(report.downloaded, 3);
    assert_eq!(
        std::fs::read_to_string(config.paths.src.join("assets/logo.svg")).unwrap(),
        "<svg/>",
        "nested files must land at the same relative path"
    );
    assert!(state.root_etag.is_some(), "the root ETag must be persisted");
}

#[tokio::test]
async fn an_unchanged_vault_costs_exactly_one_request() {
    let mock = MockNextcloud::start(&[("index.md", "home"), ("a.md", "a")]).await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    source.sync(&config.paths.src, &mut state).await.unwrap();
    mock.reset_requests();

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert!(!report.changed);
    assert_eq!(
        mock.requests().len(),
        1,
        "ETag propagation should make one PROPFIND sufficient, got {:?}",
        mock.requests()
    );
    assert_eq!(mock.requests()[0].depth.as_deref(), Some("0"));
}

#[tokio::test]
async fn only_the_changed_file_is_downloaded_again() {
    let mock = MockNextcloud::start(&[("a.md", "one"), ("b.md", "two"), ("c.md", "three")]).await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    source.sync(&config.paths.src, &mut state).await.unwrap();
    mock.reset_requests();
    mock.write("b.md", "two, edited");

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(
        report.downloaded, 1,
        "unchanged files must not be re-fetched"
    );
    let fetched: Vec<_> = mock
        .requests()
        .into_iter()
        .filter(|r| r.method == "GET")
        .map(|r| r.path)
        .collect();
    assert_eq!(fetched.len(), 1);
    assert!(fetched[0].ends_with("/b.md"), "{fetched:?}");
    assert_eq!(
        std::fs::read_to_string(config.paths.src.join("b.md")).unwrap(),
        "two, edited"
    );
}

#[tokio::test]
async fn a_file_deleted_remotely_disappears_locally() {
    let mock = MockNextcloud::start(&[("a.md", "one"), ("b.md", "two")]).await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    source.sync(&config.paths.src, &mut state).await.unwrap();
    mock.delete("b.md");

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(report.deleted, 1);
    assert!(!config.paths.src.join("b.md").exists());
    assert!(config.paths.src.join("a.md").exists());
}

#[tokio::test]
async fn deep_directory_trees_are_descended() {
    let mock = MockNextcloud::start(&[
        ("index.md", "home"),
        ("a/b/c/deep.md", "deep"),
        ("a/b/other.md", "other"),
    ])
    .await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(report.downloaded, 3);
    assert_eq!(
        std::fs::read_to_string(config.paths.src.join("a/b/c/deep.md")).unwrap(),
        "deep"
    );
}

#[tokio::test]
async fn names_with_spaces_and_umlauts_survive_the_round_trip() {
    let mock = MockNextcloud::start(&[("Über Bäume.md", "trees"), ("a b/c d.md", "spaces")]).await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(report.downloaded, 2);
    assert_eq!(
        std::fs::read_to_string(config.paths.src.join("Über Bäume.md")).unwrap(),
        "trees"
    );
    assert_eq!(
        std::fs::read_to_string(config.paths.src.join("a b/c d.md")).unwrap(),
        "spaces"
    );
}

#[tokio::test]
async fn conflict_copies_are_excluded_from_the_build_and_reported() {
    let mock = MockNextcloud::start(&[
        ("note.md", "real"),
        ("note (conflicted copy 2026-08-14 120000).md", "lost work"),
    ])
    .await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(report.downloaded, 1);
    assert_eq!(report.conflict_copies.len(), 1, "the operator must be told");
    assert!(!config
        .paths
        .src
        .join("note (conflicted copy 2026-08-14 120000).md")
        .exists());
}

#[tokio::test]
async fn a_public_share_link_is_a_complete_credential() {
    // The smallest possible setup: no account, no app password, just a share
    // token. The share is read-only by nature, which matches what ncpages needs.
    let mock = MockNextcloud::start(&[("a.md", "one"), ("sub/b.md", "two")]).await;
    let work = workdir();
    let config = mock.share_config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(report.downloaded, 2);
    assert_eq!(
        std::fs::read_to_string(config.paths.src.join("sub/b.md")).unwrap(),
        "two"
    );

    let listing = mock
        .requests()
        .into_iter()
        .find(|r| r.method == "PROPFIND")
        .unwrap();
    assert!(
        listing.path.starts_with("/public.php/dav/files/"),
        "a share must use the public endpoint, got {}",
        listing.path
    );
}

#[tokio::test]
async fn an_unchanged_share_also_costs_exactly_one_request() {
    let mock = MockNextcloud::start(&[("a.md", "one")]).await;
    let work = workdir();
    let config = mock.share_config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    source.sync(&config.paths.src, &mut state).await.unwrap();
    mock.reset_requests();
    source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(mock.requests().len(), 1, "{:?}", mock.requests());
}

#[tokio::test]
async fn a_hostile_server_cannot_write_outside_the_working_copy() {
    // A compromised or malicious server can put anything in an href. Joining it
    // onto the destination unchecked would let it write anywhere this process
    // can reach.
    let mock = MockNextcloud::start(&[("a.md", "one")]).await;
    mock.poison_listing_with("../escaped.md");

    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(
        report.downloaded, 1,
        "only the legitimate file may be fetched"
    );
    assert!(
        !work.path().join("escaped.md").exists(),
        "a crafted href escaped the working copy"
    );
    assert!(
        config.paths.src.join("a.md").exists(),
        "the sync must still work"
    );
}

#[tokio::test]
async fn unauthorized_stops_immediately_rather_than_retrying() {
    let mock = MockNextcloud::start(&[("a.md", "one")]).await;
    mock.set_mode(Mode::Unauthorized);
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let error = source
        .sync(&config.paths.src, &mut state)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("401"), "{error}");
    assert!(error.contains("brute-force"), "{error}");
    assert_eq!(mock.requests().len(), 1, "it must not keep knocking");
}

#[tokio::test]
async fn maintenance_mode_is_named_in_the_error() {
    let mock = MockNextcloud::start(&[("a.md", "one")]).await;
    mock.set_mode(Mode::Maintenance);
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    let error = source
        .sync(&config.paths.src, &mut state)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("maintenance mode"), "{error}");
}

#[tokio::test]
async fn requests_carry_the_configured_host_header_and_credentials() {
    let mock = MockNextcloud::start(&[("a.md", "one")]).await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();

    source.probe().await.unwrap();

    let request = &mock.requests()[0];
    assert_eq!(
        request.host.as_deref(),
        Some("cloud.example.org"),
        "server_name matching and trusted_domains depend on this"
    );
    assert!(request.authorization, "credentials must be sent");
}

#[tokio::test]
async fn a_recovered_source_resumes_where_it_left_off() {
    let mock = MockNextcloud::start(&[("a.md", "one")]).await;
    let work = workdir();
    let config = mock.config(work.path());
    let source = Source::from_config(&config).unwrap();
    let mut state = State::default();

    source.sync(&config.paths.src, &mut state).await.unwrap();

    mock.set_mode(Mode::Maintenance);
    assert!(source.sync(&config.paths.src, &mut state).await.is_err());

    mock.set_mode(Mode::Ok);
    mock.write("b.md", "two");
    let report = source.sync(&config.paths.src, &mut state).await.unwrap();

    assert_eq!(
        report.downloaded, 1,
        "state must survive an unreachable source"
    );
    assert!(config.paths.src.join("b.md").exists());
}
