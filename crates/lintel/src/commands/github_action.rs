use anyhow::Result;

/// Run the `github-action` command: check files and post results as a GitHub Check Run.
pub async fn run(args: &lintel_github_action::GithubActionArgs) -> Result<bool> {
    lintel_github_action::run(args).await
}
