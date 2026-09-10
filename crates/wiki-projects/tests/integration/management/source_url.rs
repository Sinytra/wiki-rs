use wiki_projects::management::match_repo_source;

const SOURCE: &str = "https://github.com/Sinytra/WikiService";

#[test]
fn identical_urls_match() {
    assert!(match_repo_source(SOURCE, SOURCE));
}

#[test]
fn trailing_slash_is_ignored() {
    assert!(match_repo_source(
        "https://github.com/Sinytra/WikiService/",
        SOURCE
    ));
    assert!(match_repo_source(
        SOURCE,
        "https://github.com/Sinytra/WikiService/"
    ));
}

#[test]
fn git_suffix_is_ignored() {
    assert!(match_repo_source(
        "https://github.com/Sinytra/WikiService.git",
        SOURCE
    ));
    assert!(match_repo_source(
        SOURCE,
        "https://github.com/Sinytra/WikiService.git"
    ));
}

#[test]
fn www_prefix_is_ignored() {
    assert!(match_repo_source(
        "https://www.github.com/Sinytra/WikiService",
        SOURCE
    ));
}

#[test]
fn differing_scheme_does_not_match() {
    assert!(!match_repo_source(
        "http://github.com/Sinytra/WikiService",
        SOURCE
    ));
    assert!(!match_repo_source(
        "ssh://git@github.com/Sinytra/WikiService.git",
        SOURCE
    ));
}

#[test]
fn case_differences_are_ignored() {
    assert!(match_repo_source(
        "HTTPS://GitHub.com/sinytra/wikiservice",
        SOURCE
    ));
}

#[test]
fn query_fragment_and_credentials_are_ignored() {
    assert!(match_repo_source(
        "https://user@github.com/Sinytra/WikiService?tab=readme#top",
        SOURCE
    ));
}

#[test]
fn redundant_slashes_and_whitespace_are_ignored() {
    assert!(match_repo_source(
        "  https://github.com//Sinytra///WikiService//  ",
        SOURCE
    ));
}

#[test]
fn urls_without_a_scheme_do_not_match() {
    assert!(!match_repo_source(
        "github.com/Sinytra/WikiService",
        SOURCE
    ));
    assert!(!match_repo_source(
        "//github.com/Sinytra/WikiService",
        SOURCE
    ));
    assert!(!match_repo_source(
        "git@github.com:Sinytra/WikiService.git",
        SOURCE
    ));
}

#[test]
fn non_http_schemes_match_their_own_kind() {
    assert!(match_repo_source(
        "ssh://git@github.com/Sinytra/WikiService.git",
        "ssh://github.com/Sinytra/WikiService"
    ));
}

#[test]
fn extra_path_segments_do_not_match() {
    let deeper = "https://github.com/Sinytra/WikiService/tree/master/docs";
    assert!(!match_repo_source(SOURCE, deeper));
    assert!(!match_repo_source(deeper, SOURCE));
}

#[test]
fn partial_segment_does_not_match() {
    assert!(!match_repo_source(
        "https://github.com/Sinytra/Wiki",
        SOURCE
    ));
}

#[test]
fn different_host_does_not_match() {
    assert!(!match_repo_source(
        "https://gitlab.com/Sinytra/WikiService",
        SOURCE
    ));
}

#[test]
fn different_owner_does_not_match() {
    assert!(!match_repo_source(
        "https://github.com/Someone/WikiService",
        SOURCE
    ));
}

#[test]
fn empty_or_invalid_urls_do_not_match() {
    assert!(!match_repo_source("", ""));
    assert!(!match_repo_source(SOURCE, ""));
    assert!(!match_repo_source("https://github.com", SOURCE));
    assert!(!match_repo_source("https:///Sinytra/WikiService", SOURCE));
    assert!(!match_repo_source("not a url", SOURCE));
}
